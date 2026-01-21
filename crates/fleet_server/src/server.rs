//! Fleet server core logic

use zenoh::Session;
use tokio::time;
use tokio::sync::mpsc;
use protocol::*;
use protocol::config::server as srv_config;
use serde_json::{to_vec, from_slice};
use std::collections::HashMap;

use crate::state::TrackedRobot;
use crate::pathfinding;
use crate::commands::{self, CommandResult};

/// Stdin command variants
pub enum StdinCmd {
    System(SystemCommand),
    Status,
    /// Send a robot to a grid position: goto <robot_id> <x> <y>
    Goto { robot_id: u32, x: usize, y: usize },
}

/// Run the fleet server main loop
pub async fn run(session: Session, mut map: GridMap) {
    // Publishers
    let cmd_publisher = session
        .declare_publisher(topics::PATH_COMMANDS)
        .await
        .expect("Failed to declare PATH_COMMANDS publisher");
    
    let control_publisher = session
        .declare_publisher(topics::ADMIN_CONTROL)
        .await
        .expect("Failed to declare ADMIN_CONTROL publisher");
    
    let map_publisher = session
        .declare_publisher(topics::MAP_VALIDATION)
        .await
        .expect("Failed to declare MAP_VALIDATION publisher");
    
    // Subscriber for robot updates
    let robot_subscriber = session
        .declare_subscriber(topics::ROBOT_UPDATES)
        .await
        .expect("Failed to declare ROBOT_UPDATES subscriber");
    
    // Broadcast map hash for validation (first time)
    let mut map_validation = MapValidation {
        sender: "fleet_server".to_string(),
        map_hash: map.hash,
        map_dimensions: (map.width, map.height),
    };
    map_publisher
        .put(to_vec(&map_validation).unwrap())
        .await
        .expect("Failed to publish map validation");
    println!("✓ Map hash broadcast for validation");
    
    // State
    let mut robots: HashMap<u32, TrackedRobot> = HashMap::new();
    let mut paused = false;
    
    // Channel for stdin commands
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);
    
    println!("✓ Fleet Server running. Waiting for robots...");
    
    let mut last_tick = std::time::Instant::now();
    let mut last_validation_publish = std::time::Instant::now();
    
    loop {
        // Republish map hash every 5 seconds (ensures latecomers can validate)
        if last_validation_publish.elapsed() >= std::time::Duration::from_secs(srv_config::MAP_HASH_REPUBLISH_SECS) {
            map_publisher
                .put(to_vec(&map_validation).unwrap())
                .await
                .ok();
            last_validation_publish = std::time::Instant::now();
        }
        
        // Handle stdin commands
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                StdinCmd::System(sys_cmd) => {
                    // Handle system command via commands module
                    match commands::handle_system_command(&sys_cmd, &mut paused, &mut robots) {
                        CommandResult::MapReloaded { map: new_map, validation, validation_bytes } => {
                            map = new_map;
                            map_validation = validation;
                            // Immediately republish map validation
                            let _ = map_publisher.put(validation_bytes).await;
                        }
                        CommandResult::Kill => {
                            std::process::exit(0);
                        }
                        CommandResult::Continue => {}
                    }
                    
                    // Broadcast command to swarm_driver & visualizer
                    control_publisher
                        .put(to_vec(&sys_cmd).unwrap())
                        .await
                        .ok();
                }
                StdinCmd::Status => {
                    commands::print_status(&robots, paused);
                }
                StdinCmd::Goto { robot_id, x, y } => {
                    if let Some(robot) = robots.get_mut(&robot_id) {
                        let start = pathfinding::world_to_grid(robot.last_update.position);
                        let goal = (x, y);
                        
                        if let Some(grid_path) = pathfinding::find_path(&map, start, goal) {
                            let world_path = pathfinding::grid_to_world_path(&grid_path);
                            println!("→ Robot {} path: {} waypoints", robot_id, world_path.len());
                            robot.set_path(world_path);
                        } else {
                            println!("✗ No path found for Robot {} to ({}, {})", robot_id, x, y);
                        }
                    } else {
                        println!("✗ Robot {} not found", robot_id);
                    }
                }
            }
        }
        
        // Process incoming robot updates (non-blocking) - expects RobotUpdateBatch
        while let Ok(Some(sample)) = robot_subscriber.try_recv() {
            // Try batched format first (current standard)
            if let Ok(batch) = from_slice::<RobotUpdateBatch>(&sample.payload().to_bytes()) {
                for update in batch.updates {
                    let robot = robots.entry(update.id).or_insert_with(|| {
                        println!("+ Robot {} connected", update.id);
                        TrackedRobot::new(update.clone())
                    });
                    robot.last_update = update;
                }
            }
            // Fallback: legacy individual RobotUpdate
            else if let Ok(update) = from_slice::<RobotUpdate>(&sample.payload().to_bytes()) {
                let robot = robots.entry(update.id).or_insert_with(|| {
                    println!("+ Robot {} connected", update.id);
                    TrackedRobot::new(update.clone())
                });
                robot.last_update = update;
            }
        }
        
        // Server tick (10 Hz) - send path commands
        if last_tick.elapsed() >= std::time::Duration::from_millis(srv_config::PATH_SEND_INTERVAL_MS) {
            last_tick = std::time::Instant::now();
            
            if !paused {
                for (robot_id, robot) in robots.iter_mut() {
                    if let Some(waypoint) = robot.next_waypoint() {
                        // Check if robot reached current waypoint
                        let pos = robot.last_update.position;
                        let dist = ((pos[0] - waypoint[0]).powi(2) + (pos[2] - waypoint[2]).powi(2)).sqrt();
                        
                        if dist < 0.2 {
                            // Advance to next waypoint
                            robot.advance_path();
                            if let Some(next) = robot.next_waypoint() {
                                let cmd = PathCmd {
                                    robot_id: *robot_id,
                                    command: PathCommand::MoveTo { target: next, speed: 2.0 },
                                };
                                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
                            }
                        } else {
                            // Keep sending current waypoint command
                            let cmd = PathCmd {
                                robot_id: *robot_id,
                                command: PathCommand::MoveTo { target: waypoint, speed: 2.0 },
                            };
                            cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
                        }
                    }
                }
            }
        }
        
        time::sleep(std::time::Duration::from_millis(srv_config::LOOP_INTERVAL_MS)).await;
    }
}

