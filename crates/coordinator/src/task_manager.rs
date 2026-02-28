//! Task Manager - Task assignment and progression logic
//!
//! This module handles:
//! - Task assignment from scheduler
//! - Task progression state machine
//! - Task timeout detection
//! - Return-to-station logic
//!
//! Extracted from server.rs for improved testability and maintainability.

use std::collections::HashMap;
use protocol::*;
use protocol::config::coordinator as coord_config;
use protocol::config::coordinator::collision as collision_config;
use protocol::config::battery as battery_config;
use protocol::grid_map::ShelfInventory;
use protocol::logs;
use serde_json::to_vec;

use crate::state::{TrackedRobot, TaskStage, ReturnReason};
use crate::pathfinding::{self, GridPos, PathfinderInstance};

// ============================================================================
// Task Assignment
// ============================================================================

/// Result of a task assignment attempt
#[derive(Debug)]
pub enum AssignmentResult {
    /// Task accepted and path calculated
    Accepted { waypoints: usize, cost: u32 },
    /// Robot not found
    RobotNotFound,
    /// Robot is faulted or blocked
    RobotFaultedOrBlocked,
    /// Robot is busy with another task
    RobotBusy,
    /// Robot returning to station with low battery
    LowBatteryReturn { battery: f32 },
    /// No pickup location in task
    NoPickupLocation,
    /// No dropoff location in task
    NoDropoffLocation,
    /// Invalid pickup/dropoff tile combination
    InvalidTileCombination,
    /// Shelf capacity check failed (empty pickup or full dropoff)
    ShelfCapacity { reason: String },
    /// No path found to pickup
    NoPathToPickup,
}

