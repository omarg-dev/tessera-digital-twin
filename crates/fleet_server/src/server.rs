//! Fleet server core logic

use zenoh::Session;
use tokio::time;
use tokio::sync::mpsc;
use protocol::*;
use protocol::config::coordinator as coord_config;
use serde_json::{to_vec, from_slice};
use std::collections::HashMap;

use crate::state::TrackedRobot;
use crate::pathfinding;
use crate::commands;

/// Stdin command variants (fleet-specific only, system commands in control_plane)
pub enum StdinCmd {
    Status,
    /// Send a robot to a grid position: goto <robot_id> <x> <y>
    Goto { robot_id: u32, x: usize, y: usize },
    Help,
}

/// Run the fleet server main loop
pub async fn run(session: Session, map: GridMap) {
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
    
    // Channel for stdin commands
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);
    
    println!("✓ Fleet Server running. Waiting for robots...");
    
    let mut last_tick = std::time::Instant::now();
    let mut last_validation_publish = std::time::Instant::now();
    
    loop {
        // Republish map hash every 5 seconds (ensures latecomers can validate)
        if last_validation_publish.elapsed() >= std::time::Duration::from_secs(coord_config::MAP_HASH_REPUBLISH_SECS) {
            map_publisher
                .put(to_vec(&map_validation).unwrap())
                .await
                .ok();
            last_validation_publish = std::time::Instant::now();
        }
        
        // Handle system commands (from control_plane via Zenoh)
        while let Ok(Some(sample)) = control_subscriber.try_recv() {
            if let Ok(sys_cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
                commands::handle_system_command(&sys_cmd, &mut paused);
            }
        }
        
        // Handle task assignments (from mission_control)
        while let Ok(Some(sample)) = task_subscriber.try_recv() {
            if let Ok(assignment) = from_slice::<TaskAssignment>(&sample.payload().to_bytes()) {
                handle_task_assignment(&assignment, &mut robots, &map, &cmd_publisher, &status_publisher).await;
            }
        }
        
        // Handle stdin commands (fleet-specific only)
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
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
        // This runs every loop iteration to be responsive to state changes
        if !paused {
            progress_tasks(&mut robots, &map, &cmd_publisher, &status_publisher).await;
        }
        
        // Server tick (10 Hz) - send path commands
        if last_tick.elapsed() >= std::time::Duration::from_millis(coord_config::PATH_SEND_INTERVAL_MS) {
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
                                    command: match robot.current_task {
                                        Some(_) => {
                                            use crate::state::TaskStage;
                                            match robot.task_stage {
                                                TaskStage::MovingToPickup => PathCommand::MoveToPickup { target: next, speed: 2.0 },
                                                TaskStage::MovingToDropoff => PathCommand::MoveToDropoff { target: next, speed: 2.0 },
                                                _ => PathCommand::MoveTo { target: next, speed: 2.0 },
                                            }
                                        }
                                        None => PathCommand::MoveTo { target: next, speed: 2.0 },
                                    },
                                };
                                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
                            }
                        } else {
                            // Keep sending current waypoint command
                            let cmd = PathCmd {
                                robot_id: *robot_id,
                                command: match robot.current_task {
                                    Some(_) => {
                                        use crate::state::TaskStage;
                                        match robot.task_stage {
                                            TaskStage::MovingToPickup => PathCommand::MoveToPickup { target: waypoint, speed: 2.0 },
                                            TaskStage::MovingToDropoff => PathCommand::MoveToDropoff { target: waypoint, speed: 2.0 },
                                            _ => PathCommand::MoveTo { target: waypoint, speed: 2.0 },
                                        }
                                    }
                                    None => PathCommand::MoveTo { target: waypoint, speed: 2.0 },
                                },
                            };
                            cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
                        }
                    }
                }
            }
        }
        
        time::sleep(std::time::Duration::from_millis(coord_config::LOOP_INTERVAL_MS)).await;
    }
}

