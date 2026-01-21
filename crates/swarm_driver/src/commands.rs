//! System command handling for swarm_driver

use protocol::{GridMap, PathCmd, SystemCommand, LAYOUT_FILE_PATH};
use serde_json::from_slice;
use zenoh::handlers::FifoChannelHandler;
use zenoh::pubsub::Subscriber;
use zenoh::sample::Sample;

use crate::robot::SimRobot;

/// Result of processing system commands
pub enum CommandResult {
    /// Continue normal operation
    Continue,
    /// Map was reloaded (caller should update reference)
    MapReloaded(GridMap),
    /// Kill signal received
    Kill,
}

/// Process system commands (pause/resume/reset/kill)
pub fn handle_system_commands(
    subscriber: &Subscriber<FifoChannelHandler<Sample>>,
    paused: &mut bool,
    robots: &mut [SimRobot],
) -> CommandResult {
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
                SystemCommand::Reset => {
                    // Reset all robots to their initial state
                    for robot in robots.iter_mut() {
                        robot.reset();
                    }
                    
                    // Reload map from file
                    match GridMap::load_from_file(LAYOUT_FILE_PATH) {
                        Ok(new_map) => {
                            println!("✓ Map reloaded: {}x{} (hash: {:016x})", 
                                new_map.width, new_map.height, new_map.hash);
                            *paused = true; // Pause after reset
                            println!("🔄 Robots RESET to stations");
                            return CommandResult::MapReloaded(new_map);
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to reload map: {}", e);
                        }
                    }
                    
                    *paused = true;
                    println!("🔄 Robots RESET to stations");
                }
                SystemCommand::Kill => {
                    println!("☠ KILL received - exiting");
                    return CommandResult::Kill;
                }
            }
        }
    }
    CommandResult::Continue
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
