//! Task Manager - Task assignment and progression logic
//!
//! This module handles:
//! - Task assignment from scheduler
//! - Task progression state machine
//! - Task timeout detection
//! - Return-to-station logic
//!
//! Extracted from server.rs for improved testability and maintainability.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};
use protocol::*;
use protocol::config::coordinator as coord_config;
use protocol::config::coordinator::collision as collision_config;
use protocol::config::firmware::battery as battery_config;
use protocol::grid_map::ShelfInventory;
use protocol::logs;

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
    /// Robot is disabled
    RobotDisabled,
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

fn inventory_milestone_for_stage(task_stage: TaskStage) -> InventoryMilestone {
    match task_stage {
        TaskStage::Idle | TaskStage::MovingToPickup | TaskStage::Picking => {
            InventoryMilestone::Reserved
        }
        TaskStage::MovingToDropoff | TaskStage::Delivering | TaskStage::ReturningToStation => {
            InventoryMilestone::PickupConfirmed
        }
    }
}

fn is_interruptible_return_reason(reason: Option<ReturnReason>) -> bool {
    matches!(reason, Some(ReturnReason::PostDelivery))
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
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            format!("Robot {} not found", robot_id),
            Some(InventoryMilestone::Reserved),
        )
        .await;
        return AssignmentResult::RobotNotFound;
    };

    // Reject assignments for faulted/blocked robots
    if matches!(robot.last_update.state, RobotState::Faulted | RobotState::Blocked) {
        println!("✗ Task {} rejected: Robot {} is {:?}", task.id, robot_id, robot.last_update.state);
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            format!("Robot {:?}", robot.last_update.state),
            Some(InventoryMilestone::Reserved),
        )
        .await;
        return AssignmentResult::RobotFaultedOrBlocked;
    }

    // Reject assignments for disabled robots.
    if !robot.last_update.enabled {
        println!("✗ Task {} rejected: Robot {} is disabled", task.id, robot_id);
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            "Robot disabled".to_string(),
            Some(InventoryMilestone::Reserved),
        )
        .await;
        return AssignmentResult::RobotDisabled;
    }
    
    // If robot is returning to station for an interruptible reason, allow reassignment.
    if robot.task_stage == TaskStage::ReturningToStation && is_interruptible_return_reason(robot.return_reason) {
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
            send_task_failure(
                status_publisher,
                task.id,
                robot_id,
                format!("Robot returning to station (low battery: {:.1}%)", battery),
                Some(InventoryMilestone::Reserved),
            )
            .await;
            return AssignmentResult::LowBatteryReturn { battery };
        }
    }

    // Reject assignments if robot is mid-task (except returning/charging with enough battery)
    let battery = robot.last_update.battery;
    let eligible_for_new_task = matches!(robot.task_stage, TaskStage::Idle)
        || (robot.task_stage == TaskStage::ReturningToStation
            && is_interruptible_return_reason(robot.return_reason))
        || (matches!(robot.last_update.state, RobotState::Charging)
            && battery >= battery_config::MIN_BATTERY_FOR_TASK);
    if !eligible_for_new_task {
        println!("✗ Task {} rejected: Robot {} busy ({:?})", task.id, robot_id, robot.task_stage);
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            format!("Robot busy ({:?})", robot.task_stage),
            Some(InventoryMilestone::Reserved),
        )
        .await;
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
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            "Invalid pickup/dropoff combination".to_string(),
            Some(InventoryMilestone::Reserved),
        )
        .await;
        return AssignmentResult::InvalidTileCombination;
    }

    // Shelf capacity enforcement: verify pickup has stock and dropoff has room
    if !inventory.can_pickup(pickup) {
        let stock = inventory.stock_at(pickup);
        let reason = format!("pickup shelf ({},{}) empty ({:?})", pickup.0, pickup.1, stock);
        println!("✗ Task {} rejected: {}", task.id, reason);
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            reason.clone(),
            Some(InventoryMilestone::Reserved),
        )
        .await;
        return AssignmentResult::ShelfCapacity { reason };
    }
    if !inventory.can_dropoff(dropoff_grid) {
        let stock = inventory.stock_at(dropoff_grid);
        let reason = format!("dropoff shelf ({},{}) full ({:?})", dropoff_grid.0, dropoff_grid.1, stock);
        println!("✗ Task {} rejected: {}", task.id, reason);
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            reason.clone(),
            Some(InventoryMilestone::Reserved),
        )
        .await;
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
        send_task_failure(
            status_publisher,
            task.id,
            robot_id,
            format!("no path to pickup ({},{})", pickup.0, pickup.1),
            Some(InventoryMilestone::Reserved),
        )
        .await;
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
    robot.reset_delivery_tracking();
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
        inventory_milestone: Some(InventoryMilestone::Reserved),
    };
    let _ = protocol::publish_json_logged(
        "Coordinator",
        "task in-progress status",
        &update,
        |payload| async move { status_publisher.put(payload).await.map(|_| ()) },
    )
    .await;
    
    // Send full path to pickup immediately (FollowPath - firmware follows all waypoints
    // without stopping, eliminating the per-tile pause from coordinator round-trips)
    let full_path = robot.current_path.clone();
    if !full_path.is_empty() {
        let cmd = PathCmd {
            cmd_id: *next_cmd_id,
            robot_id,
            command: PathCommand::FollowPath {
                waypoints: full_path,
                speed: coord_config::DEFAULT_SPEED,
            },
        };
        *next_cmd_id += 1;
        if protocol::publish_json_logged(
            "Coordinator",
            "task assignment follow path",
            &cmd,
            |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
        )
        .await
        {
            robot.path_sent = true;
        }
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

fn tile_type_at(map: &GridMap, pos: GridPos) -> Option<grid_map::TileType> {
    map.get_tile(pos.0, pos.1).map(|tile| tile.tile_type)
}

fn is_special_tile(map: &GridMap, pos: GridPos) -> bool {
    matches!(
        tile_type_at(map, pos),
        Some(grid_map::TileType::Dropoff | grid_map::TileType::Station | grid_map::TileType::Shelf(_))
    )
}

fn is_non_special_ground_tile(map: &GridMap, pos: GridPos) -> bool {
    matches!(tile_type_at(map, pos), Some(grid_map::TileType::Ground))
}

fn find_nearest_non_special_ground_tile(map: &GridMap, start: GridPos) -> Option<GridPos> {
    if is_non_special_ground_tile(map, start) {
        return Some(start);
    }

    let mut visited: HashSet<GridPos> = HashSet::new();
    let mut queue: VecDeque<GridPos> = VecDeque::new();

    visited.insert(start);
    queue.push_back(start);

    let directions: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in directions {
            let next_x = x as isize + dx;
            let next_y = y as isize + dy;

            if next_x < 0 || next_y < 0 {
                continue;
            }

            let next = (next_x as usize, next_y as usize);
            if next.0 >= map.width || next.1 >= map.height {
                continue;
            }

            if !visited.insert(next) || !map.is_walkable(next.0, next.1) {
                continue;
            }

            if is_non_special_ground_tile(map, next) {
                return Some(next);
            }

            queue.push_back(next);
        }
    }

    None
}

