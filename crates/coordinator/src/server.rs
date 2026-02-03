//! Coordinator server main loop
//!
//! Handles task assignments, path planning, and robot command dispatch.

use zenoh::Session;
use tokio::time;
use tokio::sync::mpsc;
use protocol::*;
use protocol::config::coordinator as coord_config;
use protocol::logs;
use serde_json::{to_vec, from_slice};
use std::collections::HashMap;

use crate::state::{TrackedRobot, TaskStage};
use crate::pathfinding::{self, Pathfinder, PathfinderInstance};
use crate::commands;
use crate::task_manager;

/// Stdin command variants (coordinator-specific only, system commands in orchestrator)
pub enum StdinCmd {
    Status,
    /// Send a robot to a grid position: goto <robot_id> <x> <y>
    Goto { robot_id: u32, x: usize, y: usize },
    Help,
}

/// Run the coordinator main loop
pub async fn run(session: Session, map: GridMap) {
    // Initialize pathfinder from config strategy
    let mut pathfinder = PathfinderInstance::from_config();
    println!(
        "✓ Pathfinder: {} (strategy: {})",
        pathfinder.name(),
        coord_config::PATHFINDING_STRATEGY
    );
    
    // Publishers
    let cmd_publisher = session
        .declare_publisher(topics::PATH_COMMANDS)
        .await
        .expect("Failed to declare PATH_COMMANDS publisher");
    
    let map_publisher = session
        .declare_publisher(topics::MAP_VALIDATION)
        .await
        .expect("Failed to declare MAP_VALIDATION publisher");
    
    let status_publisher = session
        .declare_publisher(topics::TASK_STATUS)
        .await
        .expect("Failed to declare TASK_STATUS publisher");
    
    // Subscribers
    let robot_subscriber = session
        .declare_subscriber(topics::ROBOT_UPDATES)
        .await
        .expect("Failed to declare ROBOT_UPDATES subscriber");
    
    let task_subscriber = session
        .declare_subscriber(topics::TASK_ASSIGNMENTS)
        .await
        .expect("Failed to declare TASK_ASSIGNMENTS subscriber");
    
    let control_subscriber = session
        .declare_subscriber(topics::ADMIN_CONTROL)
        .await
        .expect("Failed to declare ADMIN_CONTROL subscriber");
    
    let queue_subscriber = session
        .declare_subscriber(topics::QUEUE_STATE)
        .await
        .expect("Failed to declare QUEUE_STATE subscriber");
    
    let response_subscriber = session
        .declare_subscriber(topics::COMMAND_RESPONSES)
        .await
        .expect("Failed to declare COMMAND_RESPONSES subscriber");
    
    // Broadcast map hash for validation
    let map_validation = MapValidation {
        sender: topics::SENDER_COORDINATOR.to_string(),
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
    let mut verbose = true;
    let mut pending_tasks: usize = 0;  // From QueueState broadcasts
    let mut next_cmd_id: u64 = 1;  // Unique ID for command tracking
    
    // Channel for stdin commands
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);
    
    println!("✓ Coordinator running. Waiting for robots...");
    
    let mut last_tick = std::time::Instant::now();
    let mut last_validation_publish = std::time::Instant::now();
    let mut chaos = protocol::config::chaos::ENABLED;
    
    loop {
        // Republish map hash periodically (ensures latecomers can validate)
        if last_validation_publish.elapsed() >= std::time::Duration::from_secs(coord_config::MAP_HASH_REPUBLISH_SECS) {
            map_publisher
                .put(to_vec(&map_validation).unwrap())
                .await
                .ok();
            last_validation_publish = std::time::Instant::now();
        }
        
        // Handle system commands (from orchestrator via Zenoh)
        while let Ok(Some(sample)) = control_subscriber.try_recv() {
            if let Ok(sys_cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
                commands::handle_system_command(&sys_cmd, &mut paused, &mut verbose, &mut chaos);
            }
        }
        
        // Handle task assignments (from scheduler)
        while let Ok(Some(sample)) = task_subscriber.try_recv() {
            if let Ok(assignment) = from_slice::<TaskAssignment>(&sample.payload().to_bytes()) {
                let result = task_manager::handle_task_assignment(
                    &assignment,
                    &mut robots,
                    &map,
                    &mut pathfinder,
                    &cmd_publisher,
                    &status_publisher,
                    &mut next_cmd_id,
                    verbose,
                ).await;

                let reason = match result {
                    task_manager::AssignmentResult::Accepted { waypoints, cost } => {
                        format!("accepted: {} waypoints (cost: {})", waypoints, cost)
                    }
                    task_manager::AssignmentResult::LowBatteryReturn { battery } => {
                        format!("rejected: low battery ({:.1}%)", battery)
                    }
                    task_manager::AssignmentResult::RobotNotFound => "rejected: robot not found".to_string(),
                    task_manager::AssignmentResult::NoPickupLocation => "rejected: no pickup location".to_string(),
                    task_manager::AssignmentResult::NoDropoffLocation => "rejected: no dropoff location".to_string(),
                    task_manager::AssignmentResult::InvalidTileCombination => "rejected: invalid pickup/dropoff".to_string(),
                    task_manager::AssignmentResult::NoPathToPickup => "rejected: no path to pickup".to_string(),
                };

                let log = &format!("Task {} assignment result (robot {}): {}", assignment.task.id, assignment.robot_id, reason);
                
                if verbose {
                    println!("{}", log);
                }
                
                logs::save_log(
                    "Coordinator",
                    log,
                );
            }
        }
        
        // Handle queue state updates (from scheduler)
        while let Ok(Some(sample)) = queue_subscriber.try_recv() {
            if let Ok(state) = from_slice::<QueueState>(&sample.payload().to_bytes()) {
                pending_tasks = state.pending;
            }
        }
        
        // Handle stdin commands (coordinator-specific only)
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                StdinCmd::Status => {
                    commands::print_status(&robots, paused, verbose);
                }
                StdinCmd::Goto { robot_id, x, y } => {
                    if let Some(robot) = robots.get_mut(&robot_id) {
                        let start = pathfinding::world_to_grid(robot.last_update.position);
                        let goal = (x, y);
                        
                        if let Some(result) = pathfinder.find_path(&map, start, goal) {
                            println!("→ Robot {} path: {} waypoints (cost: {})", robot_id, result.world_path.len(), result.cost);
                            robot.set_path(result.world_path);
                        } else {
                            println!("✗ No path found for Robot {} to ({}, {})", robot_id, x, y);
                        }
                    } else {
                        println!("✗ Robot {} not found", robot_id);
                    }
                }
                StdinCmd::Help => {} // Already printed by stdin reader
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
        
        // Task progression: detect state transitions and send next waypoint
        if !paused {
            // Advance WHCA* planning window for multi-robot collision avoidance
            pathfinder.tick();
            
            task_manager::progress_tasks(&mut robots, &map, &mut pathfinder, &cmd_publisher, &status_publisher, &mut next_cmd_id, verbose, pending_tasks).await;
        }
        
        // Handle command responses from firmware
        handle_command_responses(&response_subscriber, verbose);
        
        // Server tick - send path commands at configured rate
        if last_tick.elapsed() >= std::time::Duration::from_millis(coord_config::PATH_SEND_INTERVAL_MS) {
            last_tick = std::time::Instant::now();
            
            if !paused {
                send_path_commands(&mut robots, &cmd_publisher, &mut next_cmd_id, verbose).await;
            }
        }
        
        time::sleep(std::time::Duration::from_millis(coord_config::LOOP_INTERVAL_MS)).await;
    }
}