fn spawn_stdin_reader(tx: mpsc::Sender<StdinCmd>) {
    use tokio::io::{self, AsyncBufReadExt, BufReader};
    
    tokio::spawn(async move {
        let mut lines = BufReader::new(io::stdin()).lines();
        println!("Commands: 'pause', 'resume', 'reset', 'status', 'kill', 'goto <robot_id> <x> <y>'");
        
        while let Ok(Some(line)) = lines.next_line().await {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            
            let cmd = match parts[0].to_ascii_lowercase().as_str() {
                "pause" => Some(StdinCmd::System(SystemCommand::Pause)),
                "resume" => Some(StdinCmd::System(SystemCommand::Resume)),
                "reset" => Some(StdinCmd::System(SystemCommand::Reset)),
                "status" => Some(StdinCmd::Status),
                "kill" => Some(StdinCmd::System(SystemCommand::Kill)),
                "goto" if parts.len() >= 4 => {
                    let robot_id = parts[1].parse().ok();
                    let x = parts[2].parse().ok();
                    let y = parts[3].parse().ok();
                    match (robot_id, x, y) {
                        (Some(r), Some(x), Some(y)) => Some(StdinCmd::Goto { robot_id: r, x, y }),
                        _ => {
                            println!("Usage: goto <robot_id> <x> <y>");
                            None
                        }
                    }
                }
                _ => None,
            };
            
            if let Some(cmd) = cmd {
                let _ = tx.send(cmd).await;
            }
        }
    });
}
