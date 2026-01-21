//! System command handling for swarm_driver

use protocol::{PathCmd, SystemCommand};
use serde_json::from_slice;
use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::Subscriber;
use zenoh::sample::Sample;

use crate::robot::SimRobot;

/// Process system commands (pause/resume)
pub fn handle_system_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    paused: &mut bool,
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Ok(cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
            match cmd {
                SystemCommand::Pause => {
                    *paused = true;
                    println!("⏸ Physics PAUSED");
                }
                SystemCommand::Resume => {
                    *paused = false;
                    println!("▶ Physics RESUMED");
                }
            }
        }
    }
}

/// Process path commands from fleet_server
pub fn handle_path_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    robots: &mut [SimRobot],
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Ok(cmd) = from_slice::<PathCmd>(&sample.payload().to_bytes()) {
            if let Some(robot) = robots.iter_mut().find(|r| r.id == cmd.robot_id) {
                robot.apply_command(&cmd.command);
                println!("→ Robot {} received command: {:?}", cmd.robot_id, cmd.command);
            } else {
                eprintln!("⚠ PathCmd for unknown robot {}", cmd.robot_id);
            }
        }
    }
}