/// Handle command responses from firmware
fn handle_command_responses(
    subscriber: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    verbose: bool,
) {
    while let Ok(Some(sample)) = subscriber.try_recv() {
        if let Ok(response) = from_slice::<CommandResponse>(&sample.payload().to_bytes()) {
            match response.status {
                CommandStatus::Accepted => {
                    if verbose {
                        println!("[{}ms] ✓ Robot {} accepted command #{}", 
                            timestamp(), response.robot_id, response.cmd_id);
                    }
                    logs::save_log("Coordinator", &format!(
                        "Robot {} accepted command #{}", 
                        response.robot_id, response.cmd_id
                    ));
                }
                CommandStatus::Rejected { ref reason } => {
                    println!("[{}ms] ✗ Robot {} rejected command #{}: {}", 
                        timestamp(), response.robot_id, response.cmd_id, reason);
                    logs::save_log("Coordinator", &format!(
                        "Robot {} rejected command #{}: {}", 
                        response.robot_id, response.cmd_id, reason
                    ));
                }
            }
        }
    }
}

// ============================================================================
// Path Command Helpers
// ============================================================================

/// Build a PathCommand based on robot's current task stage
fn build_path_command(robot: &TrackedRobot, target: [f32; 3]) -> PathCommand {
    let speed = coord_config::DEFAULT_SPEED;
    
    match robot.current_task {
        Some(_) => match robot.task_stage {
            TaskStage::MovingToPickup => PathCommand::MoveToPickup { target, speed },
            TaskStage::MovingToDropoff => PathCommand::MoveToDropoff { target, speed },
            _ => PathCommand::MoveTo { target, speed },
        },
        None => PathCommand::MoveTo { target, speed },
    }
}

