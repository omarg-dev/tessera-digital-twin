//! System command handling for coordinator layer

use std::collections::HashMap;
use protocol::SystemCommand;
use crate::state::TrackedRobot;

/// Handle a system command (pause/resume/verbose/chaos)
pub fn handle_system_command(
    cmd: &SystemCommand,
    paused: &mut bool,
    verbose: &mut bool,
    chaos: &mut bool,
    time_scale: &mut f32,
) {
    let effect = cmd.apply_with_log("Coordinator", Some(paused), Some(verbose), Some(chaos));
    if let protocol::SystemCommandEffect::TimeScale(scale) = effect {
        *time_scale = scale.clamp(0.1, 1000.0);
    }
}

/// Print current status of tracked robots
pub fn print_status(robots: &HashMap<u32, TrackedRobot>, paused: bool, verbose: bool) {
    println!("═══ COORDINATOR STATUS ═══");
    println!("  Paused: {}  Verbose: {}", paused, verbose);
    println!("  Tracked robots: {}", robots.len());
    for (id, robot) in robots {
        let path_status = if robot.path_complete() {
            "idle".to_string()
        } else {
            format!("waypoint {}/{}", robot.path_index + 1, robot.current_path.len())
        };
        let task_info = robot.current_task
            .map(|t| format!(" [Task#{}:{:?}]", t, robot.task_stage))
            .unwrap_or_default();
        println!("    Robot {}: {:?} @ [{:.1}, {:.1}, {:.1}] ({}){}",
            id, robot.last_update.state,
            robot.last_update.position[0],
            robot.last_update.position[1],
            robot.last_update.position[2],
            path_status,
            task_info);
    }
    println!("═══════════════════════════");
}