fn spawn_stdin_reader(tx: mpsc::Sender<StdinCmd>) {
    use tokio::io::{self, AsyncBufReadExt, BufReader};
    
    tokio::spawn(async move {
        let mut lines = BufReader::new(io::stdin()).lines();
        println!("Commands: 'status', 'goto <robot_id> <x> <y>', 'help'");
        println!("(System commands: run control_plane)");
        
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
                    println!("│  FLEET SERVER COMMANDS                  │");
                    println!("├─────────────────────────────────────────┤");
                    println!("│  status, s          - Show robot status │");
                    println!("│  goto <id> <x> <y>  - Send robot to pos │");
                    println!("│  help, h            - Show this help    │");
                    println!("├─────────────────────────────────────────┤");
                    println!("│  System commands: run control_plane    │");
                    println!("╰─────────────────────────────────────────╯");
                    Some(StdinCmd::Help)
                }
                "pause" | "resume" | "reset" | "kill" => {
                    println!("System commands moved to control_plane.");
                    println!("Run: cargo run -p control_plane");
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

/// Handle a task assignment from mission_control
async fn handle_task_assignment(
    assignment: &TaskAssignment,
    robots: &mut HashMap<u32, TrackedRobot>,
    map: &GridMap,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
) {
    use crate::state::TaskStage;
    
    let robot_id = assignment.robot_id;
    let task = &assignment.task;
    
    println!("📥 Task {} assigned to Robot {}", task.id, robot_id);
    
    // Get the robot
    let Some(robot) = robots.get_mut(&robot_id) else {
        println!("✗ Robot {} not found for task {}", robot_id, task.id);
        // Send failure status
        let update = TaskStatusUpdate {
            task_id: task.id,
            status: TaskStatus::Failed { reason: format!("Robot {} not found", robot_id) },
            robot_id: Some(robot_id),
        };
        if let Ok(payload) = to_vec(&update) {
            status_publisher.put(payload).await.ok();
        }
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
    let pickup_tile = map.get_tile(pickup.0, pickup.1).map(|t| t.tile_type);
    let dropoff_tile = map.get_tile(dropoff_grid.0, dropoff_grid.1).map(|t| t.tile_type);
    let valid_combo = match (pickup_tile, dropoff_tile) {
        (Some(protocol::grid_map::TileType::Shelf(_)), Some(protocol::grid_map::TileType::Shelf(_))) => true,
        (Some(protocol::grid_map::TileType::Shelf(_)), Some(protocol::grid_map::TileType::Dropoff)) => true,
        (Some(protocol::grid_map::TileType::Dropoff), Some(protocol::grid_map::TileType::Shelf(_))) => true,
        _ => false,
    };
    if !valid_combo {
        println!("✗ Task {} rejected: pickup {:?} → dropoff {:?} invalid (allowed: shelf→shelf/dropoff, dropoff→shelf)", task.id, pickup_tile, dropoff_tile);
        let update = TaskStatusUpdate {
            task_id: task.id,
            status: TaskStatus::Failed { reason: "Invalid pickup/dropoff combination".to_string() },
            robot_id: Some(robot_id),
        };
        if let Ok(payload) = to_vec(&update) {
            status_publisher.put(payload).await.ok();
        }
        return;
    }

    // Mark task as in-progress
    robot.current_task = Some(task.id);
    robot.task_stage = TaskStage::MovingToPickup;

    // Convert to world coordinates
    let pickup_world = [pickup.0 as f32, 0.25, pickup.1 as f32];
    let dropoff_world = Some([dropoff_grid.0 as f32, 0.25, dropoff_grid.1 as f32]);

    robot.pickup_location = Some(pickup_world);
    robot.dropoff_location = dropoff_world;

    // Calculate path to pickup
    let start = pathfinding::world_to_grid(robot.last_update.position);
    
    if let Some(grid_path) = pathfinding::find_path(map, start, pickup) {
        let world_path = pathfinding::grid_to_world_path(&grid_path);
        println!("→ Robot {} path to pickup: {} waypoints", robot_id, world_path.len());
        robot.set_path(world_path);
        
        // Send in-progress status
        let update = TaskStatusUpdate {
            task_id: task.id,
            status: TaskStatus::InProgress { robot_id },
            robot_id: Some(robot_id),
        };
        if let Ok(payload) = to_vec(&update) {
            status_publisher.put(payload).await.ok();
        }
        
        // Send first waypoint command immediately (pickup-specific)
        if let Some(waypoint) = robot.next_waypoint() {
            let cmd = PathCmd {
                robot_id,
                command: PathCommand::MoveToPickup { target: waypoint, speed: 2.0 },
            };
            cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
        }
    } else {
        println!("✗ No path found from Robot {} to pickup {:?}", robot_id, pickup);
        let update = TaskStatusUpdate {
            task_id: task.id,
            status: TaskStatus::Failed { reason: format!("No path to pickup {:?}", pickup) },
            robot_id: Some(robot_id),
        };
        if let Ok(payload) = to_vec(&update) {
            status_publisher.put(payload).await.ok();
        }
    }
}
/// Monitor task progression and send next waypoint when state changes
/// Handles: Picking → MovingToDrop → Delivering → MovingToStation → Idle
async fn progress_tasks(
    robots: &mut HashMap<u32, TrackedRobot>,
    map: &GridMap,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
) {
    use crate::state::TaskStage;
    use protocol::RobotState;
    
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
                    println!("📦 Robot {} picked up cargo for task {}", robot_id, task_id);
                    
                    // Send path to dropoff if available
                    if let Some(dropoff_world) = robot.dropoff_location {
                        let start = pathfinding::world_to_grid(robot.last_update.position);
                        let dropoff_grid = pathfinding::world_to_grid(dropoff_world);
                        
                        if let Some(grid_path) = pathfinding::find_path(map, start, dropoff_grid) {
                            let world_path = pathfinding::grid_to_world_path(&grid_path);
                            println!("🚚 Robot {} path to dropoff: {} waypoints", robot_id, world_path.len());
                            robot.set_path(world_path);
                            robot.task_stage = TaskStage::MovingToDropoff;
                            
                            // Send first waypoint
                            if let Some(waypoint) = robot.next_waypoint() {
                                let cmd = PathCmd {
                                    robot_id: *robot_id,
                                    command: PathCommand::MoveToDropoff { target: waypoint, speed: 2.0 },
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