async fn dispatch_return_to_station_path(
    robot: &mut TrackedRobot,
    robot_id: u32,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    publish_label: &'static str,
) -> bool {
    let return_path = robot.current_path[robot.path_index..].to_vec();
    if return_path.is_empty() {
        return false;
    }

    let cmd = PathCmd {
        cmd_id: *next_cmd_id,
        robot_id,
        command: PathCommand::ReturnToStation {
            waypoints: return_path,
            speed: coord_config::DEFAULT_SPEED,
        },
    };
    *next_cmd_id += 1;
    if protocol::publish_json_logged(
        "Coordinator",
        publish_label,
        &cmd,
        |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
    )
    .await
    {
        robot.path_sent = true;
        return true;
    }

    false
}

async fn try_dispatch_staging_path(
    robot: &mut TrackedRobot,
    robot_id: u32,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
    context: &str,
) -> bool {
    let current_grid = pathfinding::world_to_grid(robot.last_update.position);
    if !is_special_tile(map, current_grid) {
        return false;
    }

    let Some(staging_grid) = find_nearest_non_special_ground_tile(map, current_grid) else {
        return false;
    };

    if staging_grid == current_grid {
        return false;
    }

    if pathfinder.is_reserved_now(staging_grid, Some(robot_id))
        || pathfinder.is_reserved_soon(
            staging_grid,
            coord_config::whca::MOVE_TIME_MS,
            Some(robot_id),
        )
    {
        return false;
    }

    let Some(result) = pathfinder.find_path_for_robot(map, current_grid, staging_grid, robot_id) else {
        return false;
    };

    pathfinder.clear_robot_reservations(robot_id);
    pathfinder.reserve_path(robot_id, &result.grid_path, robot.last_update.velocity);
    robot.set_path(result.world_path);
    robot.clear_wait();

    let dispatched = dispatch_return_to_station_path(
        robot,
        robot_id,
        cmd_publisher,
        next_cmd_id,
        "staging return path",
    )
    .await;

    if dispatched {
        robot.mark_progress();
        if verbose {
            println!(
                "[{}ms] 🧭 Robot {} staged to ground tile ({},{}) while {}",
                timestamp(),
                robot_id,
                staging_grid.0,
                staging_grid.1,
                context
            );
        }
        logs::save_log(
            "Coordinator",
            &format!(
                "Robot {} staged to ground tile ({},{}) while {}",
                robot_id, staging_grid.0, staging_grid.1, context
            ),
        );
    }

    dispatched
}

