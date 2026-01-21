//! System command handling for fleet_server

use std::collections::HashMap;
use protocol::{GridMap, MapValidation, SystemCommand, LAYOUT_FILE_PATH};
use serde_json::to_vec;
use crate::state::TrackedRobot;

/// Result of processing a system command
pub enum CommandResult {
    /// Continue normal operation
    Continue,
    /// Map was reloaded - includes new map and validation message
    MapReloaded {
        map: GridMap,
        validation: MapValidation,
        validation_bytes: Vec<u8>,
    },
    /// Kill signal received
    Kill,
}

/// Handle a system command (pause/resume/reset/kill)
/// Returns CommandResult indicating what action was taken
pub fn handle_system_command(
    cmd: &SystemCommand,
    paused: &mut bool,
    robots: &mut HashMap<u32, TrackedRobot>,
) -> CommandResult {
    match cmd {
        SystemCommand::Pause => {
            *paused = true;
            println!("⏸ System PAUSED");
            CommandResult::Continue
        }
        SystemCommand::Resume => {
            *paused = false;
            println!("▶ System RESUMED");
            CommandResult::Continue
        }
        SystemCommand::Reset => {
            // Clear tracked robots (will be repopulated as updates arrive)
            robots.clear();
            
            // Reload map from file
            match GridMap::load_from_file(LAYOUT_FILE_PATH) {
                Ok(map) => {
                    println!("✓ Map reloaded: {}x{} (hash: {:016x})", map.width, map.height, map.hash);
                    
                    let validation = MapValidation {
                        sender: "fleet_server".to_string(),
                        map_hash: map.hash,
                        map_dimensions: (map.width, map.height),
                    };
                    
                    let validation_bytes = to_vec(&validation).unwrap_or_default();
                    
                    println!("🔄 System RESET - broadcast sent to swarm_driver & visualizer");
                    CommandResult::MapReloaded { map, validation, validation_bytes }
                }
                Err(e) => {
                    eprintln!("✗ Failed to reload map: {}", e);
                    println!("🔄 System RESET - broadcast sent (map reload failed)");
                    CommandResult::Continue
                }
            }
        }
        SystemCommand::Kill => {
            println!("☠ System KILL - exiting");
            CommandResult::Kill
        }
    }
}

/// Print current status of tracked robots
pub fn print_status(robots: &HashMap<u32, TrackedRobot>, paused: bool) {
    println!("═══ STATUS ═══");
    println!("  Paused: {}", paused);
    println!("  Tracked robots: {}", robots.len());
    for (id, robot) in robots {
        let path_status = if robot.path_complete() {
            "idle".to_string()
        } else {
            format!("waypoint {}/{}", robot.path_index + 1, robot.current_path.len())
        };
        println!("    Robot {}: {:?} @ [{:.1}, {:.1}, {:.1}] ({})",
            id, robot.last_update.state,
            robot.last_update.position[0],
            robot.last_update.position[1],
            robot.last_update.position[2],
            path_status);
    }
    println!("══════════════");
}