/// Handle a task assignment from scheduler
pub async fn handle_task_assignment(
    assignment: &TaskAssignment,
    robots: &mut HashMap<u32, TrackedRobot>,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    inventory: &mut ShelfInventory,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
) -> AssignmentResult {
    let robot_id = assignment.robot_id;
    let task = &assignment.task;
    
    if verbose {
        println!("[{}ms] 📥 Task {} assigned to Robot {}", timestamp(), task.id, robot_id);
    }
    logs::save_log("Coordinator", &format!("Task {} assigned to Robot {}", task.id, robot_id));
    
    // Get the robot
    let Some(robot) = robots.get_mut(&robot_id) else {
        println!("✗ Robot {} not found for task {}", robot_id, task.id);
        send_task_failure(status_publisher, task.id, robot_id, format!("Robot {} not found", robot_id)).await;
        return AssignmentResult::RobotNotFound;
    };

    // Reject assignments for faulted/blocked robots
    if matches!(robot.last_update.state, RobotState::Faulted | RobotState::Blocked) {
        println!("✗ Task {} rejected: Robot {} is {:?}", task.id, robot_id, robot.last_update.state);
        send_task_failure(status_publisher, task.id, robot_id,
            format!("Robot {:?}", robot.last_update.state)).await;
        return AssignmentResult::RobotFaultedOrBlocked;
    }
    
    // If robot is returning to station due to no pending tasks, interrupt the return
    if robot.task_stage == TaskStage::ReturningToStation && robot.return_reason == Some(ReturnReason::NoPendingTasks) {
        if verbose {
            println!("[{}ms] ↩ Robot {} interrupted return-to-station for new task {}", 
                timestamp(), robot_id, task.id);
        }
        robot.task_stage = TaskStage::Idle;
        robot.return_reason = None;
    } else if robot.task_stage == TaskStage::ReturningToStation && robot.return_reason == Some(ReturnReason::LowBattery) {
        // Check if robot now has sufficient battery (was charging at station)
        let battery = robot.last_update.battery;
        if battery >= battery_config::MIN_BATTERY_FOR_TASK {
            if verbose {
                println!("[{}ms] 🔋 Robot {} charged to {:.1}%, accepting task {}", 
                    timestamp(), robot_id, battery, task.id);
            }
            robot.task_stage = TaskStage::Idle;
            robot.return_reason = None;
        } else {
            // Cannot accept tasks while returning due to low battery
            println!("✗ Task {} rejected: Robot {} is returning to station due to low battery ({:.1}%)", 
                task.id, robot_id, battery);
            send_task_failure(status_publisher, task.id, robot_id, 
                format!("Robot returning to station (low battery: {:.1}%)", battery)).await;
            return AssignmentResult::LowBatteryReturn { battery };
        }
    }

    // Reject assignments if robot is mid-task (except returning/charging with enough battery)
    let battery = robot.last_update.battery;
    let eligible_for_new_task = matches!(robot.task_stage, TaskStage::Idle)
        || (robot.task_stage == TaskStage::ReturningToStation
            && robot.return_reason == Some(ReturnReason::NoPendingTasks))
        || (matches!(robot.last_update.state, RobotState::Charging)
            && battery >= battery_config::MIN_BATTERY_FOR_TASK);
    if !eligible_for_new_task {
        println!("✗ Task {} rejected: Robot {} busy ({:?})", task.id, robot_id, robot.task_stage);
        send_task_failure(status_publisher, task.id, robot_id,
            format!("Robot busy ({:?})", robot.task_stage)).await;
        return AssignmentResult::RobotBusy;
    }
    
    // Get pickup location
    let Some(pickup) = task.pickup_location() else {
        println!("✗ Task {} has no pickup location", task.id);
        return AssignmentResult::NoPickupLocation;
    };
    
    logs::save_log("Coordinator", &format!(
        "[TASK {} DETAILS] Pickup grid: ({},{}), Pickup world: [{:.1},{:.1},{:.1}]",
        task.id, pickup.0, pickup.1,
        pathfinding::grid_to_world(pickup)[0],
        pathfinding::grid_to_world(pickup)[1],
        pathfinding::grid_to_world(pickup)[2]
    ));
    
    // Get dropoff location
    let Some(dropoff_grid) = task.target_location() else {
        println!("✗ Task {} has no dropoff location", task.id);
        return AssignmentResult::NoDropoffLocation;
    };
    
    logs::save_log("Coordinator", &format!(
        "[TASK {} DETAILS] Dropoff grid: ({},{}), Dropoff world: [{:.1},{:.1},{:.1}]",
        task.id, dropoff_grid.0, dropoff_grid.1,
        pathfinding::grid_to_world(dropoff_grid)[0],
        pathfinding::grid_to_world(dropoff_grid)[1],
        pathfinding::grid_to_world(dropoff_grid)[2]
    ));
    
    // Validate tile types: only shelf→shelf/dropoff or dropoff→shelf
    if !validate_pickup_dropoff(map, pickup, dropoff_grid) {
        let pickup_tile = map.get_tile(pickup.0, pickup.1).map(|t| t.tile_type);
        let dropoff_tile = map.get_tile(dropoff_grid.0, dropoff_grid.1).map(|t| t.tile_type);
        println!("✗ Task {} rejected: {:?} → {:?} invalid", task.id, pickup_tile, dropoff_tile);
        send_task_failure(status_publisher, task.id, robot_id, "Invalid pickup/dropoff combination".to_string()).await;
        return AssignmentResult::InvalidTileCombination;
    }

    // Shelf capacity enforcement: verify pickup has stock and dropoff has room
    if !inventory.can_pickup(pickup) {
        let stock = inventory.stock_at(pickup);
        let reason = format!("pickup shelf ({},{}) empty ({:?})", pickup.0, pickup.1, stock);
        println!("✗ Task {} rejected: {}", task.id, reason);
        send_task_failure(status_publisher, task.id, robot_id, reason.clone()).await;
        return AssignmentResult::ShelfCapacity { reason };
    }
    if !inventory.can_dropoff(dropoff_grid) {
        let stock = inventory.stock_at(dropoff_grid);
        let reason = format!("dropoff shelf ({},{}) full ({:?})", dropoff_grid.0, dropoff_grid.1, stock);
        println!("✗ Task {} rejected: {}", task.id, reason);
        send_task_failure(status_publisher, task.id, robot_id, reason.clone()).await;
        return AssignmentResult::ShelfCapacity { reason };
    }

    // Calculate path to pickup BEFORE accepting task
    let robot_world_pos = robot.last_update.position;
    let start = pathfinding::world_to_grid(robot_world_pos);
    
    logs::save_log("Coordinator", &format!(
        "[TASK {}] Robot {} starting at world [{:.1},{:.1},{:.1}] = grid ({},{})",
        task.id, robot_id, robot_world_pos[0], robot_world_pos[1], robot_world_pos[2], start.0, start.1
    ));
    
    let Some(path_result) = pathfinder.find_path_to_non_walkable_for_robot(map, start, pickup, robot_id) else {
        println!("✗ No path found from Robot {} to pickup {:?}", robot_id, pickup);
        logs::save_log("Coordinator", &format!("[ERROR] Pathfinding failed: Robot {} cannot reach pickup {:?}", robot_id, pickup));
        send_task_failure(status_publisher, task.id, robot_id, format!("No path to pickup {:?}", pickup)).await;
        return AssignmentResult::NoPathToPickup;
    };

    let waypoints = path_result.world_path.len();
    let cost = path_result.cost;

    // Reserve this path in WHCA* for multi-robot collision avoidance (use robot's velocity)
    let velocity = robot.last_update.velocity;
    pathfinder.reserve_path(robot_id, &path_result.grid_path, velocity);

    // Mark task as in-progress
    robot.current_task = Some(task.id);
    robot.task_stage = TaskStage::MovingToPickup;
    robot.task_started = Some(std::time::Instant::now());
    robot.mark_progress();
    
    // Store arrival points
    let pickup_arrival = path_result.world_path.last().copied()
        .unwrap_or_else(|| pathfinding::grid_to_world(pickup));
    let dropoff_world = pathfinding::grid_to_world(dropoff_grid);
    robot.pickup_location = Some(pickup_arrival);
    robot.dropoff_location = Some(dropoff_world);
    robot.pickup_grid = Some(pickup);
    robot.dropoff_grid = Some(dropoff_grid);
    
    if verbose {
        println!("[{}ms] → Robot {} path to pickup: {} waypoints (cost: {})", timestamp(), robot_id, waypoints, cost);
        for (i, wp) in path_result.world_path.iter().enumerate() {
            println!("  Waypoint {}: [{:.1}, {:.1}, {:.1}]", i, wp[0], wp[1], wp[2]);
        }
    }
    
    logs::save_log("Coordinator", &format!(
        "[TASK {}] Path to pickup ({},{}): {} waypoints, first=[{:.1},{:.1},{:.1}], arrival=[{:.1},{:.1},{:.1}]",
        task.id, pickup.0, pickup.1, waypoints,
        path_result.world_path.first().map(|w| w[0]).unwrap_or(-999.0),
        path_result.world_path.first().map(|w| w[1]).unwrap_or(-999.0),
        path_result.world_path.first().map(|w| w[2]).unwrap_or(-999.0),
        pickup_arrival[0], pickup_arrival[1], pickup_arrival[2]
    ));
    
    robot.set_path(path_result.world_path);
    
    // Skip to next waypoint if already at first waypoint
    let robot_pos = robot.last_update.position;
    if let Some(first_wp) = robot.next_waypoint() {
        let dist = ((robot_pos[0] - first_wp[0]).powi(2) + (robot_pos[2] - first_wp[2]).powi(2)).sqrt();
        if dist < coord_config::WAYPOINT_ARRIVAL_THRESHOLD {
            logs::save_log("Coordinator", &format!(
                "[TASK {}] Robot {} already at first waypoint [{:.1},{:.1},{:.1}], advancing",
                task.id, robot_id, first_wp[0], first_wp[1], first_wp[2]
            ));
            robot.advance_path();
        }
    }
    
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
            cmd_id: *next_cmd_id,
            robot_id,
            command: PathCommand::MoveToPickup { target: waypoint, speed: coord_config::DEFAULT_SPEED },
        };
        *next_cmd_id += 1;
        cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
    }
    
    AssignmentResult::Accepted { waypoints, cost }
}