/// Send a task failure status update
pub async fn send_task_failure(
    publisher: &zenoh::pubsub::Publisher<'_>,
    task_id: u64,
    robot_id: u32,
    reason: String,
    inventory_milestone: Option<InventoryMilestone>,
) {
    let update = TaskStatusUpdate {
        task_id,
        status: TaskStatus::Failed { reason },
        robot_id: Some(robot_id),
        inventory_milestone,
    };
    let _ = protocol::publish_json_logged(
        "Coordinator",
        "task failure status",
        &update,
        |payload| async move { publisher.put(payload).await.map(|_| ()) },
    )
    .await;
}

// ============================================================================
// Task Progression
// ============================================================================

/// Check if robot position is near a target (within arrival threshold)
fn is_near(pos: [f32; 3], target: [f32; 3]) -> bool {
    let dist = ((pos[0] - target[0]).powi(2) + (pos[2] - target[2]).powi(2)).sqrt();
    dist < coord_config::WAYPOINT_ARRIVAL_THRESHOLD
}

fn should_reserve_stationary(robot: &TrackedRobot) -> bool {
    matches!(robot.task_stage, TaskStage::Idle | TaskStage::Picking | TaskStage::Delivering)
        || matches!(robot.last_update.state, RobotState::Faulted | RobotState::Blocked)
}