/// Send path commands for all robots with active paths
async fn send_path_commands(
    robots: &mut HashMap<u32, TrackedRobot>,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    _verbose: bool,
) {
    for (robot_id, robot) in robots.iter_mut() {
        // Pause waypoint commands while robot is performing a pickup/dropoff action
        if matches!(robot.last_update.state, RobotState::Picking) {
            continue;
        }

        let Some(waypoint) = robot.next_waypoint() else {
            continue;
        };
        
        // Check if robot reached current waypoint
        let pos = robot.last_update.position;
        let dist = ((pos[0] - waypoint[0]).powi(2) + (pos[2] - waypoint[2]).powi(2)).sqrt();
        
        if dist < coord_config::WAYPOINT_ARRIVAL_THRESHOLD {
            // Advance to next waypoint - this is progress!
            robot.advance_path();
            robot.mark_progress();
            
            if let Some(next) = robot.next_waypoint() {
                let cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id: *robot_id,
                    command: build_path_command(robot, next),
                };
                *next_cmd_id += 1;
                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
            }
        } else {
            // Keep sending current waypoint command
            let cmd = PathCmd {
                cmd_id: *next_cmd_id,
                robot_id: *robot_id,
                command: build_path_command(robot, waypoint),
            };
            *next_cmd_id += 1;
            cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
        }
    }
}

// ============================================================================
// Stdin Reader
// ============================================================================

fn spawn_stdin_reader(tx: mpsc::Sender<StdinCmd>) {
    use tokio::io::{self, AsyncBufReadExt, BufReader};
    
    tokio::spawn(async move {
        let mut lines = BufReader::new(io::stdin()).lines();
        println!("Commands: 'status', 'goto <robot_id> <x> <y>', 'help'");
        println!("(System commands: run orchestrator)");
        
        while let Ok(Some(line)) = lines.next_line().await {
            let parts: Vec<&str> = line.trim().split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            
            let cmd = match parts[0].to_ascii_lowercase().as_str() {
                "status" | "s" => Some(StdinCmd::Status),
                "goto" | "g" if parts.len() >= 4 => {
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
                "help" | "h" | "?" => {
                    println!("╭─────────────────────────────────────────╮");
                    println!("│  COORDINATOR COMMANDS                   │");
                    println!("├─────────────────────────────────────────┤");
                    println!("│  status, s          - Show robot status │");
                    println!("│  goto <id> <x> <y>  - Send robot to pos │");
                    println!("│  help, h            - Show this help    │");
                    println!("├─────────────────────────────────────────┤");
                    println!("│  System commands: run orchestrator      │");
                    println!("╰─────────────────────────────────────────╯");
                    Some(StdinCmd::Help)
                }
                "pause" | "resume" | "reset" | "kill" => {
                    println!("System commands moved to orchestrator.");
                    println!("Run: cargo run -p orchestrator");
                    None
                }
                _ => {
                    println!("Unknown command. Type 'help' for available commands.");
                    None
                }
            };
            
            if let Some(cmd) = cmd {
                let _ = tx.send(cmd).await;
            }
        }
    });
}