/// Validate pickup/dropoff tile combination
fn validate_pickup_dropoff(map: &GridMap, pickup: GridPos, dropoff: GridPos) -> bool {
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
pub async fn send_task_failure(
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

/// Check if robot position is near a target (within arrival threshold)
fn is_near(pos: [f32; 3], target: [f32; 3]) -> bool {
    let dist = ((pos[0] - target[0]).powi(2) + (pos[2] - target[2]).powi(2)).sqrt();
    dist < coord_config::WAYPOINT_ARRIVAL_THRESHOLD
}

/// Monitor task progression and send commands when robot reaches destinations
pub async fn progress_tasks(
    robots: &mut HashMap<u32, TrackedRobot>,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    inventory: &mut ShelfInventory,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    robot_control_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
    pending_tasks: usize,
) {
    // First pass: check for timed out tasks
    handle_task_timeouts(robots, status_publisher, pathfinder).await;

    // Fault cleanup: restart faulted robots after delay
    handle_fault_cleanup(robots, robot_control_publisher, verbose).await;

    // Blocked handling: immediate replan or fault escalation
    handle_blocked_robots(robots, map, pathfinder, cmd_publisher, status_publisher, next_cmd_id, verbose).await;
    
    // Second pass: reserve stationary robot positions for WHCA* collision avoidance
    for (robot_id, robot) in robots.iter() {
        let robot_pos = pathfinding::world_to_grid(robot.last_update.position);
        
        // Reserve position if robot is stationary (not actively moving)
        match robot.task_stage {
            TaskStage::Idle | TaskStage::Picking | TaskStage::Delivering => {
                pathfinder.reserve_stationary_history(*robot_id, &robot.recent_positions, robot_pos);
            }
            _ => {}
        }

        if matches!(robot.last_update.state, RobotState::Faulted | RobotState::Blocked) {
            pathfinder.reserve_stationary_history(*robot_id, &robot.recent_positions, robot_pos);
        }
    }
    
    // Third pass: normal task progression
    for (robot_id, robot) in robots.iter_mut() {
        // Deadlock breaker: if waiting too long on a reserved cell, attempt replan
        if let Some(wait_secs) = robot.wait_elapsed_secs() {
            if wait_secs >= collision_config::RESERVATION_WAIT_REPLAN_SECS {
                let replanned = attempt_replan(
                    robot,
                    *robot_id,
                    map,
                    pathfinder,
                    cmd_publisher,
                    next_cmd_id,
                    verbose,
                ).await;
                if replanned {
                    robot.clear_wait();
                    continue;
                }
            }
        }

        // Replan immediately if robot has deviated from its path
        if should_replan_for_deviation(robot) {
            let replanned = attempt_replan(
                robot,
                *robot_id,
                map,
                pathfinder,
                cmd_publisher,
                next_cmd_id,
                verbose,
            ).await;
            if replanned {
                continue;
            }
        }

        // Handle returning to station (no task required)
        if robot.task_stage == TaskStage::ReturningToStation {
            handle_returning_to_station(robot, *robot_id, cmd_publisher, next_cmd_id, verbose).await;
            continue;
        }
        
        // Only process robots with active tasks
        let Some(task_id) = robot.current_task else {
            // Idle robot with low battery should return to station
            handle_idle_low_battery(robot, *robot_id, map, pathfinder, cmd_publisher, next_cmd_id, verbose).await;
            continue;
        };
        
        match robot.task_stage {
            TaskStage::MovingToPickup => {
                handle_moving_to_pickup(robot, *robot_id, task_id, cmd_publisher, next_cmd_id, verbose).await;
            }
            TaskStage::Picking => {
                handle_picking(robot, *robot_id, task_id, map, pathfinder, inventory, cmd_publisher, next_cmd_id, verbose).await;
            }
            TaskStage::MovingToDropoff => {
                handle_moving_to_dropoff(
                    robot, *robot_id, task_id,
                    cmd_publisher, next_cmd_id, verbose
                ).await;
            }
            TaskStage::Delivering => {
                handle_delivering(
                    robot, *robot_id, task_id,
                    map,
                    pathfinder,
                    inventory,
                    cmd_publisher,
                    status_publisher,
                    next_cmd_id,
                    verbose,
                    pending_tasks,
                ).await;
            }
            _ => {}
        }
    }
}

/// Handle timed out tasks
async fn handle_task_timeouts(
    robots: &mut HashMap<u32, TrackedRobot>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    pathfinder: &mut PathfinderInstance,
) {
    let timeout_secs = coord_config::TASK_TIMEOUT_SECS;
    let mut timed_out_tasks: Vec<(u32, u64)> = Vec::new();
    
    for (robot_id, robot) in robots.iter() {
        if let Some(task_id) = robot.current_task {
            if robot.is_task_timed_out(timeout_secs) {
                timed_out_tasks.push((*robot_id, task_id));
            }
        }
    }
    
    for (robot_id, task_id) in timed_out_tasks {
        if let Some(robot) = robots.get_mut(&robot_id) {
            let elapsed = robot.last_progress.elapsed().as_secs();
            println!("[{}ms] ⏰ TIMEOUT: Task {} on Robot {} (no progress for {}s)", 
                timestamp(), task_id, robot_id, elapsed);
            logs::save_log("Coordinator", &format!(
                "Task {} timed out on Robot {} ({}s no progress - possibly disabled/crashed)", 
                task_id, robot_id, elapsed
            ));
            
            let update = TaskStatusUpdate {
                task_id,
                status: TaskStatus::Failed { reason: format!("Timeout ({}s no progress)", elapsed) },
                robot_id: Some(robot_id),
            };
            if let Ok(payload) = to_vec(&update) {
                status_publisher.put(payload).await.ok();
            }
            
            // Clear robot state
            robot.current_task = None;
            robot.task_stage = TaskStage::Idle;
            robot.task_started = None;
            robot.pickup_location = None;
            robot.dropoff_location = None;
            robot.pickup_grid = None;
            robot.dropoff_grid = None;
            robot.clear_wait();
            
            // Clear reservations from multi-robot coordination
            pathfinder.clear_robot_reservations(robot_id);
            robot.current_path.clear();
            robot.path_index = 0;
            robot.mark_progress();
        }
    }
}

/// Mark a robot as faulted and clear its task/path state
pub async fn mark_robot_faulted(
    robot: &mut TrackedRobot,
    robot_id: u32,
    reason: String,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    pathfinder: &mut PathfinderInstance,
    verbose: bool,
) {
    if robot.faulted_since.is_some() {
        return; // already faulted
    }

    if verbose {
        println!("[{}ms] ⚠ Robot {} FAULTED: {}", timestamp(), robot_id, reason);
    }
    logs::save_log("Coordinator", &format!("Robot {} faulted: {}", robot_id, reason));

    if let Some(task_id) = robot.current_task {
        send_task_failure(status_publisher, task_id, robot_id, reason).await;
    }

    // Clear robot state
    robot.current_task = None;
    robot.task_stage = TaskStage::Idle;
    robot.task_started = None;
    robot.pickup_location = None;
    robot.dropoff_location = None;
    robot.pickup_grid = None;
    robot.dropoff_grid = None;
    robot.return_reason = None;
    robot.current_path.clear();
    robot.path_index = 0;
    robot.replan_attempts = 0;
    robot.blocked_since = None;
    robot.clear_wait();
    robot.faulted_since = Some(std::time::Instant::now());

    // Update local state to reflect fault
    robot.last_update.state = RobotState::Faulted;

    // Clear reservations for this robot
    pathfinder.clear_robot_reservations(robot_id);
    robot.mark_progress();
}

/// Handle blocked robots: immediate replan or escalate to fault
async fn handle_blocked_robots(
    robots: &mut HashMap<u32, TrackedRobot>,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
) {
    let mut blocked_ids = Vec::new();
    for (robot_id, robot) in robots.iter_mut() {
        if robot.last_update.state == RobotState::Blocked {
            if robot.blocked_since.is_none() {
                robot.blocked_since = Some(std::time::Instant::now());
            }
            blocked_ids.push(*robot_id);
        }
    }

    for robot_id in blocked_ids {
        let Some(robot) = robots.get_mut(&robot_id) else { continue; };

        // If blocked too long, escalate to fault
        if let Some(since) = robot.blocked_since {
            if since.elapsed().as_secs() >= collision_config::BLOCKED_TIMEOUT_SECS {
                mark_robot_faulted(
                    robot,
                    robot_id,
                    format!("Blocked timeout ({}s)", collision_config::BLOCKED_TIMEOUT_SECS),
                    status_publisher,
                    pathfinder,
                    verbose,
                ).await;
                continue;
            }
        }

        // Try immediate replan
        let replanned = attempt_replan(
            robot,
            robot_id,
            map,
            pathfinder,
            cmd_publisher,
            next_cmd_id,
            verbose,
        ).await;

        if replanned {
            robot.blocked_since = None;
            robot.replan_attempts = 0;
        } else {
            robot.replan_attempts += 1;
            if robot.replan_attempts >= collision_config::MAX_REPLAN_ATTEMPTS {
                mark_robot_faulted(
                    robot,
                    robot_id,
                    format!("Replan failed ({} attempts)", robot.replan_attempts),
                    status_publisher,
                    pathfinder,
                    verbose,
                ).await;
            }
        }
    }
}

/// Restart faulted robots after cleanup delay
async fn handle_fault_cleanup(
    robots: &mut HashMap<u32, TrackedRobot>,
    robot_control_publisher: &zenoh::pubsub::Publisher<'_>,
    verbose: bool,
) {
    for (robot_id, robot) in robots.iter_mut() {
        let Some(since) = robot.faulted_since else { continue; };
        if since.elapsed().as_secs() >= collision_config::FAULT_CLEANUP_DELAY_SECS {
            let cmd = RobotControl::Restart(*robot_id);
            if let Ok(payload) = to_vec(&cmd) {
                robot_control_publisher.put(payload).await.ok();
            }
            if verbose {
                println!("[{}ms] 🔄 Robot {} restart after fault cleanup", timestamp(), robot_id);
            }
            logs::save_log("Coordinator", &format!("Robot {} restart after fault cleanup", robot_id));
            robot.faulted_since = None;
            robot.blocked_since = None;
            robot.replan_attempts = 0;
            robot.task_stage = TaskStage::Idle;
            robot.last_update.state = RobotState::Idle;
            robot.last_tick = None;
            robot.skip_next_validation = true;
        }
    }
}

/// Check if robot deviated from path enough to replan
fn should_replan_for_deviation(robot: &TrackedRobot) -> bool {
    if robot.current_task.is_none() {
        return false;
    }
    match robot.task_stage {
        TaskStage::MovingToPickup | TaskStage::MovingToDropoff | TaskStage::Delivering => {}
        _ => return false,
    }

    let Some(next_wp) = robot.next_waypoint() else { return false; };
    let pos = robot.last_update.position;
    let dist = ((pos[0] - next_wp[0]).powi(2) + (pos[2] - next_wp[2]).powi(2)).sqrt();
    dist > collision_config::MAX_PATH_DEVIATION_TILES
}

/// Attempt to replan to the current task target
async fn attempt_replan(
    robot: &mut TrackedRobot,
    robot_id: u32,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
) -> bool {
    let Some(task_id) = robot.current_task else { return false; };

    let target_world = match robot.task_stage {
        TaskStage::MovingToPickup => robot.pickup_location,
        TaskStage::MovingToDropoff | TaskStage::Delivering => robot.dropoff_location,
        _ => None,
    };

    let Some(target_world) = target_world else { return false; };
    let start = pathfinding::world_to_grid(robot.last_update.position);
    let goal = pathfinding::world_to_grid(target_world);

    let Some(result) = pathfinder.find_path_for_robot(map, start, goal, robot_id) else { return false; };

    // Clear old reservations and reserve new path
    pathfinder.clear_robot_reservations(robot_id);
    pathfinder.reserve_path(robot_id, &result.grid_path, robot.last_update.velocity);

    robot.set_path(result.world_path);
    robot.mark_progress();

    if verbose {
        println!("[{}ms] 🔁 Robot {} replanned (task {}) - {} waypoints", timestamp(), robot_id, task_id, robot.current_path.len());
    }

    if let Some(waypoint) = robot.next_waypoint() {
        let command = match robot.task_stage {
            TaskStage::MovingToPickup => PathCommand::MoveToPickup { target: waypoint, speed: coord_config::DEFAULT_SPEED },
            TaskStage::MovingToDropoff | TaskStage::Delivering => PathCommand::MoveToDropoff { target: waypoint, speed: coord_config::DEFAULT_SPEED },
            _ => return false,
        };
        let cmd = PathCmd { cmd_id: *next_cmd_id, robot_id, command };
        *next_cmd_id += 1;
        cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
    }

    true
}

/// Handle robot returning to station
async fn handle_returning_to_station(
    robot: &mut TrackedRobot,
    robot_id: u32,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
) {
    let robot_pos = robot.last_update.position;
    
    // Check if robot has completed path to station
    if robot.path_complete() {
        let station_pos = robot.last_update.station_position;
        if is_near(robot_pos, station_pos) {
            if verbose {
                println!("[{}ms] 🔋 Robot {} arrived at station, charging", timestamp(), robot_id);
            }
            
            let cmd = PathCmd {
                cmd_id: *next_cmd_id,
                robot_id,
                command: PathCommand::ReturnToCharge,
            };
            *next_cmd_id += 1;
            cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
            
            robot.current_path.clear();
            robot.path_index = 0;
        }
    }
    
    // Check if robot is done charging
    let battery = robot.last_update.battery;
    if battery >= battery_config::MIN_BATTERY_FOR_TASK {
        if verbose {
            println!("[{}ms] ✅ Robot {} charged to {:.1}%, available for tasks", 
                timestamp(), robot_id, battery);
        }
        logs::save_log("Coordinator", &format!(
            "Robot {} ready: battery {:.1}%", robot_id, battery
        ));
        
        robot.task_stage = TaskStage::Idle;
        robot.return_reason = None;
    }
}

/// Handle idle robot with low battery
async fn handle_idle_low_battery(
    robot: &mut TrackedRobot,
    robot_id: u32,
    map: &GridMap,
    pathfinder: &PathfinderInstance,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,    next_cmd_id: &mut u64,    verbose: bool,
) {
    let robot_pos = robot.last_update.position;
    let battery = robot.last_update.battery;
    let station_pos = robot.last_update.station_position;
    
    if battery < battery_config::LOW_THRESHOLD && !is_near(robot_pos, station_pos) {
        let current_grid = pathfinding::world_to_grid(robot_pos);
        let station_grid = pathfinding::world_to_grid(station_pos);
        
        if let Some(result) = pathfinder.find_path_for_robot(map, current_grid, station_grid, robot_id) {
            if verbose {
                println!("[{}ms] ⚠ Robot {} low battery ({:.1}%), returning to station", 
                    timestamp(), robot_id, battery);
            }
            logs::save_log("Coordinator", &format!(
                "Robot {} low battery ({:.1}%), returning to station", robot_id, battery
            ));
            
            robot.set_path(result.world_path);
            robot.task_stage = TaskStage::ReturningToStation;
            robot.return_reason = Some(ReturnReason::LowBattery);
            
            if let Some(waypoint) = robot.next_waypoint() {
                let cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id,
                    command: PathCommand::MoveTo { target: waypoint, speed: coord_config::DEFAULT_SPEED },
                };
                *next_cmd_id += 1;
                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
            }
        }
    }
}

/// Handle robot moving to pickup
async fn handle_moving_to_pickup(
    robot: &mut TrackedRobot,
    robot_id: u32,
    task_id: u64,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
) {
    let robot_pos = robot.last_update.position;
    
    if robot.path_complete() {
        if let Some(pickup_world) = robot.pickup_location {
            if is_near(robot_pos, pickup_world) {
                robot.task_stage = TaskStage::Picking;
                robot.mark_progress();
                
                if verbose {
                    println!("[{}ms] 📍 Robot {} arrived at pickup for task {}", timestamp(), robot_id, task_id);
                }
                
                let cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id,
                    command: PathCommand::Pickup { cargo_id: task_id as u32 },
                };
                *next_cmd_id += 1;
                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
                
                if verbose {
                    println!("[{}ms] 📦 Robot {} picking up cargo ({}s delay)...", 
                        timestamp(), robot_id, coord_config::PICKUP_DELAY_SECS);
                }
            }
        }
    }
}

