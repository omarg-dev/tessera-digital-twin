//! System command handling for mock firmware (firmware layer)

use protocol::{PathCmd, RobotControl, SystemCommand, CommandResponse, CommandStatus};
use serde_json::from_slice;
use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::{Subscriber, Publisher};
use zenoh::sample::Sample;

use crate::robot::SimRobot;

async fn publish_response(response_publisher: &Publisher<'_>, response: CommandResponse) {
    let _ = protocol::publish_json_logged(
        "Firmware",
        &format!("CommandResponse cmd_id={} robot_id={}", response.cmd_id, response.robot_id),
        &response,
        |payload| async move { response_publisher.put(payload).await.map(|_| ()) },
    )
    .await;
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
            protocol::logs::save_log("Firmware", &format!("Robot {} disabled", id));
        }
        RobotControl::Up(id) => {
            robot.enabled = true;
            protocol::logs::save_log("Firmware", &format!("Robot {} enabled", id));
        }
        RobotControl::Restart(id) => {
            robot.restart();
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
            let effect = cmd.apply(Some(paused), None, Some(chaos));
            if let protocol::SystemCommandEffect::TimeScale(s) = effect {
                *time_scale = s.clamp(0.1, 1000.0);
            }
            match effect {
                protocol::SystemCommandEffect::Paused(true) => {
                    protocol::logs::save_log("Firmware", "System paused");
                }
                protocol::SystemCommandEffect::Paused(false) => {
                    protocol::logs::save_log("Firmware", "System resumed");
                }
                protocol::SystemCommandEffect::Verbose(v) => {
                    protocol::logs::save_log("Firmware", &format!("Verbose set to {}", v));
                }
                protocol::SystemCommandEffect::Chaos(c) => {
                    protocol::logs::save_log("Firmware", &format!("Chaos set to {}", c));
                }
                protocol::SystemCommandEffect::TimeScale(s) => {
                    protocol::logs::save_log("Firmware", &format!("Time scale set to {:.2}x", s));
                }
                protocol::SystemCommandEffect::None => {}
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
                protocol::logs::save_log("Firmware", &format!("Robot control ignored, robot {} not found", id));
            }
        }
    }
}

/// Process path commands from coordinator
pub async fn handle_path_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    response_publisher: &Publisher<'_>,
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
                ).await;
                continue;
            }

            if let Some(robot) = find_robot_mut(robots, cmd.robot_id) {
                if !robot.enabled {
                    // Disabled robots reject commands silently
                    publish_response(
                        response_publisher,
                        CommandResponse::rejected(cmd.cmd_id, cmd.robot_id, "Robot disabled"),
                    ).await;
                    continue;
                }
                
                // Apply command and get status
                let status = robot.apply_command(&cmd.command);
                
                // Log execution
                match &status {
                    CommandStatus::Accepted => {
                        protocol::logs::save_log("Firmware", &format!("Robot {} executed command: {:?}", cmd.robot_id, cmd.command));
                    }
                    CommandStatus::Rejected { reason } => {
                        protocol::logs::save_log("Firmware", &format!("Robot {} rejected command: {}", cmd.robot_id, reason));
                    }
                }
                
                // Send response back to coordinator
                publish_response(
                    response_publisher,
                    match status {
                        CommandStatus::Accepted => CommandResponse::accepted(cmd.cmd_id, cmd.robot_id),
                        CommandStatus::Rejected { reason } => CommandResponse::rejected(cmd.cmd_id, cmd.robot_id, reason),
                    },
                ).await;
            } else {
                protocol::logs::save_log("Firmware", &format!("PathCmd rejected, unknown robot {}", cmd.robot_id));
                publish_response(
                    response_publisher,
                    CommandResponse::rejected(cmd.cmd_id, cmd.robot_id, "Robot not found"),
                ).await;
            }
        }
    }
}
