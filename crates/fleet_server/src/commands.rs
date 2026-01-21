//! System command handling for fleet_server

use std::collections::HashMap;
use protocol::SystemCommand;
use crate::state::TrackedRobot;

/// Handle a system command (pause/resume)
pub fn handle_system_command(
    cmd: &SystemCommand,
    paused: &mut bool,
    _robots: &mut HashMap<u32, TrackedRobot>,
) {
    match cmd {
        SystemCommand::Pause => {
            *paused = true;
            println!("⏸ System PAUSED");
        }
        SystemCommand::Resume => {
            *paused = false;
            println!("▶ System RESUMED");
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
