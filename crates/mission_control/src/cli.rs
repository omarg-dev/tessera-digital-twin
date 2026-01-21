//! Command-line interface for mission control

use crate::allocator::RobotInfo;
use crate::queue::TaskQueue;
use protocol::SystemCommand;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

/// Stdin command variants
pub enum StdinCmd {
    Status,
    AddTask {
        pickup: (usize, usize),
        dropoff: (usize, usize),
    },
    System(SystemCommand),
}

/// Spawn a background task to read stdin commands
pub fn spawn_stdin_reader(tx: mpsc::Sender<StdinCmd>) {
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let cmd = match parts[0] {
                "status" => Some(StdinCmd::Status),
                "pause" => Some(StdinCmd::System(SystemCommand::Pause)),
                "resume" => Some(StdinCmd::System(SystemCommand::Resume)),
                "reset" => Some(StdinCmd::System(SystemCommand::Reset)),
                "kill" => Some(StdinCmd::System(SystemCommand::Kill)),
                "add" if parts.len() == 5 => {
                    let px = parts[1].parse().ok();
                    let py = parts[2].parse().ok();
                    let dx = parts[3].parse().ok();
                    let dy = parts[4].parse().ok();
                    match (px, py, dx, dy) {
                        (Some(px), Some(py), Some(dx), Some(dy)) => {
                            Some(StdinCmd::AddTask {
                                pickup: (px, py),
                                dropoff: (dx, dy),
                            })
                        }
                        _ => {
                            println!("Usage: add <pickup_x> <pickup_y> <dropoff_x> <dropoff_y>");
                            None
                        }
                    }
                }
                "add" => {
                    println!("Usage: add <pickup_x> <pickup_y> <dropoff_x> <dropoff_y>");
                    None
                }
                "help" => {
                    print_help();
                    None
                }
                _ => {
                    println!("Unknown command: {}. Type 'help' for available commands.", parts[0]);
                    None
                }
            };

            if let Some(cmd) = cmd {
                tx.send(cmd).await.ok();
            }
        }
    });
}

fn print_help() {
    println!("\n╔══════════════════════════════════════════╗");
    println!("║          MISSION CONTROL COMMANDS        ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║ status                  - Show status    ║");
    println!("║ add <px> <py> <dx> <dy> - Add task       ║");
    println!("║ pause                   - Pause system   ║");
    println!("║ resume                  - Resume system  ║");
    println!("║ reset                   - Reset all      ║");
    println!("║ kill                    - Shutdown all   ║");
    println!("║ help                    - Show this      ║");
    println!("╚══════════════════════════════════════════╝\n");
}

/// Print current status
pub fn print_status(queue: &dyn TaskQueue, robots: &HashMap<u32, RobotInfo>, paused: bool) {
    println!("\n╔══════════════════════════════════════════╗");
    println!("║          MISSION CONTROL STATUS          ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║ State: {:6}                            ║", if paused { "PAUSED" } else { "RUNNING" });
    println!("║ Queue: {} pending / {} total", queue.pending_count(), queue.total_count());
    println!("║ Robots: {} online", robots.len());
    println!("╠══════════════════════════════════════════╣");

    if !queue.all_tasks().is_empty() {
        println!("║ Tasks:");
        for task in queue.all_tasks() {
            println!("║   #{}: {:?} - {:?}", task.id, task.priority, task.status);
        }
    }

    if !robots.is_empty() {
        println!("╠══════════════════════════════════════════╣");
        println!("║ Robots:");
        for robot in robots.values() {
            let assigned = robot.assigned_task
                .map(|t| format!("Task {}", t))
                .unwrap_or_else(|| "idle".to_string());
            println!("║   Robot {}: {:?} @ ({:.0}, {:.0}) [{}]",
                robot.id, robot.state, robot.position[0], robot.position[2], assigned);
        }
    }

    println!("╚══════════════════════════════════════════╝\n");
}