/// Handle robot picking up cargo
async fn handle_picking(
    robot: &mut TrackedRobot,
    robot_id: u32,
    task_id: u64,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    inventory: &mut ShelfInventory,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,    next_cmd_id: &mut u64,    verbose: bool,
) {
    let robot_pos = robot.last_update.position;
    
    // Wait for firmware to report MovingToDrop (after pickup timer expires)
    if matches!(robot.last_update.state, RobotState::MovingToDrop) {
        robot.task_stage = TaskStage::MovingToDropoff;
        robot.mark_progress();
        
        // Decrement shelf inventory on confirmed pickup
        if let Some(pickup_grid) = robot.pickup_grid {
            if inventory.pickup(pickup_grid) {
                if verbose {
                    let stock = inventory.stock_at(pickup_grid);
                    println!("[{}ms] 📦 Shelf ({},{}) stock decremented: {:?}",
                        timestamp(), pickup_grid.0, pickup_grid.1, stock);
                }
                logs::save_log("Coordinator", &format!(
                    "Task {} pickup: shelf ({},{}) stock now {:?}",
                    task_id, pickup_grid.0, pickup_grid.1, inventory.stock_at(pickup_grid)
                ));
            }
        }
        
        if verbose {
            println!("[{}ms] ✓ Robot {} loaded cargo for task {}", timestamp(), robot_id, task_id);
        }
        
        // Calculate path to dropoff
        if let Some(dropoff_world) = robot.dropoff_location {
            let start = pathfinding::world_to_grid(robot_pos);
            let dropoff_grid = pathfinding::world_to_grid(dropoff_world);
            
            if let Some(result) = pathfinder.find_path_to_non_walkable_for_robot(map, start, dropoff_grid, robot_id) {
                if verbose {
                    println!("[{}ms] 🚚 Robot {} path to dropoff: {} waypoints (cost: {})", 
                        timestamp(), robot_id, result.world_path.len(), result.cost);
                }
                
                if let Some(last_wp) = result.world_path.last() {
                    robot.dropoff_location = Some(*last_wp);
                }
                
                // Clear old reservations and reserve new path to dropoff
                pathfinder.clear_robot_reservations(robot_id);
                pathfinder.reserve_path(robot_id, &result.grid_path, robot.last_update.velocity);

                robot.set_path(result.world_path);
                
                if let Some(first_wp) = robot.next_waypoint() {
                    if is_near(robot_pos, first_wp) {
                        robot.advance_path();
                    }
                }
                
                if let Some(waypoint) = robot.next_waypoint() {
                    let cmd = PathCmd {
                        cmd_id: *next_cmd_id,
                        robot_id,
                        command: PathCommand::MoveToDropoff { target: waypoint, speed: coord_config::DEFAULT_SPEED },
                    };
                    *next_cmd_id += 1;
                    cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
                }
            }
        }
    }
}

