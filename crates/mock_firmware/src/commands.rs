//! System command handling for mock firmware (firmware layer)

use protocol::{PathCmd, RobotControl, SystemCommand, CommandResponse, CommandStatus};
use serde_json::{from_slice, to_vec};
use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::{Subscriber, Publisher};
use zenoh::sample::Sample;

use crate::robot::SimRobot;

fn publish_response(response_publisher: &Publisher, response: CommandResponse) {
    if let Ok(payload) = to_vec(&response) {
        let _ = response_publisher.put(payload);
    }
}

fn parse_sample<T: serde::de::DeserializeOwned>(sample: &Sample, kind: &str) -> Option<T> {
    match from_slice::<T>(&sample.payload().to_bytes()) {
        Ok(value) => Some(value),
        Err(e) => {
            protocol::logs::save_log("Firmware", &format!("Failed to parse {} payload: {}", kind, e));
            None
        }
    }
}

fn find_robot_mut(robots: &mut [SimRobot], id: u32) -> Option<&mut SimRobot> {
    robots.iter_mut().find(|r| r.id == id)
}

fn apply_robot_control(robot: &mut SimRobot, cmd: &RobotControl) {
    match cmd {
        RobotControl::Down(id) => {
            robot.enabled = false;
            println!("🔻 Robot {} disabled", id);
            protocol::logs::save_log("Firmware", &format!("Robot {} disabled", id));
        }
        RobotControl::Up(id) => {
            robot.enabled = true;
            println!("🤖 Robot {} enabled", id);
            protocol::logs::save_log("Firmware", &format!("Robot {} enabled", id));
        }
        RobotControl::Restart(id) => {
            robot.restart();
            println!("🔄 Robot {} restarted", id);
            protocol::logs::save_log("Firmware", &format!("Robot {} restarted", id));
        }
    }
}

/// Process system commands (pause/resume/chaos/time_scale)
pub fn handle_system_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    paused: &mut bool,
    chaos: &mut bool,
    time_scale: &mut f32,
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Some(cmd) = parse_sample::<SystemCommand>(&sample, "SystemCommand") {
            let effect = cmd.apply_with_log("Physics", Some(paused), None, Some(chaos));
            if let protocol::SystemCommandEffect::TimeScale(s) = effect {
                *time_scale = s.clamp(0.1, 1000.0);
            }
        }
    }
}

/// Process robot control commands (up/down/restart)
pub fn handle_robot_control(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    robots: &mut [SimRobot],
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Some(cmd) = parse_sample::<RobotControl>(&sample, "RobotControl") {
            let id = cmd.id();
            if let Some(robot) = find_robot_mut(robots, id) {
                apply_robot_control(robot, &cmd);
            } else {
                eprintln!("⚠ Cannot apply {:?}: robot {} not found", cmd, id);
                protocol::logs::save_log("Firmware", &format!("Robot control ignored, robot {} not found", id));
            }
        }
    }
}

/// Process path commands from coordinator
pub fn handle_path_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    response_publisher: &Publisher,
    robots: &mut [SimRobot],
    chaos: bool,
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Some(cmd) = parse_sample::<PathCmd>(&sample, "PathCmd") {
            // Chaos: occasionally reject commands
            if protocol::chaos::should_reject_command(chaos) {
                protocol::chaos::log_chaos_event(
                    &format!("Rejected PathCmd for robot {}", cmd.robot_id),
                    "Firmware",
                );
                publish_response(
                    response_publisher,
                    CommandResponse::rejected(cmd.cmd_id, cmd.robot_id, "Chaos: Random rejection"),
                );
                continue;
            }

            if let Some(robot) = find_robot_mut(robots, cmd.robot_id) {
                if !robot.enabled {
                    // Disabled robots reject commands silently
                    publish_response(
                        response_publisher,
                        CommandResponse::rejected(cmd.cmd_id, cmd.robot_id, "Robot disabled"),
                    );
                    continue;
                }
                
                // Apply command and get status
                let status = robot.apply_command(&cmd.command);
                
                // Log execution
                match &status {
                    CommandStatus::Accepted => {
                        println!("[{}ms] → Robot {} received command: {:?}", protocol::timestamp(), cmd.robot_id, cmd.command);
                        protocol::logs::save_log("Firmware", &format!("Robot {} executed command: {:?}", cmd.robot_id, cmd.command));
                    }
                    CommandStatus::Rejected { reason } => {
                        println!("[{}ms] ✗ Robot {} rejected command: {}", protocol::timestamp(), cmd.robot_id, reason);
                        protocol::logs::save_log("Firmware", &format!("Robot {} rejected command: {}", cmd.robot_id, reason));
                    }
                }
                
                // Send response back to coordinator
                publish_response(
                    response_publisher,
                    CommandResponse {
                        cmd_id: cmd.cmd_id,
                        robot_id: cmd.robot_id,
                        status,
                    },
                );
            } else {
                eprintln!("⚠ PathCmd for unknown robot {}", cmd.robot_id);
                publish_response(
                    response_publisher,
                    CommandResponse::rejected(cmd.cmd_id, cmd.robot_id, "Robot not found"),
                );
            }
        }
    }
}
