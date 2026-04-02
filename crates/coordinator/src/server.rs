//! Coordinator server main loop
//!
//! Handles task assignments, path planning, and robot command dispatch.

use zenoh::Session;
use tokio::time;
use tokio::sync::mpsc;
use protocol::*;
use protocol::config::coordinator as coord_config;
use protocol::config::coordinator::{collision as collision_config, sensor as sensor_config};
use protocol::logs;
use protocol::grid_map::ShelfInventory;
use serde_json::{to_vec, from_slice};
use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};

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
    pathfinder.reset_whca_stats();
    
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

    let robot_control_publisher = session
        .declare_publisher(topics::ROBOT_CONTROL)
        .await
        .expect("Failed to declare ROBOT_CONTROL publisher");

    let path_telemetry_publisher = session
        .declare_publisher(topics::TELEMETRY_PATHS)
        .await
        .expect("Failed to declare TELEMETRY_PATHS publisher");

    let whca_metrics_publisher = session
        .declare_publisher(topics::TELEMETRY_WHCA_METRICS)
        .await
        .expect("Failed to declare TELEMETRY_WHCA_METRICS publisher");
    
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
    let map_validation_payload = to_vec(&map_validation)
        .expect("Failed to serialize map validation payload");
    map_publisher
        .put(map_validation_payload)
        .await
        .expect("Failed to publish map validation");
    println!("✓ Map hash broadcast for validation");
    
    // State
    let mut robots: HashMap<u32, TrackedRobot> = HashMap::new();
    let mut paused = false;
    let mut verbose = true;
    let mut inventory = ShelfInventory::from_map(&map);
    let mut pending_tasks: usize = 0;  // From QueueState broadcasts
    let mut next_cmd_id: u64 = 1;  // Unique ID for command tracking
    let mut time_scale: f32 = 1.0;
    
    // Channel for stdin commands
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);
    
    println!("✓ Coordinator running. Waiting for robots...");
    
    let mut last_tick = std::time::Instant::now();
    let mut last_validation_publish = std::time::Instant::now();
    let mut last_whca_stats_log = std::time::Instant::now();
    let mut last_whca_stats_snapshot: Option<pathfinding::WHCAStatsSnapshot> = None;
    let mut chaos = protocol::config::chaos::ENABLED;
    let mut path_telemetry_cache: HashMap<u32, PathTelemetryCacheEntry> = HashMap::new();
    
    loop {
        // Republish map hash periodically (ensures latecomers can validate)
        if last_validation_publish.elapsed() >= std::time::Duration::from_secs(coord_config::MAP_HASH_REPUBLISH_SECS) {
            let _ = protocol::publish_json_logged(
                "Coordinator",
                "periodic map validation",
                &map_validation,
                |payload| async { map_publisher.put(payload).await.map(|_| ()) },
            )
            .await;
            last_validation_publish = std::time::Instant::now();
        }
        
        // Handle system commands (from orchestrator via Zenoh)
        while let Ok(Some(sample)) = control_subscriber.try_recv() {
            let payload = sample.payload().to_bytes();
            match from_slice::<SystemCommand>(&payload) {
                Ok(sys_cmd) => {
                    commands::handle_system_command(
                        &sys_cmd,
                        &mut paused,
                        &mut verbose,
                        &mut chaos,
                        &mut time_scale,
                    );
                }
                Err(err) => log_deserialize_failure("SystemCommand", topics::ADMIN_CONTROL, &payload, &err),
            }
        }
        
        // Handle task assignments (from scheduler)
        while let Ok(Some(sample)) = task_subscriber.try_recv() {
            let payload = sample.payload().to_bytes();
            match from_slice::<TaskAssignment>(&payload) {
                Ok(assignment) => {
                    let result = task_manager::handle_task_assignment(
                        &assignment,
                        &mut robots,
                        &map,
                        &mut pathfinder,
                        &mut inventory,
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
                        task_manager::AssignmentResult::RobotFaultedOrBlocked => "rejected: robot faulted/blocked".to_string(),
                        task_manager::AssignmentResult::RobotDisabled => "rejected: robot disabled".to_string(),
                        task_manager::AssignmentResult::RobotBusy => "rejected: robot busy".to_string(),
                        task_manager::AssignmentResult::NoPickupLocation => "rejected: no pickup location".to_string(),
                        task_manager::AssignmentResult::NoDropoffLocation => "rejected: no dropoff location".to_string(),
                        task_manager::AssignmentResult::InvalidTileCombination => "rejected: invalid pickup/dropoff".to_string(),
                        task_manager::AssignmentResult::ShelfCapacity { reason } => format!("rejected: {}", reason),
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
                Err(err) => log_deserialize_failure("TaskAssignment", topics::TASK_ASSIGNMENTS, &payload, &err),
            }
        }
        
        // Handle queue state updates (from scheduler)
        while let Ok(Some(sample)) = queue_subscriber.try_recv() {
            let payload = sample.payload().to_bytes();
            match from_slice::<QueueState>(&payload) {
                Ok(state) => {
                    pending_tasks = state.pending;
                }
                Err(err) => log_deserialize_failure("QueueState", topics::QUEUE_STATE, &payload, &err),
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
                        
                        if let Some(result) = pathfinder.find_path_for_robot(&map, start, goal, robot_id) {
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
            let payload = sample.payload().to_bytes();
            // Try batched format first (current standard)
            match from_slice::<RobotUpdateBatch>(&payload) {
                Ok(batch) => {
                    for update in batch.updates {
                        handle_robot_update(
                            &map,
                            &mut robots,
                            update,
                            Some(batch.tick),
                            time_scale,
                            &cmd_publisher,
                            &status_publisher,
                            &mut next_cmd_id,
                            &mut pathfinder,
                            verbose,
                        ).await;
                    }
                }
                // Fallback: legacy individual RobotUpdate
                Err(batch_err) => match from_slice::<RobotUpdate>(&payload) {
                    Ok(update) => {
                        handle_robot_update(
                            &map,
                            &mut robots,
                            update,
                            None,
                            time_scale,
                            &cmd_publisher,
                            &status_publisher,
                            &mut next_cmd_id,
                            &mut pathfinder,
                            verbose,
                        ).await;
                    }
                    Err(single_err) => {
                        log_deserialize_failure("RobotUpdateBatch", topics::ROBOT_UPDATES, &payload, &batch_err);
                        log_deserialize_failure("RobotUpdate", topics::ROBOT_UPDATES, &payload, &single_err);
                    }
                },
            }
        }
        
        // Detect inter-robot collisions after updates
        detect_inter_robot_collisions(&mut robots, &status_publisher, &cmd_publisher, &mut next_cmd_id, &mut pathfinder, verbose).await;

        // Task progression: detect state transitions and send next waypoint
        if !paused {
            // Advance WHCA* planning window for multi-robot collision avoidance
            pathfinder.tick();
            
            task_manager::progress_tasks(
                &mut robots,
                &map,
                &mut pathfinder,
                &mut inventory,
                &cmd_publisher,
                &status_publisher,
                &robot_control_publisher,
                &mut next_cmd_id,
                verbose,
                pending_tasks,
            ).await;
        }
        
        // Handle command responses from firmware
        handle_command_responses(&response_subscriber, verbose);
        
        // Server tick - send path commands at configured rate
        if last_tick.elapsed() >= std::time::Duration::from_millis(coord_config::PATH_SEND_INTERVAL_MS) {
            last_tick = std::time::Instant::now();
            
            if !paused {
                send_path_commands(&mut robots, &cmd_publisher, &mut next_cmd_id, &mut pathfinder, verbose).await;
            }

            // Broadcast remaining paths when changed, with heartbeat fallback.
            broadcast_path_telemetry(
                &robots,
                &path_telemetry_publisher,
                &mut path_telemetry_cache,
            )
            .await;
        }

        if last_whca_stats_log.elapsed() >= std::time::Duration::from_secs(5) {
            if let Some(current) = pathfinder.whca_stats_snapshot() {
                let window_secs = 5;
                let delta = if let Some(previous) = last_whca_stats_snapshot {
                    pathfinding::WHCAStatsSnapshot {
                        searches_total: current.searches_total.saturating_sub(previous.searches_total),
                        searches_succeeded: current.searches_succeeded.saturating_sub(previous.searches_succeeded),
                        searches_failed: current.searches_failed.saturating_sub(previous.searches_failed),
                        nodes_expanded_total: current.nodes_expanded_total.saturating_sub(previous.nodes_expanded_total),
                        reservation_probe_calls_total: current.reservation_probe_calls_total.saturating_sub(previous.reservation_probe_calls_total),
                        edge_collision_checks_total: current.edge_collision_checks_total.saturating_sub(previous.edge_collision_checks_total),
                        wait_actions_added_total: current.wait_actions_added_total.saturating_sub(previous.wait_actions_added_total),
                        open_set_peak_observed: current.open_set_peak_observed,
                        reservation_entries_peak: current.reservation_entries_peak,
                        total_search_time_us: current.total_search_time_us.saturating_sub(previous.total_search_time_us),
                        last_search_time_us: current.last_search_time_us,
                    }
                } else {
                    current
                };

                let searches = delta.searches_total;
                let avg_us = if searches > 0 {
                    delta.total_search_time_us / searches
                } else {
                    0
                };
                let success_pct = if searches > 0 {
                    (delta.searches_succeeded as f64 * 100.0) / searches as f64
                } else {
                    0.0
                };
                let msg = format!(
                    "WHCA metrics[5s]: searches={} success={:.1}% expanded={} reserve_probes={} edge_checks={} waits={} avg_us={} last_us={} open_peak={} reservations_peak={}",
                    searches,
                    success_pct,
                    delta.nodes_expanded_total,
                    delta.reservation_probe_calls_total,
                    delta.edge_collision_checks_total,
                    delta.wait_actions_added_total,
                    avg_us,
                    delta.last_search_time_us,
                    delta.open_set_peak_observed,
                    delta.reservation_entries_peak,
                );

                if verbose {
                    println!("{}", msg);
                }
                logs::save_log("Coordinator", &msg);

                let telemetry = WhcaMetricsTelemetry {
                    window_secs,
                    searches_total: delta.searches_total,
                    searches_succeeded: delta.searches_succeeded,
                    searches_failed: delta.searches_failed,
                    nodes_expanded_total: delta.nodes_expanded_total,
                    reservation_probe_calls_total: delta.reservation_probe_calls_total,
                    edge_collision_checks_total: delta.edge_collision_checks_total,
                    wait_actions_added_total: delta.wait_actions_added_total,
                    avg_search_time_us: avg_us,
                    last_search_time_us: delta.last_search_time_us,
                    open_set_peak_observed: delta.open_set_peak_observed,
                    reservation_entries_peak: delta.reservation_entries_peak,
                };

                let _ = protocol::publish_json_logged(
                    "Coordinator",
                    "whca metrics telemetry",
                    &telemetry,
                    |payload| async { whca_metrics_publisher.put(payload).await.map(|_| ()) },
                )
                .await;

                last_whca_stats_snapshot = Some(current);
            }
            last_whca_stats_log = std::time::Instant::now();
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
        let payload = sample.payload().to_bytes();
        match from_slice::<CommandResponse>(&payload) {
            Ok(response) => {
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
            Err(err) => log_deserialize_failure("CommandResponse", topics::COMMAND_RESPONSES, &payload, &err),
        }
    }
}

// ============================================================================
// Path Command Helpers
// ============================================================================

/// Send path commands for all robots with active paths.
///
/// With lookahead batching, the coordinator sends a single FollowPath
/// containing all remaining waypoints. The firmware advances through them
/// internally without stopping. This function's 100 ms tick now serves as:
///  - a watchdog (re-sends if path_sent is cleared by replan/set_path)
///  - a path_index sync (advances coordinator's position tracking for
///    deviation detection and telemetry)
///  - a reservation checker (sends Stop if next cell is reserved, then
///    re-sends FollowPath from the current position once the cell clears)
async fn send_path_commands(
    robots: &mut HashMap<u32, TrackedRobot>,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    pathfinder: &mut PathfinderInstance,
    _verbose: bool,
) {
    for (robot_id, robot) in robots.iter_mut() {
        // pause during pickup/dropoff actions
        if matches!(robot.last_update.state, RobotState::Picking) {
            continue;
        }

        // skip if no path to follow
        if robot.path_complete() {
            continue;
        }

        // scan the next LOOKAHEAD_BLOCK_SCAN_CELLS waypoints for reservations.
        // checking only next_waypoint() gave the coordinator one tick to react before
        // the firmware crossed into a reserved cell; scanning ahead stops the robot
        // earlier, giving WHCA* time to find an alternate route.
        let Some(next_wp) = robot.next_waypoint() else {
            logs::save_log(
                "Coordinator",
                &format!(
                    "Robot {} has incomplete path but no next waypoint; resetting send state",
                    robot_id
                ),
            );
            robot.path_sent = false;
            continue;
        };
        let target_grid = pathfinding::world_to_grid(next_wp);
        let is_blocked = robot.current_path[robot.path_index..]
            .iter()
            .take(coord_config::LOOKAHEAD_BLOCK_SCAN_CELLS)
            .enumerate()
            .any(|(step_idx, &wp)| {
                let g = pathfinding::world_to_grid(wp);
                let offset_ms = coord_config::whca::MOVE_TIME_MS
                    .saturating_mul((step_idx as u64).saturating_add(1));
                let reserved_future = pathfinder.is_reserved_soon(g, offset_ms, Some(*robot_id));
                let reserved_now = step_idx == 0 && pathfinder.is_reserved_now(g, Some(*robot_id));
                reserved_future || reserved_now
            });

        if is_blocked {
            robot.set_wait(target_grid);

            // If blocked for a long time, report it but keep holding position.
            // forcing a resume into known reservations caused avoidable collisions.
            if let Some(wait_secs) = robot.wait_elapsed_secs() {
                if wait_secs >= collision_config::RESERVATION_WAIT_OVERRIDE_SECS {
                    logs::save_log(
                        "Coordinator",
                        &format!(
                            "Robot {} still blocked after {}s; maintaining stop until reservation clears",
                            robot_id, wait_secs
                        ),
                    );
                }
            }

            if robot.path_sent {
                // still blocked and path already sent - stop firmware and wait
                let stop_cmd = PathCmd {
                    cmd_id: *next_cmd_id,
                    robot_id: *robot_id,
                    command: PathCommand::Stop,
                };
                *next_cmd_id += 1;
                let _ = protocol::publish_json_logged(
                    "Coordinator",
                    "reservation stop command",
                    &stop_cmd,
                    |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                )
                .await;
                robot.path_sent = false; // will resend FollowPath once unblocked
            }

            let current_grid = pathfinding::world_to_grid(robot.last_update.position);
            pathfinder.reserve_stationary(*robot_id, current_grid);
            robot.mark_progress();
            continue;
        } else {
            robot.clear_wait();
        }

        // send path command if not yet dispatched for this path segment.
        // use ReturnToStation when the robot is heading home so firmware sets
        // MovingToStation (not MovingToPickup which FollowPath infers from no cargo).
        if !robot.path_sent {
            let remaining: Vec<[f32; 3]> = robot.current_path[robot.path_index..].to_vec();
            if !remaining.is_empty() {
                let command = if robot.task_stage == crate::state::TaskStage::ReturningToStation {
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
                    robot_id: *robot_id,
                    command,
                };
                *next_cmd_id += 1;
                if protocol::publish_json_logged(
                    "Coordinator",
                    "path follow command",
                    &cmd,
                    |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
                )
                .await
                {
                    robot.path_sent = true;
                }
            }
        }

        // Refresh rolling reservations for long paths so windowed WHCA* protection
        // does not expire on the untraversed path tail.
        let remaining_grid: Vec<(usize, usize)> = robot.current_path[robot.path_index..]
            .iter()
            .map(|wp| pathfinding::world_to_grid(*wp))
            .collect();
        if remaining_grid.len() > 1 {
            pathfinder.reserve_path(*robot_id, &remaining_grid, robot.last_update.velocity);
        }
    }
}

// ============================================================================
// Path Telemetry
// ============================================================================

/// Broadcast remaining waypoints for all tracked robots.
/// Sends immediately on path change and periodically as a heartbeat.
#[derive(Debug, Clone)]
struct PathTelemetryCacheEntry {
    signature: u64,
    last_sent: std::time::Instant,
}

fn remaining_path_slice(robot: &TrackedRobot) -> &[[f32; 3]] {
    if robot.path_index < robot.current_path.len() {
        &robot.current_path[robot.path_index..]
    } else {
        &[]
    }
}

fn path_signature(waypoints: &[[f32; 3]]) -> u64 {
    let mut hasher = DefaultHasher::new();
    waypoints.len().hash(&mut hasher);
    for [x, y, z] in waypoints {
        x.to_bits().hash(&mut hasher);
        y.to_bits().hash(&mut hasher);
        z.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn should_send_path_telemetry(
    previous: Option<&PathTelemetryCacheEntry>,
    signature: u64,
    now: std::time::Instant,
    heartbeat: std::time::Duration,
) -> bool {
    let Some(previous) = previous else {
        return true;
    };

    previous.signature != signature || now.duration_since(previous.last_sent) >= heartbeat
}

async fn broadcast_path_telemetry(
    robots: &HashMap<u32, TrackedRobot>,
    publisher: &zenoh::pubsub::Publisher<'_>,
    cache: &mut HashMap<u32, PathTelemetryCacheEntry>,
) {
    use protocol::RobotPathTelemetry;
    let now = std::time::Instant::now();
    let heartbeat = std::time::Duration::from_millis(coord_config::PATH_TELEMETRY_HEARTBEAT_MS);

    cache.retain(|robot_id, _| robots.contains_key(robot_id));

    for (&robot_id, robot) in robots.iter() {
        let remaining = remaining_path_slice(robot);
        let signature = path_signature(remaining);

        if !should_send_path_telemetry(cache.get(&robot_id), signature, now, heartbeat) {
            continue;
        }

        let waypoints = remaining.to_vec();

        let telemetry = RobotPathTelemetry { robot_id, waypoints };
        if protocol::publish_json_logged(
            "Coordinator",
            "path telemetry broadcast",
            &telemetry,
            |payload| async move { publisher.put(payload).await.map(|_| ()) },
        )
        .await
        {
            cache.insert(
                robot_id,
                PathTelemetryCacheEntry {
                    signature,
                    last_sent: now,
                },
            );
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

/// Handle a single robot update with validation
async fn handle_robot_update(
    map: &GridMap,
    robots: &mut HashMap<u32, TrackedRobot>,
    update: RobotUpdate,
    tick: Option<u64>,
    time_scale: f32,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    pathfinder: &mut PathfinderInstance,
    verbose: bool,
) {
    let robot = robots.entry(update.id).or_insert_with(|| {
        println!("+ Robot {} connected", update.id);
        TrackedRobot::new(update.clone())
    });

    let prev_update = robot.last_update.clone();

    // Detect firmware restart: Faulted/Blocked → Idle means the firmware
    // teleported the robot back to its station. Clean up stale coordinator
    // state so other robots are not permanently blocked by ghost reservations.
    let was_faulted = matches!(prev_update.state, RobotState::Faulted | RobotState::Blocked);
    let now_idle = matches!(update.state, RobotState::Idle);
    if was_faulted && now_idle {
        pathfinder.clear_robot_reservations(update.id);
        robot.current_task = None;
        robot.task_stage = TaskStage::Idle;
        robot.current_path = Vec::new();
        robot.path_index = 0;
        robot.blocked_since = None;
        robot.faulted_since = None;
        robot.replan_attempts = 0;
        robot.waiting_since = None;
        robot.waiting_for = None;
        // skip position validation: teleport after restart is expected
        robot.skip_next_validation = true;
        println!("[Coordinator] Robot {} recovered ({:?} → Idle) — reservations cleared", update.id, prev_update.state);
        logs::save_log("Coordinator", &format!("Robot {} recovered from {:?}, reservations cleared", update.id, prev_update.state));
    }

    let validation = if robot.skip_next_validation {
        robot.skip_next_validation = false;
        Ok(())
    } else {
        validate_robot_update(map, &prev_update, &update, robot.last_tick, tick, time_scale)
    };

    robot.last_update = update;
    robot.last_tick = tick;

    // Auto-unassign policy: if a robot is disabled while carrying an active assignment,
    // fail that task immediately so scheduler can requeue it.
    if !robot.last_update.enabled {
        if let Some(task_id) = robot.current_task {
            logs::save_log(
                "Coordinator",
                &format!(
                    "Robot {} disabled with active task {}; auto-unassigning",
                    robot.last_update.id,
                    task_id
                ),
            );

            let inventory_milestone = match robot.task_stage {
                TaskStage::Idle | TaskStage::MovingToPickup | TaskStage::Picking => {
                    Some(InventoryMilestone::Reserved)
                }
                TaskStage::MovingToDropoff
                | TaskStage::Delivering
                | TaskStage::ReturningToStation => Some(InventoryMilestone::PickupConfirmed),
            };

            task_manager::send_task_failure(
                status_publisher,
                task_id,
                robot.last_update.id,
                "Robot disabled (auto-unassign)".to_string(),
                inventory_milestone,
            )
            .await;

            pathfinder.clear_robot_reservations(robot.last_update.id);
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
            robot.mark_progress();
        }
    }

    let grid_pos = pathfinding::world_to_grid(robot.last_update.position);
    if robot.recent_positions.back().copied() != Some(grid_pos) {
        robot.recent_positions.push_back(grid_pos);
        let max_len = coord_config::whca::STATIONARY_HISTORY_TILES.max(1);
        while robot.recent_positions.len() > max_len {
            robot.recent_positions.pop_front();
        }
    }

    // advance path_index at firmware rate (20 Hz) so broadcast_path_telemetry
    // and deviation detection stay accurate; gated behind path_sent so a stopped
    // robot never overruns its index and triggers spurious replans
    if robot.path_sent && !robot.path_complete() {
        let pos = robot.last_update.position;
        while let Some(wp) = robot.next_waypoint() {
            let dist = ((pos[0] - wp[0]).powi(2) + (pos[2] - wp[2]).powi(2)).sqrt();
            if dist < coord_config::WAYPOINT_ARRIVAL_THRESHOLD {
                robot.advance_path();
                robot.mark_progress();
            } else {
                break;
            }
        }
    }

    if let Err(reason) = validation {
        task_manager::mark_robot_faulted(
            robot,
            robot.last_update.id,
            reason,
            cmd_publisher,
            status_publisher,
            next_cmd_id,
            pathfinder,
            verbose,
        ).await;
    }
}

/// Validate incoming robot update against map and sensor thresholds
fn validate_robot_update(
    map: &GridMap,
    prev: &RobotUpdate,
    update: &RobotUpdate,
    prev_tick: Option<u64>,
    current_tick: Option<u64>,
    time_scale: f32,
) -> Result<(), String> {
    if update.position[0].is_nan() || update.position[2].is_nan() {
        return Err("Invalid position: NaN detected".to_string());
    }

    // Teleport / anomaly detection
    let dx = update.position[0] - prev.position[0];
    let dz = update.position[2] - prev.position[2];
    let dist = (dx * dx + dz * dz).sqrt();
    let max_delta = if let (Some(prev_tick), Some(current_tick)) = (prev_tick, current_tick) {
        let tick_delta = current_tick.saturating_sub(prev_tick).max(1);
        let dt_secs = tick_delta as f32
            * (protocol::config::firmware::physics::TICK_INTERVAL_MS as f32 / 1000.0)
            * time_scale.clamp(0.1, 1000.0);
        (coord_config::DEFAULT_SPEED * dt_secs) + sensor_config::MAX_POSITION_DELTA
    } else {
        sensor_config::MAX_POSITION_DELTA
    };
    let soft_delta = max_delta * sensor_config::POSITION_JUMP_SOFT_LIMIT_MULT;
    if dist > soft_delta {
        return Err(format!("Position jump {:.2} > {:.2}", dist, soft_delta));
    }

    // Map validation
    let grid = pathfinding::world_to_grid(update.position);
    if !map.is_walkable(grid.0, grid.1) {
        return Err(format!("Non-walkable tile at ({}, {})", grid.0, grid.1));
    }

    // Grid alignment tolerance
    let center = pathfinding::grid_to_world(grid);
    let cx = update.position[0] - center[0];
    let cz = update.position[2] - center[2];
    let offset = (cx * cx + cz * cz).sqrt();
    if offset > sensor_config::GRID_VALIDATION_SOFT_LIMIT {
        return Err(format!("Off-grid position offset {:.2} > {:.2}", offset, sensor_config::GRID_VALIDATION_SOFT_LIMIT));
    }

    Ok(())
}

/// Build collision candidate pairs using spatial buckets.
///
/// Robots can only collide with robots in the same or adjacent bucket when the
/// bucket size equals the collision radius.
fn collect_collision_candidate_pairs(
    robots: &HashMap<u32, TrackedRobot>,
    collision_radius: f32,
) -> Vec<(u32, u32)> {
    let mut ids: Vec<u32> = robots
        .iter()
        .filter_map(|(id, robot)| (robot.faulted_since.is_none()).then_some(*id))
        .collect();

    if ids.len() < 2 {
        return Vec::new();
    }

    if collision_radius <= f32::EPSILON {
        ids.sort_unstable();
        let mut pairs = Vec::new();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                pairs.push((ids[i], ids[j]));
            }
        }
        return pairs;
    }

    const NEIGHBOR_OFFSETS: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

    let mut buckets: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    for id in ids {
        let Some(robot) = robots.get(&id) else { continue; };
        let x = (robot.last_update.position[0] / collision_radius).floor() as i32;
        let z = (robot.last_update.position[2] / collision_radius).floor() as i32;
        buckets.entry((x, z)).or_default().push(id);
    }

    let mut pairs = Vec::new();
    for (&bucket, bucket_ids) in &buckets {
        // intra-bucket candidates
        for i in 0..bucket_ids.len() {
            for j in (i + 1)..bucket_ids.len() {
                let a = bucket_ids[i];
                let b = bucket_ids[j];
                pairs.push((a.min(b), a.max(b)));
            }
        }

        // adjacent-bucket candidates (single-direction offsets avoid duplicates)
        for (dx, dz) in NEIGHBOR_OFFSETS {
            let neighbor_bucket = (bucket.0 + dx, bucket.1 + dz);
            let Some(neighbor_ids) = buckets.get(&neighbor_bucket) else {
                continue;
            };

            for &a in bucket_ids {
                for &b in neighbor_ids {
                    if a == b {
                        continue;
                    }
                    pairs.push((a.min(b), a.max(b)));
                }
            }
        }
    }

    pairs
}

/// Detect inter-robot collisions, fault offenders, and immediately stop any other
/// robot whose remaining path overlaps the collision zone.
///
/// Three-pass structure to avoid borrow conflicts:
/// 1. Collect pairs to fault (read-only pass)
/// 2. Fault and collect their grid positions
/// 3. Stop other robots routed through the faulted cells
async fn detect_inter_robot_collisions(
    robots: &mut HashMap<u32, TrackedRobot>,
    status_publisher: &zenoh::pubsub::Publisher<'_>,
    cmd_publisher: &zenoh::pubsub::Publisher<'_>,
    next_cmd_id: &mut u64,
    pathfinder: &mut PathfinderInstance,
    verbose: bool,
) {
    // pass 1: determine which robots need to be faulted (read-only)
    let mut to_fault: Vec<(u32, String)> = Vec::new();
    let candidate_pairs = collect_collision_candidate_pairs(robots, collision_config::ROBOT_COLLISION_RADIUS);
    for (id_a, id_b) in candidate_pairs {
        let (Some(robot_a), Some(robot_b)) = (robots.get(&id_a), robots.get(&id_b)) else { continue; };

        // skip already-faulted robots to avoid re-faulting in subsequent ticks
        if robot_a.faulted_since.is_some() || robot_b.faulted_since.is_some() {
            continue;
        }

        let dx = robot_a.last_update.position[0] - robot_b.last_update.position[0];
        let dz = robot_a.last_update.position[2] - robot_b.last_update.position[2];
        let dist = (dx * dx + dz * dz).sqrt();

        if dist > collision_config::ROBOT_COLLISION_RADIUS {
            continue;
        }

        let speed_a = (robot_a.last_update.velocity[0].powi(2) + robot_a.last_update.velocity[2].powi(2)).sqrt();
        let speed_b = (robot_b.last_update.velocity[0].powi(2) + robot_b.last_update.velocity[2].powi(2)).sqrt();

        let head_on = speed_a > 0.1 && speed_b > 0.1 &&
            (robot_a.last_update.velocity[0] * robot_b.last_update.velocity[0] +
                robot_a.last_update.velocity[2] * robot_b.last_update.velocity[2]) < 0.0;

        if head_on {
            to_fault.push((id_a, "Head-on collision".to_string()));
            to_fault.push((id_b, "Head-on collision".to_string()));
        } else if speed_a > speed_b {
            to_fault.push((id_a, format!("Collision with robot {}", id_b)));
        } else if speed_b > speed_a {
            to_fault.push((id_b, format!("Collision with robot {}", id_a)));
        } else {
            to_fault.push((id_a, format!("Collision with robot {}", id_b)));
            to_fault.push((id_b, format!("Collision with robot {}", id_a)));
        }
    }

    if to_fault.is_empty() {
        return;
    }

    // pass 2: fault the identified robots, collect their grid positions so we
    // can stop other robots routing through the same cells
    let mut faulted_cells: HashSet<(usize, usize)> = HashSet::new();
    let faulted_ids: HashSet<u32> = to_fault.iter().map(|(id, _)| *id).collect();
    for (robot_id, _) in &to_fault {
        if let Some(robot) = robots.get(robot_id) {
            faulted_cells.insert(pathfinding::world_to_grid(robot.last_update.position));
        }
    }
    for (robot_id, reason) in to_fault {
        if let Some(robot) = robots.get_mut(&robot_id) {
            task_manager::mark_robot_faulted(robot, robot_id, reason, cmd_publisher, status_publisher, next_cmd_id, pathfinder, verbose).await;
        }
    }

    // pass 3: stop any robot whose remaining path passes through a faulted cell.
    // when a collision clears reservations, other robots' dispatched FollowPaths
    // may route through those now-unowned cells toward the restarting robots.
    // stopping them forces a replan via the send_path_commands watchdog.
    for (robot_id, robot) in robots.iter_mut() {
        if faulted_ids.contains(robot_id) || !robot.path_sent {
            continue;
        }
        let path_intersects = robot.current_path[robot.path_index..]
            .iter()
            .any(|&wp| faulted_cells.contains(&pathfinding::world_to_grid(wp)));
        if path_intersects {
            let stop_cmd = PathCmd {
                cmd_id: *next_cmd_id,
                robot_id: *robot_id,
                command: PathCommand::Stop,
            };
            *next_cmd_id += 1;
            if protocol::publish_json_logged(
                "Coordinator",
                "collision stop command",
                &stop_cmd,
                |payload| async move { cmd_publisher.put(payload).await.map(|_| ()) },
            )
            .await
            {
                robot.path_sent = false;
            }
            if verbose {
                println!("[{}ms] Robot {} stopped: path routes through collision zone", timestamp(), robot_id);
            }
        }
    }
}

fn log_deserialize_failure(expected_type: &str, topic: &str, payload: &[u8], err: &serde_json::Error) {
    let preview = payload
        .iter()
        .take(8)
        .map(|byte| format!("{:02x}", byte))
        .collect::<Vec<_>>()
        .join("");
    logs::save_log(
        "Coordinator",
        &format!(
            "Malformed {} on {} ({} bytes, preview={}): {}",
            expected_type,
            topic,
            payload.len(),
            preview,
            err
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{RobotState, RobotUpdate};
    use std::time::Duration;

    fn tracked_robot(id: u32, x: f32, z: f32) -> TrackedRobot {
        TrackedRobot::new(RobotUpdate {
            id,
            position: [x, 0.25, z],
            velocity: [0.0, 0.0, 0.0],
            state: RobotState::Idle,
            battery: 100.0,
            carrying_cargo: None,
            station_position: [0.0, 0.25, 0.0],
            enabled: true,
        })
    }

    fn pair_set(pairs: Vec<(u32, u32)>) -> HashSet<(u32, u32)> {
        pairs.into_iter().collect()
    }

    #[test]
    fn collision_candidates_limit_far_pairs() {
        let mut robots = HashMap::new();
        robots.insert(1, tracked_robot(1, 0.00, 0.00));
        robots.insert(2, tracked_robot(2, 0.20, 0.10));
        robots.insert(3, tracked_robot(3, 20.0, 20.0));

        let pairs = pair_set(collect_collision_candidate_pairs(&robots, 0.4));
        assert!(pairs.contains(&(1, 2)));
        assert!(!pairs.contains(&(1, 3)));
        assert!(!pairs.contains(&(2, 3)));
    }

    #[test]
    fn collision_candidates_fallback_when_radius_is_zero() {
        let mut robots = HashMap::new();
        robots.insert(10, tracked_robot(10, 0.0, 0.0));
        robots.insert(20, tracked_robot(20, 50.0, 50.0));
        robots.insert(30, tracked_robot(30, -8.0, -8.0));

        let pairs = pair_set(collect_collision_candidate_pairs(&robots, 0.0));
        assert_eq!(pairs.len(), 3);
        assert!(pairs.contains(&(10, 20)));
        assert!(pairs.contains(&(10, 30)));
        assert!(pairs.contains(&(20, 30)));
    }

    #[test]
    fn path_signature_changes_when_waypoints_change() {
        let path_a = vec![[1.0, 0.25, 2.0], [2.0, 0.25, 2.0]];
        let path_b = vec![[1.0, 0.25, 2.0], [2.0, 0.25, 3.0]];

        let sig_a = path_signature(&path_a);
        let sig_b = path_signature(&path_b);

        assert_eq!(sig_a, path_signature(&path_a));
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn telemetry_send_policy_honors_change_and_heartbeat() {
        let now = std::time::Instant::now();
        let heartbeat = Duration::from_millis(1000);
        let signature = 42_u64;

        assert!(should_send_path_telemetry(None, signature, now, heartbeat));

        let previous = PathTelemetryCacheEntry {
            signature,
            last_sent: now,
        };

        assert!(!should_send_path_telemetry(
            Some(&previous),
            signature,
            now + Duration::from_millis(500),
            heartbeat,
        ));

        assert!(should_send_path_telemetry(
            Some(&previous),
            signature,
            now + Duration::from_millis(1000),
            heartbeat,
        ));

        assert!(should_send_path_telemetry(
            Some(&previous),
            signature + 1,
            now + Duration::from_millis(100),
            heartbeat,
        ));
    }
}