fn stationary_history_signature(robot: &TrackedRobot, current_pos: GridPos) -> u64 {
    let mut hasher = DefaultHasher::new();
    robot.recent_positions.len().hash(&mut hasher);
    for &pos in &robot.recent_positions {
        pos.hash(&mut hasher);
    }
    current_pos.hash(&mut hasher);
    hasher.finish()
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
    handle_fault_cleanup(robots, robot_control_publisher, pathfinder, verbose).await;

    // Blocked handling: immediate replan or fault escalation
    handle_blocked_robots(robots, map, pathfinder, cmd_publisher, status_publisher, next_cmd_id, verbose).await;
    
    // Second pass: refresh stationary reservations when state/tile/history changed or interval elapsed.
    let now = Instant::now();
    let refresh_interval = Duration::from_millis(coord_config::whca::STATIONARY_REFRESH_INTERVAL_MS);
    for (robot_id, robot) in robots.iter_mut() {
        if !should_reserve_stationary(robot) {
            continue;
        }

        let robot_pos = pathfinding::world_to_grid(robot.last_update.position);
        let history_sig = stationary_history_signature(robot, robot_pos);
        if !robot.stationary_refresh_due(now, refresh_interval, robot_pos, history_sig) {
            continue;
        }

        pathfinder.reserve_stationary_history(*robot_id, &robot.recent_positions, robot_pos);
        robot.record_stationary_reservation_refresh(now, robot_pos, history_sig);
    }
    
    // collect (robot_id, new_path) pairs for each successful replan so we can stop
    // conflicting robots in the fourth pass below
    let mut replanned_paths: Vec<(u32, Vec<[f32; 3]>)> = Vec::new();

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
                    replanned_paths.push((*robot_id, robot.current_path.clone()));
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
                replanned_paths.push((*robot_id, robot.current_path.clone()));
                continue;
            }
        }

        // Handle returning to station (no task required)
        if robot.task_stage == TaskStage::ReturningToStation {
            handle_returning_to_station(
                robot,
                *robot_id,
                map,
                pathfinder,
                cmd_publisher,
                next_cmd_id,
                verbose,
            )
            .await;
            continue;
        }
        
        // Only process robots with active tasks
        let Some(task_id) = robot.current_task else {
            // Idle robot with low battery should return to station
            handle_idle_low_battery(
                robot,
                *robot_id,
                map,
                pathfinder,
                cmd_publisher,
                next_cmd_id,
                verbose,
            )
            .await;
            continue;
        };
        
        match robot.task_stage {
            TaskStage::MovingToPickup => {
                handle_moving_to_pickup(robot, *robot_id, task_id, cmd_publisher, next_cmd_id, verbose).await;
            }
            TaskStage::Picking => {
                handle_picking(
                    robot,
                    *robot_id,
                    task_id,
                    map,
                    pathfinder,
                    inventory,
                    cmd_publisher,
                    status_publisher,
                    next_cmd_id,
                    verbose,
                )
                .await;
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

    // Fourth pass: stop robots whose next waypoint conflicts with a newly replanned path.
    // this eliminates the ~100 ms blind-spot where a robot would enter a cell just
    // reserved by another robot's fresh FollowPath command.
    for (replanned_id, new_path) in &replanned_paths {
        let new_cells: std::collections::HashSet<(usize, usize)> = new_path
            .iter()
            .map(|p| pathfinding::world_to_grid(*p))
            .collect();
        for (robot_id, robot) in robots.iter_mut() {
            if *robot_id == *replanned_id || !robot.path_sent {
                continue;
            }
            if let Some(wp) = robot.next_waypoint() {
                if new_cells.contains(&pathfinding::world_to_grid(wp)) {
                    let stop_cmd = PathCmd {
                        cmd_id: *next_cmd_id,
                        robot_id: *robot_id,
                        command: PathCommand::Stop,
                    };
                    *next_cmd_id += 1;
                    if protocol::publish_json_logged(
                        "Coordinator",
                        "replan conflict stop",
                        &stop_cmd,
                        |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                    )
                    .await
                    {
                        robot.path_sent = false;
                    }
                    if verbose {
                        println!("[{}ms] Robot {} stopped: next waypoint conflicts with replanned path of robot {}", timestamp(), robot_id, replanned_id);
                    }
                }
            }
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
                inventory_milestone: Some(inventory_milestone_for_stage(robot.task_stage)),
            };
            let _ = protocol::publish_json_logged(
                "Coordinator",
                "task timeout status",
                &update,
                |payload| async move { status_publisher.put(payload).await.map(|_| ()) },
            )
            .await;
            
            // Clear robot state
            robot.current_task = None;
            robot.task_stage = TaskStage::Idle;
            robot.task_started = None;
            robot.pickup_location = None;
            robot.dropoff_location = None;
            robot.pickup_grid = None;
            robot.dropoff_grid = None;
            robot.return_reason = None;
            robot.path_sent = false;
            robot.clear_wait();
            robot.reset_delivery_tracking();
            
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
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
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

    let failure_milestone = inventory_milestone_for_stage(robot.task_stage);
    if let Some(task_id) = robot.current_task {
        send_task_failure(
            status_publisher,
            task_id,
            robot_id,
            reason,
            Some(failure_milestone),
        )
        .await;
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
    robot.path_sent = false;
    robot.replan_attempts = 0;
    robot.blocked_since = None;
    robot.clear_wait();
    robot.reset_delivery_tracking();
    robot.faulted_since = Some(std::time::Instant::now());

    // Update local state to reflect fault
    robot.last_update.state = RobotState::Faulted;

    // Tell firmware to stop and set Faulted state so it broadcasts via Zenoh.
    let fault_cmd = PathCmd {
        cmd_id: *next_cmd_id,
        robot_id,
        command: PathCommand::Fault,
    };
    *next_cmd_id += 1;
    let _ = protocol::publish_json_logged(
        "Coordinator",
        "robot fault command",
        &fault_cmd,
        |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
    )
    .await;

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
                    cmd_publisher,
                    status_publisher,
                    next_cmd_id,
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
                    cmd_publisher,
                    status_publisher,
                    next_cmd_id,
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
    pathfinder: &mut PathfinderInstance,
    verbose: bool,
) {
    for (robot_id, robot) in robots.iter_mut() {
        let Some(since) = robot.faulted_since else { continue; };
        if since.elapsed().as_secs() >= collision_config::FAULT_CLEANUP_DELAY_SECS {
            // clear stale WHCA* reservations before sending restart
            pathfinder.clear_robot_reservations(*robot_id);
            let cmd = RobotControl::Restart(*robot_id);
            let _ = protocol::publish_json_logged(
                "Coordinator",
                "robot restart control",
                &cmd,
                |payload| async move { robot_control_publisher.put(payload).await.map(|_| ()) },
            )
            .await;
            if verbose {
                println!("[{}ms] 🔄 Robot {} restart after fault cleanup", timestamp(), robot_id);
            }
            logs::save_log("Coordinator", &format!("Robot {} restart after fault cleanup", robot_id));
            robot.faulted_since = None;
            robot.blocked_since = None;
            robot.replan_attempts = 0;
            robot.task_stage = TaskStage::Idle;
            robot.last_update.state = RobotState::Idle;
            robot.return_reason = None;
            robot.path_sent = false;
            robot.reset_delivery_tracking();
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
    let (task_id_for_log, target_world, use_return_command) = match robot.task_stage {
        TaskStage::MovingToPickup => {
            let Some(task_id) = robot.current_task else { return false; };
            let Some(target_world) = robot.pickup_location else { return false; };
            (Some(task_id), target_world, false)
        }
        TaskStage::MovingToDropoff | TaskStage::Delivering => {
            let Some(task_id) = robot.current_task else { return false; };
            let Some(target_world) = robot.dropoff_location else { return false; };
            (Some(task_id), target_world, false)
        }
        TaskStage::ReturningToStation => (None, robot.last_update.station_position, true),
        _ => return false,
    };
    let start = pathfinding::world_to_grid(robot.last_update.position);
    let goal = pathfinding::world_to_grid(target_world);

    let Some(result) = pathfinder.find_path_for_robot(map, start, goal, robot_id) else { return false; };

    // Clear old reservations and reserve new path
    pathfinder.clear_robot_reservations(robot_id);
    pathfinder.reserve_path(robot_id, &result.grid_path, robot.last_update.velocity);

    robot.set_path(result.world_path);
    robot.mark_progress();

    if verbose {
        if let Some(task_id) = task_id_for_log {
            println!(
                "[{}ms] 🔁 Robot {} replanned (task {}) - {} waypoints",
                timestamp(),
                robot_id,
                task_id,
                robot.current_path.len()
            );
        } else {
            println!(
                "[{}ms] 🔁 Robot {} replanned (return-to-station) - {} waypoints",
                timestamp(),
                robot_id,
                robot.current_path.len()
            );
        }
    }

    // Send full replanned path immediately so robot corrects course without waiting
    // for the next send_path_commands watchdog tick
    let remaining = robot.current_path[robot.path_index..].to_vec();
    if !remaining.is_empty() {
        let command = if use_return_command {
            PathCommand::ReturnToStation {
                waypoints: remaining,
                speed: coord_config::DEFAULT_SPEED,
            }
        } else {
            PathCommand::FollowPath {
                waypoints: remaining,
                speed: coord_config::DEFAULT_SPEED,
            }
        };
        let cmd = PathCmd {
            cmd_id: *next_cmd_id,
            robot_id,
            command,
        };
        *next_cmd_id += 1;
        if protocol::publish_json_logged(
            "Coordinator",
            "replanned follow path",
            &cmd,
            |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
        )
        .await
        {
            robot.path_sent = true;
        }
    }

    true
}

/// Handle robot returning to station
async fn handle_returning_to_station(
    robot: &mut TrackedRobot,
    robot_id: u32,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
) {
    let robot_pos = robot.last_update.position;
    let station_pos = robot.last_update.station_position;

    if is_near(robot_pos, station_pos) {
        let station_grid = pathfinding::world_to_grid(station_pos);
        if pathfinder.is_reserved_now(station_grid, Some(robot_id)) {
            // Station is currently owned by another robot reservation.
            let staged = try_dispatch_staging_path(
                robot,
                robot_id,
                map,
                pathfinder,
                cmd_publisher,
                next_cmd_id,
                verbose,
                "station occupied",
            )
            .await;

            if !staged {
                let current_grid = pathfinding::world_to_grid(robot_pos);
                pathfinder.reserve_stationary(robot_id, current_grid);
                if verbose {
                    println!("[{}ms] ⏸ Robot {} waiting: station occupied", timestamp(), robot_id);
                }
                logs::save_log("Coordinator", &format!("Robot {} waiting outside occupied station", robot_id));
            }
            return;
        }

        if verbose {
            println!("[{}ms] 🔋 Robot {} arrived at station, charging", timestamp(), robot_id);
        }

        let cmd = PathCmd {
            cmd_id: *next_cmd_id,
            robot_id,
            command: PathCommand::ReturnToCharge,
        };
        *next_cmd_id += 1;
        let _ = protocol::publish_json_logged(
            "Coordinator",
            "return to charge command",
            &cmd,
            |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
        )
        .await;

        robot.current_path.clear();
        robot.path_index = 0;

        // Only transition to Idle here — the robot has physically arrived.
        let battery = robot.last_update.battery;
        if battery >= battery_config::MIN_BATTERY_FOR_TASK {
            if verbose {
                println!(
                    "[{}ms] ✅ Robot {} charged to {:.1}%, available for tasks",
                    timestamp(),
                    robot_id,
                    battery
                );
            }
            logs::save_log("Coordinator", &format!("Robot {} ready: battery {:.1}%", robot_id, battery));
            robot.task_stage = TaskStage::Idle;
            robot.return_reason = None;
        }
        return;
    }

    // If no active return path, retry path planning to station.
    if robot.path_complete() {
        let current_grid = pathfinding::world_to_grid(robot_pos);
        let station_grid = pathfinding::world_to_grid(station_pos);

        if pathfinder.is_reserved_now(station_grid, Some(robot_id))
            || pathfinder.is_reserved_soon(
                station_grid,
                coord_config::whca::MOVE_TIME_MS,
                Some(robot_id),
            )
        {
            let staged = try_dispatch_staging_path(
                robot,
                robot_id,
                map,
                pathfinder,
                cmd_publisher,
                next_cmd_id,
                verbose,
                "waiting for station availability while returning",
            )
            .await;

            if !staged {
                pathfinder.reserve_stationary(robot_id, current_grid);
                robot.set_wait(station_grid);
                if verbose {
                    println!(
                        "[{}ms] ⏸ Robot {} waiting for station availability while returning",
                        timestamp(),
                        robot_id
                    );
                }
            }
            return;
        }

        if let Some(result) = pathfinder.find_path_for_robot(map, current_grid, station_grid, robot_id) {
            pathfinder.clear_robot_reservations(robot_id);
            pathfinder.reserve_path(robot_id, &result.grid_path, robot.last_update.velocity);
            robot.set_path(result.world_path);
            robot.clear_wait();
            if dispatch_return_to_station_path(
                robot,
                robot_id,
                cmd_publisher,
                next_cmd_id,
                "retry return-to-station path",
            )
            .await
                && verbose
            {
                println!(
                    "[{}ms] 🧭 Robot {} retried return path: {} waypoints",
                    timestamp(),
                    robot_id,
                    robot.current_path.len()
                );
            }
        } else {
            let staged = try_dispatch_staging_path(
                robot,
                robot_id,
                map,
                pathfinder,
                cmd_publisher,
                next_cmd_id,
                verbose,
                "return-to-station path unavailable",
            )
            .await;

            if !staged {
                pathfinder.reserve_stationary(robot_id, current_grid);
                robot.set_wait(station_grid);
                logs::save_log(
                    "Coordinator",
                    &format!(
                        "Robot {} return-to-station path not found yet; holding position",
                        robot_id
                    ),
                );
            }
        }
    }
}

/// Handle idle robot with low battery
async fn handle_idle_low_battery(
    robot: &mut TrackedRobot,
    robot_id: u32,
    map: &GridMap,
    pathfinder: &mut PathfinderInstance,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,    next_cmd_id: &mut u64,    verbose: bool,
) {
    let robot_pos = robot.last_update.position;
    let battery = robot.last_update.battery;
    let station_pos = robot.last_update.station_position;
    
    if battery < battery_config::LOW_THRESHOLD && !is_near(robot_pos, station_pos) {
        let current_grid = pathfinding::world_to_grid(robot_pos);
        let station_grid = pathfinding::world_to_grid(station_pos);

        if pathfinder.is_reserved_now(station_grid, Some(robot_id))
            || pathfinder.is_reserved_soon(
                station_grid,
                coord_config::whca::MOVE_TIME_MS,
                Some(robot_id),
            )
        {
            pathfinder.reserve_stationary(robot_id, current_grid);
            if verbose {
                println!("[{}ms] ⏸ Robot {} waiting for station availability", timestamp(), robot_id);
            }
            return;
        }
        
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

            let return_path = robot.current_path.clone();
            if !return_path.is_empty() {
                let cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id,
                    command: PathCommand::ReturnToStation {
                        waypoints: return_path,
                        speed: coord_config::DEFAULT_SPEED,
                    },
                };
                *next_cmd_id += 1;
                if protocol::publish_json_logged(
                    "Coordinator",
                    "idle low battery return path",
                    &cmd,
                    |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                )
                .await
                {
                    robot.path_sent = true;
                }
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
                let _ = protocol::publish_json_logged(
                    "Coordinator",
                    "pickup command",
                    &cmd,
                    |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                )
                .await;
                
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
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    verbose: bool,
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

        let milestone_update = TaskStatusUpdate {
            task_id,
            status: TaskStatus::InProgress { robot_id },
            robot_id: Some(robot_id),
            inventory_milestone: Some(InventoryMilestone::PickupConfirmed),
        };
        let _ = protocol::publish_json_logged(
            "Coordinator",
            "task pickup confirmed status",
            &milestone_update,
            |payload| async move { status_publisher.put(payload).await.map(|_| ()) },
        )
        .await;
        
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

                // skip first waypoint if robot is already on top of it
                if let Some(first_wp) = robot.next_waypoint() {
                    if is_near(robot_pos, first_wp) {
                        robot.advance_path();
                    }
                }

                let dropoff_path = robot.current_path[robot.path_index..].to_vec();
                if !dropoff_path.is_empty() {
                    let cmd = PathCmd {
                        cmd_id: *next_cmd_id,
                        robot_id,
                        command: PathCommand::FollowPath {
                            waypoints: dropoff_path,
                            speed: coord_config::DEFAULT_SPEED,
                        },
                    };
                    *next_cmd_id += 1;
                    if protocol::publish_json_logged(
                        "Coordinator",
                        "dropoff follow path",
                        &cmd,
                        |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                    )
                    .await
                    {
                        robot.path_sent = true;
                    }
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
                robot.delivery_retry_attempts = 0;
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
                let _ = protocol::publish_json_logged(
                    "Coordinator",
                    "drop command",
                    &cmd,
                    |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                )
                .await;
                robot.mark_drop_command_sent();

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
    _pending_tasks: usize,
) {
    // Firmware transitions to Idle after Drop is applied. Keep retrying Drop if
    // the robot still reports cargo after the retry interval.
    if !matches!(robot.last_update.state, RobotState::Idle) {
        // Carrying cargo means drop was not applied yet. Retry with a bounded budget.
        if robot.last_update.carrying_cargo.is_some() {
            let retry_interval = Duration::from_secs(
                coord_config::DELIVERY_CONFIRM_RETRY_INTERVAL_SECS.max(1),
            );
            let retry_due = robot
                .last_drop_command_sent_at
                .map(|last_sent| last_sent.elapsed() >= retry_interval)
                .unwrap_or(true);

            if retry_due {
                if robot.delivery_retry_attempts >= coord_config::DELIVERY_CONFIRM_MAX_RETRIES {
                    let reason = format!(
                        "drop confirmation timeout after {} retries",
                        coord_config::DELIVERY_CONFIRM_MAX_RETRIES
                    );
                    println!(
                        "[{}ms] ✗ Task {} failed on Robot {}: {}",
                        timestamp(),
                        task_id,
                        robot_id,
                        reason
                    );
                    logs::save_log(
                        "Coordinator",
                        &format!("Task {} failed on Robot {}: {}", task_id, robot_id, reason),
                    );

                    send_task_failure(
                        status_publisher,
                        task_id,
                        robot_id,
                        reason,
                        Some(InventoryMilestone::PickupConfirmed),
                    )
                    .await;

                    pathfinder.clear_robot_reservations(robot_id);
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
                    robot.path_sent = false;
                    robot.clear_wait();
                    robot.reset_delivery_tracking();
                    robot.mark_progress();
                    return;
                }

                robot.delivery_retry_attempts += 1;
                let attempt = robot.delivery_retry_attempts;

                let cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id,
                    command: PathCommand::Drop,
                };
                *next_cmd_id += 1;
                robot.mark_drop_command_sent();
                let sent = protocol::publish_json_logged(
                    "Coordinator",
                    "drop retry command",
                    &cmd,
                    |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                )
                .await;

                if verbose {
                    println!(
                        "[{}ms] 🔁 Robot {} drop retry {}/{} for task {}",
                        timestamp(),
                        robot_id,
                        attempt,
                        coord_config::DELIVERY_CONFIRM_MAX_RETRIES,
                        task_id
                    );
                }
                logs::save_log(
                    "Coordinator",
                    &format!(
                        "Robot {} drop retry {}/{} for task {} (sent={})",
                        robot_id,
                        attempt,
                        coord_config::DELIVERY_CONFIRM_MAX_RETRIES,
                        task_id,
                        sent
                    ),
                );

                if sent {
                    robot.mark_progress();
                }
            }

            return;
        }

        // Drop command was likely accepted and unload is in progress.
        robot.mark_progress();
        return;
    }

    robot.reset_delivery_tracking();

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
        inventory_milestone: Some(InventoryMilestone::DropoffConfirmed),
    };
    let _ = protocol::publish_json_logged(
        "Coordinator",
        "task completed status",
        &update,
        |payload| async move { status_publisher.put(payload).await.map(|_| ()) },
    )
    .await;

    // Clear task state
    robot.current_task = None;
    robot.task_started = None;
    robot.pickup_location = None;
    robot.dropoff_location = None;
    robot.pickup_grid = None;
    robot.dropoff_grid = None;
    robot.return_reason = None;
    robot.current_path.clear();
    robot.path_index = 0;
    robot.path_sent = false;
    robot.clear_wait();
    robot.reset_delivery_tracking();

    // Always transition into return flow after delivery.
    let battery = robot.last_update.battery;
    let station_pos = robot.last_update.station_position;
    let (reason_str, return_reason) = if battery < battery_config::LOW_THRESHOLD {
        (format!("low battery ({:.1}%)", battery), ReturnReason::LowBattery)
    } else {
        (
            "post-delivery repositioning".to_string(),
            ReturnReason::PostDelivery,
        )
    };

    robot.task_stage = TaskStage::ReturningToStation;
    robot.return_reason = Some(return_reason);

    let current_grid = pathfinding::world_to_grid(robot.last_update.position);
    let station_grid = pathfinding::world_to_grid(station_pos);

    if pathfinder.is_reserved_now(station_grid, Some(robot_id))
        || pathfinder.is_reserved_soon(
            station_grid,
            coord_config::whca::MOVE_TIME_MS,
            Some(robot_id),
        )
    {
        let staged = try_dispatch_staging_path(
            robot,
            robot_id,
            map,
            pathfinder,
            cmd_publisher,
            next_cmd_id,
            verbose,
            "waiting for station occupancy to clear",
        )
        .await;

        if !staged {
            pathfinder.reserve_stationary(robot_id, current_grid);
            robot.set_wait(station_grid);
            if verbose {
                println!(
                    "[{}ms] ⏸ Robot {} waiting for station occupancy to clear",
                    timestamp(),
                    robot_id
                );
            }
        }
        return;
    }

    if let Some(result) = pathfinder.find_path_for_robot(map, current_grid, station_grid, robot_id) {
        if verbose {
            println!(
                "[{}ms] 🏠 Robot {} returning to station ({}) - {} waypoints",
                timestamp(),
                robot_id,
                reason_str,
                result.world_path.len()
            );
        }
        logs::save_log(
            "Coordinator",
            &format!("Robot {} returning to station: {}", robot_id, reason_str),
        );

        pathfinder.reserve_path(robot_id, &result.grid_path, robot.last_update.velocity);
        robot.set_path(result.world_path);
        robot.clear_wait();
        let _ = dispatch_return_to_station_path(
            robot,
            robot_id,
            cmd_publisher,
            next_cmd_id,
            "post-delivery return path",
        )
        .await;
    } else {
        let staged = try_dispatch_staging_path(
            robot,
            robot_id,
            map,
            pathfinder,
            cmd_publisher,
            next_cmd_id,
            verbose,
            "post-delivery return path unavailable",
        )
        .await;

        if !staged {
            pathfinder.reserve_stationary(robot_id, current_grid);
            robot.set_wait(station_grid);
            logs::save_log(
                "Coordinator",
                &format!(
                    "Robot {} post-delivery return path unavailable; will retry",
                    robot_id
                ),
            );
        }
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

    #[test]
    fn test_interruptible_return_reason_helper() {
        assert!(is_interruptible_return_reason(Some(ReturnReason::PostDelivery)));
        assert!(!is_interruptible_return_reason(Some(ReturnReason::LowBattery)));
        assert!(!is_interruptible_return_reason(None));
    }

    #[test]
    fn test_is_special_tile_detection() {
        let map = GridMap::parse("v _ x5 .").unwrap();
        assert!(is_special_tile(&map, (0, 0)));
        assert!(is_special_tile(&map, (1, 0)));
        assert!(is_special_tile(&map, (2, 0)));
        assert!(!is_special_tile(&map, (3, 0)));
    }

    #[test]
    fn test_find_nearest_non_special_ground_tile_from_dropoff() {
        let map = GridMap::parse(
            "v . #\n# . _\n# # .",
        )
        .unwrap();

        let nearest = find_nearest_non_special_ground_tile(&map, (0, 0));
        assert_eq!(nearest, Some((1, 0)));
    }

    #[test]
    fn test_find_nearest_non_special_ground_tile_none_when_absent() {
        let map = GridMap::parse("v _\n# #").unwrap();
        let nearest = find_nearest_non_special_ground_tile(&map, (0, 0));
        assert_eq!(nearest, None);
    }
}