/// Handle robot moving to dropoff
async fn handle_moving_to_dropoff(
    robot: &mut TrackedRobot,
    robot_id: u32,
    task_id: u64,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
) {
    let robot_pos = robot.last_update.position;
    
    if robot.path_complete() {
        if let Some(dropoff_world) = robot.dropoff_location {
            if is_near(robot_pos, dropoff_world) {
                robot.task_stage = TaskStage::Delivering;
                robot.mark_progress();
                
                if verbose {
                    println!("[{}ms] 📍 Robot {} arrived at dropoff for task {}", timestamp(), robot_id, task_id);
                }
                
                // Send Drop command
                let cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id,
                    command: PathCommand::Drop,
                };
                *next_cmd_id += 1;
                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();

                if verbose {
                    println!(
                        "[{}ms] 📦 Robot {} delivering cargo ({}s delay)...",
                        timestamp(),
                        robot_id,
                        coord_config::DROPOFF_DELAY_SECS
                    );
                }
            }
        }
    }
}

/// Handle robot delivering cargo (wait for firmware to confirm drop)
async fn handle_delivering(
    robot: &mut TrackedRobot,
    robot_id: u32,
    task_id: u64,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    inventory: &mut ShelfInventory,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
    pending_tasks: usize,
) {
    // Firmware transitions to Idle after Drop is applied
    if !matches!(robot.last_update.state, RobotState::Idle) {
        return;
    }

    // Increment shelf inventory on confirmed delivery (if target is a shelf)
    if let Some(dropoff_grid) = robot.dropoff_grid {
        if inventory.dropoff(dropoff_grid) {
            if verbose {
                let stock = inventory.stock_at(dropoff_grid);
                println!("[{}ms] 📦 Shelf ({},{}) stock incremented: {:?}",
                    timestamp(), dropoff_grid.0, dropoff_grid.1, stock);
            }
            logs::save_log("Coordinator", &format!(
                "Task {} delivery: shelf ({},{}) stock now {:?}",
                task_id, dropoff_grid.0, dropoff_grid.1, inventory.stock_at(dropoff_grid)
            ));
        }
    }

    // Task completed!
    println!("[{}ms] ✓ Task {} completed by Robot {}", timestamp(), task_id, robot_id);
    logs::save_log("Coordinator", &format!("Task {} completed by Robot {}", task_id, robot_id));

    // Clear this robot's reservations from WHCA* multi-robot table
    pathfinder.clear_robot_reservations(robot_id);

    // Send completion status
    let update = TaskStatusUpdate {
        task_id,
        status: TaskStatus::Completed,
        robot_id: Some(robot_id),
    };
    if let Ok(payload) = to_vec(&update) {
        status_publisher.put(payload).await.ok();
    }

    // Clear task state
    robot.current_task = None;
    robot.pickup_location = None;
    robot.dropoff_location = None;
    robot.pickup_grid = None;
    robot.dropoff_grid = None;
    robot.current_path.clear();
    robot.path_index = 0;
    robot.task_stage = TaskStage::Idle;
    robot.clear_wait();

    // Return to station if no pending tasks or battery is low
    let battery = robot.last_update.battery;
    let station_pos = robot.last_update.station_position;
    let should_return = pending_tasks == 0 || battery < battery_config::LOW_THRESHOLD;

    if should_return {
        let (reason_str, return_reason) = if battery < battery_config::LOW_THRESHOLD {
            (format!("low battery ({:.1}%)", battery), ReturnReason::LowBattery)
        } else {
            ("no pending tasks".to_string(), ReturnReason::NoPendingTasks)
        };

        let current_grid = pathfinding::world_to_grid(robot.last_update.position);
        let station_grid = pathfinding::world_to_grid(station_pos);

        if let Some(result) = pathfinder.find_path_for_robot(map, current_grid, station_grid, robot_id) {
            if verbose {
                println!("[{}ms] 🏠 Robot {} returning to station ({}) - {} waypoints", 
                    timestamp(), robot_id, reason_str, result.world_path.len());
            }
            logs::save_log("Coordinator", &format!(
                "Robot {} returning to station: {}", robot_id, reason_str
            ));

            // Reserve the return path immediately to avoid head-on conflicts
            pathfinder.reserve_path(robot_id, &result.grid_path, robot.last_update.velocity);

            robot.set_path(result.world_path);
            robot.task_stage = TaskStage::ReturningToStation;
            robot.return_reason = Some(return_reason);

            if let Some(waypoint) = robot.next_waypoint() {
                let cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id,
                    command: PathCommand::MoveTo { target: waypoint, speed: coord_config::DEFAULT_SPEED },
                };
                *next_cmd_id += 1;
                cmd_publisher.put(to_vec(&cmd).unwrap()).await.ok();
            }
        }
    } else {
        // Staying idle at dropoff: reserve the current cell immediately
        let current_grid = pathfinding::world_to_grid(robot.last_update.position);
        pathfinder.reserve_stationary(robot_id, current_grid);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_near() {
        assert!(is_near([1.0, 0.25, 1.0], [1.1, 0.25, 1.0]));
        assert!(!is_near([1.0, 0.25, 1.0], [2.0, 0.25, 1.0]));
    }

    #[test]
    fn test_validate_pickup_dropoff_shelf_to_dropoff() {
        let map_str = "x5 . v";
        let map = GridMap::parse(map_str).unwrap();
        assert!(validate_pickup_dropoff(&map, (0, 0), (2, 0)));
    }

    #[test]
    fn test_validate_pickup_dropoff_shelf_to_shelf() {
        let map_str = "x5 . x3";
        let map = GridMap::parse(map_str).unwrap();
        assert!(validate_pickup_dropoff(&map, (0, 0), (2, 0)));
    }

    #[test]
    fn test_validate_pickup_dropoff_ground_invalid() {
        let map_str = ". . .";
        let map = GridMap::parse(map_str).unwrap();
        assert!(!validate_pickup_dropoff(&map, (0, 0), (2, 0)));
    }
}
