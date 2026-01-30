//! Coordinator server main loop
//!
//! Handles task assignments, path planning, and robot command dispatch.

use zenoh::Session;
use tokio::time;
use tokio::sync::mpsc;
use protocol::*;
use protocol::config::coordinator as coord_config;
use serde_json::{to_vec, from_slice};
use std::collections::HashMap;

use crate::state::{TrackedRobot, TaskStage};
use crate::pathfinding::{self, Pathfinder, AStarPathfinder};
use crate::commands;

/// Stdin command variants (coordinator-specific only, system commands in orchestrator)
pub enum StdinCmd {
    Status,
    /// Send a robot to a grid position: goto <robot_id> <x> <y>
    Goto { robot_id: u32, x: usize, y: usize },
    Help,
}

/// Run the coordinator main loop
pub async fn run(session: Session, map: GridMap) {
    // Initialize pathfinder (easily swappable for WHCA* later)
    let pathfinder = AStarPathfinder::new();
    println!("✓ Pathfinder: {}", pathfinder.name());
    
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
    
    // Channel for stdin commands
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);
    
    println!("✓ Coordinator running. Waiting for robots...");
    
    let mut last_tick = std::time::Instant::now();
    let mut last_validation_publish = std::time::Instant::now();
    
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
                commands::handle_system_command(&sys_cmd, &mut paused, &mut verbose);
            }
        }
        
        // Handle task assignments (from scheduler)
        while let Ok(Some(sample)) = task_subscriber.try_recv() {
            if let Ok(assignment) = from_slice::<TaskAssignment>(&sample.payload().to_bytes()) {
                handle_task_assignment(&assignment, &mut robots, &map, &pathfinder, &cmd_publisher, &status_publisher, verbose).await;
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
            progress_tasks(&mut robots, &map, &pathfinder, &cmd_publisher, &status_publisher, verbose).await;
        }
        
        // Server tick - send path commands at configured rate
        if last_tick.elapsed() >= std::time::Duration::from_millis(coord_config::PATH_SEND_INTERVAL_MS) {
            last_tick = std::time::Instant::now();
            
            if !paused {
                send_path_commands(&mut robots, &cmd_publisher, verbose).await;
            }
        }
        
        time::sleep(std::time::Duration::from_millis(coord_config::LOOP_INTERVAL_MS)).await;
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
    _verbose: bool,
) {
    for (robot_id, robot) in robots.iter_mut() {
        let Some(waypoint) = robot.next_waypoint() else {
            continue;
        };
        
        // Check if robot reached current waypoint
        let pos = robot.last_update.position;
        let dist = ((pos[0] - waypoint[0]).powi(2) + (pos[2] - waypoint[2]).powi(2)).sqrt();
        
        if dist < coord_config::WAYPOINT_ARRIVAL_THRESHOLD {
            // Advance to next waypoint
            robot.advance_path();
            if let Some(next) = robot.next_waypoint() {
                let cmd = PathCmd {
                    robot_id: *robot_id,
                    command: build_path_command(robot, next),
                };
                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
            }
        } else {
            // Keep sending current waypoint command
            let cmd = PathCmd {
                robot_id: *robot_id,
                command: build_path_command(robot, waypoint),
            };
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

// ============================================================================
// Task Assignment Handler
// ============================================================================

/// Handle a task assignment from scheduler
async fn handle_task_assignment(
    assignment: &TaskAssignment,
    robots: &mut HashMap<u32, TrackedRobot>,
    map: &GridMap,
    pathfinder: &impl Pathfinder,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    verbose: bool,
) {
    let robot_id = assignment.robot_id;
    let task = &assignment.task;
    
    if verbose {
        println!("📥 Task {} assigned to Robot {}", task.id, robot_id);
    }
    
    // Get the robot
    let Some(robot) = robots.get_mut(&robot_id) else {
        println!("✗ Robot {} not found for task {}", robot_id, task.id);
        send_task_failure(status_publisher, task.id, robot_id, format!("Robot {} not found", robot_id)).await;
        return;
    };
    
    // Get pickup location
    let Some(pickup) = task.pickup_location() else {
        println!("✗ Task {} has no pickup location", task.id);
        return;
    };
    
    // Get dropoff location
    let Some(dropoff_grid) = task.target_location() else {
        println!("✗ Task {} has no dropoff location", task.id);
        return;
    };
    
    // Validate tile types: only shelf→shelf/dropoff or dropoff→shelf
    if !validate_pickup_dropoff(map, pickup, dropoff_grid) {
        let pickup_tile = map.get_tile(pickup.0, pickup.1).map(|t| t.tile_type);
        let dropoff_tile = map.get_tile(dropoff_grid.0, dropoff_grid.1).map(|t| t.tile_type);
        println!("✗ Task {} rejected: {:?} → {:?} invalid", task.id, pickup_tile, dropoff_tile);
        send_task_failure(status_publisher, task.id, robot_id, "Invalid pickup/dropoff combination".to_string()).await;
        return;
    }

    // Calculate path to pickup BEFORE accepting task
    let start = pathfinding::world_to_grid(robot.last_update.position);
    let Some(path_result) = pathfinder.find_path(map, start, pickup) else {
        println!("✗ No path found from Robot {} to pickup {:?}", robot_id, pickup);
        send_task_failure(status_publisher, task.id, robot_id, format!("No path to pickup {:?}", pickup)).await;
        return;
    };

    // Mark task as in-progress
    robot.current_task = Some(task.id);
    robot.task_stage = TaskStage::MovingToPickup;
    robot.pickup_location = Some(pathfinding::grid_to_world(pickup));
    robot.dropoff_location = Some(pathfinding::grid_to_world(dropoff_grid));
    
    if verbose {
        println!("→ Robot {} path to pickup: {} waypoints (cost: {})", robot_id, path_result.world_path.len(), path_result.cost);
    }
    robot.set_path(path_result.world_path);
    
    // Send in-progress status
    let update = TaskStatusUpdate {
        task_id: task.id,
        status: TaskStatus::InProgress { robot_id },
        robot_id: Some(robot_id),
    };
    if let Ok(payload) = to_vec(&update) {
        status_publisher.put(payload).await.ok();
    }
    
    // Send first waypoint command immediately
    if let Some(waypoint) = robot.next_waypoint() {
        let cmd = PathCmd {
            robot_id,
            command: PathCommand::MoveToPickup { target: waypoint, speed: coord_config::DEFAULT_SPEED },
        };
        cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
    }
}

/// Validate pickup/dropoff tile combination
fn validate_pickup_dropoff(map: &GridMap, pickup: (usize, usize), dropoff: (usize, usize)) -> bool {
    let pickup_tile = map.get_tile(pickup.0, pickup.1).map(|t| t.tile_type);
    let dropoff_tile = map.get_tile(dropoff.0, dropoff.1).map(|t| t.tile_type);
    
    matches!(
        (pickup_tile, dropoff_tile),
        (Some(grid_map::TileType::Shelf(_)), Some(grid_map::TileType::Shelf(_))) |
        (Some(grid_map::TileType::Shelf(_)), Some(grid_map::TileType::Dropoff)) |
        (Some(grid_map::TileType::Dropoff), Some(grid_map::TileType::Shelf(_)))
    )
}

/// Send a task failure status update
async fn send_task_failure(
    publisher: &zenoh::pubsub::Publisher<'_>,
    task_id: u64,
    robot_id: u32,
    reason: String,
) {
    let update = TaskStatusUpdate {
        task_id,
        status: TaskStatus::Failed { reason },
        robot_id: Some(robot_id),
    };
    if let Ok(payload) = to_vec(&update) {
        publisher.put(payload).await.ok();
    }
}

// ============================================================================
// Task Progression
// ============================================================================

/// Monitor task progression and send next waypoint when state changes
async fn progress_tasks(
    robots: &mut HashMap<u32, TrackedRobot>,
    map: &GridMap,
    pathfinder: &impl Pathfinder,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    verbose: bool,
) {
    for (robot_id, robot) in robots.iter_mut() {
        // Only process robots with active tasks
        let Some(task_id) = robot.current_task else {
            continue;
        };
        
        let robot_state = &robot.last_update.state;
        
        // State machine: detect transitions and send next PathCmd
        match robot_state {
            RobotState::Picking => {
                // Robot arrived at pickup - now send to dropoff
                if robot.task_stage == TaskStage::MovingToPickup {
                    robot.task_stage = TaskStage::Picking;
                    if verbose {
                        println!("📦 Robot {} picked up cargo for task {}", robot_id, task_id);
                    }
                    
                    // Send path to dropoff if available
                    if let Some(dropoff_world) = robot.dropoff_location {
                        let start = pathfinding::world_to_grid(robot.last_update.position);
                        let dropoff_grid = pathfinding::world_to_grid(dropoff_world);
                        
                        if let Some(result) = pathfinder.find_path(map, start, dropoff_grid) {
                            if verbose {
                                println!("🚚 Robot {} path to dropoff: {} waypoints", robot_id, result.world_path.len());
                            }
                            robot.set_path(result.world_path);
                            robot.task_stage = TaskStage::MovingToDropoff;
                            
                            // Send first waypoint
                            if let Some(waypoint) = robot.next_waypoint() {
                                let cmd = PathCmd {
                                    robot_id: *robot_id,
                                    command: PathCommand::MoveToDropoff { target: waypoint, speed: coord_config::DEFAULT_SPEED },
                                };
                                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
                            }
                        }
                    }
                }
            }
            RobotState::Idle => {
                // Robot is idle - check if it completed a task
                if robot.task_stage == TaskStage::MovingToDropoff || robot.task_stage == TaskStage::Delivering {
                    // Task completed!
                    robot.task_stage = TaskStage::Idle;
                    println!("✓ Task {} completed by Robot {}", task_id, robot_id);
                    
                    // Send completion status
                    let update = TaskStatusUpdate {
                        task_id,
                        status: TaskStatus::Completed,
                        robot_id: Some(*robot_id),
                    };
                    if let Ok(payload) = to_vec(&update) {
                        status_publisher.put(payload).await.ok();
                    }
                    
                    // Clear task
                    robot.current_task = None;
                    robot.pickup_location = None;
                    robot.dropoff_location = None;
                }
            }
            _ => {}
        }
    }
}
