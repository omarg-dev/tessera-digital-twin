//! System command handling for mock firmware (firmware layer)

use protocol::{PathCmd, RobotControl, SystemCommand, CommandResponse, CommandStatus};
use serde_json::{from_slice, to_vec};
use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::{Subscriber, Publisher};
use zenoh::sample::Sample;

use crate::robot::SimRobot;

/// Process system commands (pause/resume/chaos/time_scale)
pub fn handle_system_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    paused: &mut bool,
    chaos: &mut bool,
    time_scale: &mut f32,
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Ok(cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
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
        if let Ok(cmd) = from_slice::<RobotControl>(&sample.payload().to_bytes()) {
            match cmd {
                RobotControl::Down(id) => {
                    if let Some(robot) = robots.iter_mut().find(|r| r.id == id) {
                        robot.enabled = false;
                        println!("🔻 Robot {} disabled", id);
                        protocol::logs::save_log("Firmware", &format!("Robot {} disabled", id));
                    } else {
                        eprintln!("⚠ Cannot disable robot {}: not found", id);
                    }
                }
                RobotControl::Up(id) => {
                    if let Some(robot) = robots.iter_mut().find(|r| r.id == id) {
                        robot.enabled = true;
                        println!("🤖 Robot {} enabled", id);
                        protocol::logs::save_log("Firmware", &format!("Robot {} enabled", id));
                    } else {
                        eprintln!("⚠ Cannot enable robot {}: not found (robots are tied to stations)", id);
                    }
                }
                RobotControl::Restart(id) => {
                    if let Some(robot) = robots.iter_mut().find(|r| r.id == id) {
                        robot.restart();
                        println!("🔄 Robot {} restarted", id);
                        protocol::logs::save_log("Firmware", &format!("Robot {} restarted", id));
                    } else {
                        eprintln!("⚠ Cannot restart robot {}: not found", id);
                    }
                }
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
        if let Ok(cmd) = from_slice::<PathCmd>(&sample.payload().to_bytes()) {
            // Chaos: occasionally reject commands
            if protocol::chaos::should_reject_command(chaos) {
                protocol::chaos::log_chaos_event(
                    &format!("Rejected PathCmd for robot {}", cmd.robot_id),
                    "Firmware",
                );
                // Send rejection response
                let response = CommandResponse {
                    cmd_id: cmd.cmd_id,
                    robot_id: cmd.robot_id,
                    status: CommandStatus::Rejected { reason: "Chaos: Random rejection".to_string() },
                };
                if let Ok(payload) = to_vec(&response) {
                    let _ = response_publisher.put(payload);
                }
                continue;
            }
            
            if let Some(robot) = robots.iter_mut().find(|r| r.id == cmd.robot_id) {
                if !robot.enabled {
                    // Disabled robots reject commands silently
                    let response = CommandResponse {
                        cmd_id: cmd.cmd_id,
                        robot_id: cmd.robot_id,
                        status: CommandStatus::Rejected { reason: "Robot disabled".to_string() },
                    };
                    if let Ok(payload) = to_vec(&response) {
                        let _ = response_publisher.put(payload);
                    }
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
                let response = CommandResponse {
                    cmd_id: cmd.cmd_id,
                    robot_id: cmd.robot_id,
                    status,
                };
                if let Ok(payload) = to_vec(&response) {
                    let _ = response_publisher.put(payload);
                }
            } else {
                eprintln!("⚠ PathCmd for unknown robot {}", cmd.robot_id);
                // Send rejection for unknown robot
                let response = CommandResponse {
                    cmd_id: cmd.cmd_id,
                    robot_id: cmd.robot_id,
                    status: CommandStatus::Rejected { reason: "Robot not found".to_string() },
                };
                if let Ok(payload) = to_vec(&response) {
                    let _ = response_publisher.put(payload);
                }
            }
        }
    }
}
