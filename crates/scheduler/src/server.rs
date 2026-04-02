//! Scheduler server loop

use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time;
use zenoh::Session;
use serde_json::from_slice;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rand::Rng;

use protocol::config::{coordinator as coord_config, scheduler as sched_config};
use protocol::{
    timestamp, logs, topics, GridMap, Priority, QueueState, RobotUpdateBatch, ShelfInventory,
    Task, TaskAssignment, TaskCommand, TaskId, TaskListSnapshot, TaskStatus, TaskStatusUpdate,
    TaskType,
    is_reachable_on_map, world_to_grid,
};

use crate::allocator::{Allocator, AllocatorInstance, RobotInfo};
use crate::cli::{print_status, print_shelves, print_dropoffs, print_stations, print_map, print_history, spawn_stdin_reader, print_help, StdinCmd};
use crate::commands::handle_system_commands;
use crate::queue::{QueueInstance, TaskQueue};

#[derive(Debug, Clone)]
struct TaskRetryState {
    attempts: u32,
    next_eligible_at: std::time::Instant,
}

fn is_disabled_failure_reason(reason: &str) -> bool {
    reason.to_ascii_lowercase().contains("disabled")
}

fn is_retryable_no_path_failure(reason: &str) -> bool {
    reason.to_ascii_lowercase().contains("no path to pickup")
}

fn base_retry_backoff_ms(attempt: u32) -> u64 {
    let exponent = attempt.saturating_sub(1).min(16);
    let base = sched_config::RETRYABLE_NO_PATH_BASE_BACKOFF_MS
        .saturating_mul(1_u64 << exponent);
    base.min(sched_config::RETRYABLE_NO_PATH_MAX_BACKOFF_MS)
}

fn retry_backoff_with_jitter_ms(attempt: u32) -> u64 {
    let base = base_retry_backoff_ms(attempt);
    let jitter_max = sched_config::RETRYABLE_NO_PATH_JITTER_MS;
    if jitter_max == 0 {
        return base;
    }
    let jitter = thread_rng().gen_range(0..=jitter_max);
    base.saturating_add(jitter)
}

fn unix_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn prune_retry_state(queue: &dyn TaskQueue, retry_state: &mut HashMap<TaskId, TaskRetryState>) {
    retry_state.retain(|task_id, _| {
        queue.get(*task_id)
            .map(|task| task.status == TaskStatus::Pending)
            .unwrap_or(false)
    });
}

fn whca_pickup_horizon_steps() -> usize {
    let move_time_ms = coord_config::whca::MOVE_TIME_MS.max(1);
    (coord_config::whca::WINDOW_SIZE_MS / move_time_ms) as usize
}

fn is_within_pickup_horizon(start: (usize, usize), pickup: (usize, usize)) -> bool {
    let manhattan_steps = start.0.abs_diff(pickup.0) + start.1.abs_diff(pickup.1);
    manhattan_steps <= whca_pickup_horizon_steps()
}

fn terminal_recency_ms(task: &Task) -> u64 {
    task.completed_at.unwrap_or(task.created_at)
}

fn oldest_recent_terminal_index(window: &[Task]) -> Option<usize> {
    window
        .iter()
        .enumerate()
        .min_by_key(|(_, task)| (terminal_recency_ms(task), task.id))
        .map(|(idx, _)| idx)
}

fn push_recent_terminal_task(window: &mut Vec<Task>, task: &Task, limit: usize) {
    if limit == 0 {
        return;
    }

    if window.len() < limit {
        window.push(task.clone());
        return;
    }

    let Some(oldest_idx) = oldest_recent_terminal_index(window) else {
        return;
    };

    let oldest = &window[oldest_idx];
    let candidate_key = (terminal_recency_ms(task), task.id);
    let oldest_key = (terminal_recency_ms(oldest), oldest.id);

    if candidate_key > oldest_key {
        window[oldest_idx] = task.clone();
    }
}

fn build_task_list_snapshot(queue: &dyn TaskQueue) -> TaskListSnapshot {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let active_limit = sched_config::TASK_LIST_ACTIVE_WINDOW;
    let recent_terminal_limit = sched_config::TASK_LIST_RECENT_TERMINAL_WINDOW;

    let mut active_tasks = Vec::with_capacity(active_limit.min(64));
    let mut recent_terminal_tasks = Vec::with_capacity(recent_terminal_limit.min(128));
    let mut active_total = 0usize;
    let mut completed_total = 0usize;
    let mut failed_total = 0usize;
    let mut cancelled_total = 0usize;

    for task in queue.all_tasks() {
        match task.status {
            TaskStatus::Pending | TaskStatus::Assigned { .. } | TaskStatus::InProgress { .. } => {
                active_total += 1;
                if active_tasks.len() < active_limit {
                    active_tasks.push(task.clone());
                }
            }
            TaskStatus::Completed => {
                completed_total += 1;
                push_recent_terminal_task(&mut recent_terminal_tasks, task, recent_terminal_limit);
            }
            TaskStatus::Failed { .. } => {
                failed_total += 1;
                push_recent_terminal_task(&mut recent_terminal_tasks, task, recent_terminal_limit);
            }
            TaskStatus::Cancelled => {
                cancelled_total += 1;
                push_recent_terminal_task(&mut recent_terminal_tasks, task, recent_terminal_limit);
            }
        }
    }

    recent_terminal_tasks.sort_by(|a, b| {
        terminal_recency_ms(b)
            .cmp(&terminal_recency_ms(a))
            .then_with(|| b.id.cmp(&a.id))
    });

    TaskListSnapshot {
        active_tasks,
        recent_terminal_tasks,
        active_total,
        completed_total,
        failed_total,
        cancelled_total,
        timestamp_ms,
    }
}

/// Run the scheduler main loop
pub async fn run(session: Session) {
    // Load warehouse map for location info
    let layout_path = protocol::layout::resolve_layout_path();
    let map = GridMap::load_from_file(&layout_path)
        .expect("Failed to load warehouse layout");
    println!("[{}ms] ✓ Loaded map from {} ({}x{}, {} shelves, {} dropoffs)",
        timestamp(),
        layout_path,
        map.width, map.height,
        map.get_shelves().len(),
        map.get_dropoffs().len(),
    );

    // Shelf inventory for capacity enforcement (shelves start full)
    let mut inventory = ShelfInventory::from_map(&map);

    // Publishers
    let assignment_pub = session.declare_publisher(topics::TASK_ASSIGNMENTS).await.unwrap();
    let queue_pub = session.declare_publisher(topics::QUEUE_STATE).await.unwrap();
    let task_list_pub = session.declare_publisher(topics::TASK_LIST).await.unwrap();

    // Subscribers
    let task_sub = session.declare_subscriber(topics::TASK_REQUESTS).await.unwrap();
    let robot_sub = session.declare_subscriber(topics::ROBOT_UPDATES).await.unwrap();
    let status_sub = session.declare_subscriber(topics::TASK_STATUS).await.unwrap();
    let control_sub = session.declare_subscriber(topics::ADMIN_CONTROL).await.unwrap();

    // State
    let mut queue = QueueInstance::from_config();
    let allocator = AllocatorInstance::from_config();
    let mut robots: HashMap<u32, RobotInfo> = HashMap::new();
    let mut retry_state: HashMap<TaskId, TaskRetryState> = HashMap::new();
    let mut paused = false;
    let mut verbose = true; // Default to verbose on

    // Stdin
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);

    println!("[{}ms] ✓ Scheduler running", timestamp());
    println!(
        "[{}ms] Commands: status, add <px> <py> <dx> <dy>, mass_add <count<= {}> [dropoff_%], help",
        timestamp(),
        sched_config::MASS_ADD_MAX_COUNT
    );
    println!("[{}ms] (System commands: run orchestrator)", timestamp());

    let mut last_broadcast = std::time::Instant::now();
    let mut chaos = protocol::config::chaos::ENABLED;

    loop {
        // System commands (from Zenoh - orchestrator)
        handle_system_commands(&control_sub, &mut paused, &mut verbose, &mut chaos);
        
        // Stdin commands (local CLI)
        handle_stdin(&mut rx, &mut queue, &robots, &map, paused, verbose).await;
        
        // Task requests (from other crates)
        handle_task_requests(&task_sub, &mut queue, &inventory, &map);
        
        // Robot updates (from firmware)
        handle_robot_updates(&robot_sub, &mut robots);
        
        // Task status updates (from coordinator)
        handle_status_updates(
            &status_sub,
            &mut queue,
            &mut robots,
            &mut inventory,
            &mut retry_state,
        );

        // Drop retry metadata for tasks that are no longer pending.
        prune_retry_state(&queue, &mut retry_state);

        // Allocate tasks
        if !paused {
            allocate_tasks(
                &mut queue,
                &allocator,
                &mut robots,
                &map,
                &mut inventory,
                &mut retry_state,
                &assignment_pub,
                verbose,
            )
            .await;
        }

        // Broadcast queue state and task list
        if last_broadcast.elapsed() >= std::time::Duration::from_secs(sched_config::QUEUE_BROADCAST_SECS) {
            broadcast_state(&queue_pub, &queue, &robots).await;
            broadcast_task_list(&task_list_pub, &queue).await;
            last_broadcast = std::time::Instant::now();
        }

        time::sleep(std::time::Duration::from_millis(sched_config::LOOP_INTERVAL_MS)).await;
    }
}

async fn handle_stdin(
    rx: &mut mpsc::Receiver<StdinCmd>,
    queue: &mut dyn TaskQueue,
    robots: &HashMap<u32, RobotInfo>,
    map: &GridMap,
    paused: bool,
    verbose: bool,
) {
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            StdinCmd::Status => print_status(queue, robots, map, paused, verbose),
            StdinCmd::AddTask { pickup, dropoff } => {
                // Resolve named locations (S1, D1, etc.)
                let resolved_pickup = resolve_location(pickup, map);
                let resolved_dropoff = resolve_location(dropoff, map);
                
                match (resolved_pickup, resolved_dropoff) {
                    (Some(p), Some(d)) => {
                        let id = queue.next_task_id();
                        let task = Task::new(id, TaskType::PickAndDeliver {
                            pickup: p, dropoff: d, cargo_id: None,
                        }, Priority::Normal);
                        println!("[{}ms] ✓ Task #{} added: ({},{}) → ({},{})", timestamp(), id, p.0, p.1, d.0, d.1);
                        logs::save_log("Scheduler", &format!("Task {} created: pickup ({},{}) -> dropoff ({},{})", id, p.0, p.1, d.0, d.1));
                        queue.enqueue(task);
                    }
                    (None, _) => println!("✗ Invalid pickup location"),
                    (_, None) => println!("✗ Invalid dropoff location"),
                }
            }
            StdinCmd::RandomTask => {
                let shelves = map.get_shelves();
                let dropoffs = map.get_dropoffs();
                if shelves.is_empty() || dropoffs.is_empty() {
                    println!("✗ Cannot create random task (shelves: {}, dropoffs: {})", shelves.len(), dropoffs.len());
                    continue;
                }

                let mut rng = thread_rng();
                let (Some(shelf), Some(dropoff)) = (
                    shelves.choose(&mut rng),
                    dropoffs.choose(&mut rng),
                ) else {
                    logs::save_log("Scheduler", "Random task creation aborted: no shelf/dropoff candidates");
                    continue;
                };

                let id = queue.next_task_id();
                let task = Task::new(id, TaskType::PickAndDeliver {
                    pickup: (shelf.x, shelf.y),
                    dropoff: (dropoff.x, dropoff.y),
                    cargo_id: None,
                }, Priority::Normal);
                println!(
                    "[{}ms] ✓ Task #{} added: ({},{}) → ({},{})",
                    timestamp(),
                    id,
                    shelf.x,
                    shelf.y,
                    dropoff.x,
                    dropoff.y
                );
                logs::save_log(
                    "Scheduler",
                    &format!(
                        "Task {} created: pickup ({},{}) -> dropoff ({},{})",
                        id,
                        shelf.x,
                        shelf.y,
                        dropoff.x,
                        dropoff.y
                    ),
                );
                queue.enqueue(task);
            }
            StdinCmd::MassAdd {
                count,
                dropoff_probability,
            } => {
                let requested = count;
                let count = clamp_mass_add_count(requested);
                let probability = effective_dropoff_probability(dropoff_probability);
                let (created, skipped) = enqueue_mass_add_tasks(queue, map, count, probability);

                if requested > count {
                    println!(
                        "[{}ms] ⚠ Mass-add capped from {} to {} tasks",
                        timestamp(),
                        requested,
                        count
                    );
                    logs::save_log(
                        "Scheduler",
                        &format!(
                            "Mass-add capped from {} to {} tasks via CLI",
                            requested,
                            count
                        ),
                    );
                }

                if created > 0 {
                    println!(
                        "[{}ms] ✓ Mass-add queued {} tasks (dropoff {:.1}%)",
                        timestamp(),
                        created,
                        probability * 100.0
                    );
                    logs::save_log(
                        "Scheduler",
                        &format!(
                            "Mass-add queued {} tasks via CLI (dropoff {:.1}%, requested {})",
                            created,
                            probability * 100.0,
                            requested
                        ),
                    );
                }

                if skipped > 0 {
                    println!(
                        "[{}ms] ⚠ Mass-add skipped {} tasks (insufficient destination candidates)",
                        timestamp(),
                        skipped
                    );
                }

                if created == 0 {
                    println!(
                        "[{}ms] ✗ Mass-add created no tasks (check map shelves/dropoffs)",
                        timestamp()
                    );
                }
            }
            StdinCmd::ListShelves => print_shelves(map),
            StdinCmd::ListDropoffs => print_dropoffs(map),
            StdinCmd::ListStations => print_stations(map),
            StdinCmd::Map => print_map(map, robots),
            StdinCmd::History => print_history(queue),
            StdinCmd::CancelTask { task_id } => {
                if let Some(task) = queue.get_mut(task_id) {
                    if matches!(task.status, TaskStatus::Pending) {
                        task.status = TaskStatus::Cancelled;
                        println!("✓ Task #{} cancelled", task_id);
                    } else {
                        println!("✗ Task #{} cannot be cancelled (status: {:?})", task_id, task.status);
                    }
                } else {
                    println!("✗ Task #{} not found", task_id);
                }
            }
            StdinCmd::SetPriority { task_id, priority } => {
                if let Some(task) = queue.get_mut(task_id) {
                    if matches!(task.status, TaskStatus::Pending) {
                        let old = task.priority;
                        task.priority = priority;
                        println!("✓ Task #{} priority: {:?} → {:?}", task_id, old, priority);
                    } else {
                        println!("✗ Task #{} already assigned/in-progress", task_id);
                    }
                } else {
                    println!("✗ Task #{} not found", task_id);
                }
            }
            StdinCmd::Help => print_help()
        }
    }
}

/// Resolve a location - either already coordinates or named (S1=10001, D1=20001, etc.)
fn resolve_location(loc: (usize, usize), map: &GridMap) -> Option<(usize, usize)> {
    let (x, y) = loc;
    
    // Check if it's a named location marker
    if x >= sched_config::SHELF_MARKER_BASE && x < sched_config::DROPOFF_MARKER_BASE {
        // Shelf: S1 = SHELF_MARKER_BASE + 1, so index = x - SHELF_MARKER_BASE
        let idx = x - sched_config::SHELF_MARKER_BASE;
        let shelves = map.get_shelves();
        if idx >= 1 && idx <= shelves.len() {
            let shelf = shelves[idx - 1];
            return Some((shelf.x, shelf.y));
        }
        println!("✗ Shelf S{} not found (valid: S1-S{})", idx, shelves.len());
        return None;
    } else if x >= sched_config::DROPOFF_MARKER_BASE {
        // Dropoff: D1 = DROPOFF_MARKER_BASE + 1, so index = x - DROPOFF_MARKER_BASE
        let idx = x - sched_config::DROPOFF_MARKER_BASE;
        let dropoffs = map.get_dropoffs();
        if idx >= 1 && idx <= dropoffs.len() {
            let dropoff = dropoffs[idx - 1];
            return Some((dropoff.x, dropoff.y));
        }
        println!("✗ Dropoff D{} not found (valid: D1-D{})", idx, dropoffs.len());
        return None;
    }
    
    // Regular coordinates - validate they exist on map
    if map.get_tile(x, y).is_some() {
        Some((x, y))
    } else {
        println!("✗ Coordinates ({},{}) not on map", x, y);
        None
    }
}

fn effective_dropoff_probability(dropoff_probability: Option<f32>) -> f32 {
    dropoff_probability
        .unwrap_or(sched_config::MASS_ADD_DROPOFF_PROBABILITY)
        .clamp(0.0, 1.0)
}

fn clamp_mass_add_count(requested: u32) -> u32 {
    requested.min(sched_config::MASS_ADD_MAX_COUNT)
}

fn enqueue_mass_add_tasks(
    queue: &mut dyn TaskQueue,
    map: &GridMap,
    count: u32,
    dropoff_probability: f32,
) -> (u32, u32) {
    let shelves = map.get_shelves();
    let dropoffs = map.get_dropoffs();

    if shelves.is_empty() {
        return (0, count);
    }

    let mut rng = thread_rng();
    let mut created = 0;
    let mut skipped = 0;

    for _ in 0..count {
        let Some(pickup_tile) = shelves.choose(&mut rng).copied() else {
            skipped += 1;
            continue;
        };
        let pickup = (pickup_tile.x, pickup_tile.y);

        let target = if !dropoffs.is_empty() && rng.gen_bool(dropoff_probability as f64) {
            dropoffs
                .choose(&mut rng)
                .copied()
                .map(|dropoff| (dropoff.x, dropoff.y))
        } else {
            shelves
                .choose(&mut rng)
                .copied()
                .filter(|candidate| (candidate.x, candidate.y) != pickup)
                .or_else(|| {
                    shelves
                        .iter()
                        .copied()
                        .find(|candidate| (candidate.x, candidate.y) != pickup)
                })
                .map(|candidate| (candidate.x, candidate.y))
        };

        let Some(dropoff) = target else {
            skipped += 1;
            continue;
        };

        let id = queue.next_task_id();
        let task = Task::new(
            id,
            TaskType::PickAndDeliver {
                pickup,
                dropoff,
                cargo_id: None,
            },
            Priority::Normal,
        );
        queue.enqueue(task);
        created += 1;
    }

    (created, skipped)
}

fn handle_task_requests(
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    queue: &mut dyn TaskQueue,
    inventory: &ShelfInventory,
    map: &GridMap,
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        let payload = sample.payload().to_bytes();
        if let Ok(cmd) = from_slice::<TaskCommand>(&payload) {
            match cmd {
                TaskCommand::New { task_type, priority } => {
                    // validate pickup shelf is not empty before queuing
                    let pickup = match &task_type {
                        TaskType::PickAndDeliver { pickup, .. } => Some(*pickup),
                        TaskType::Relocate { from, .. } => Some(*from),
                        TaskType::ReturnToStation { .. } => None,
                    };
                    if let Some(pos) = pickup {
                        if !inventory.can_pickup(pos) {
                            println!("[{}ms] ✗ Task rejected: pickup shelf ({},{}) is empty", timestamp(), pos.0, pos.1);
                            logs::save_log("Scheduler", &format!("Task rejected: pickup ({},{}) is empty", pos.0, pos.1));
                            continue;
                        }
                    }
                    let id = queue.next_task_id();
                    let task = Task::new(id, task_type, priority);
                    println!("[{}ms] + Task #{} created via UI", timestamp(), id);
                    logs::save_log("Scheduler", &format!("Task {} created via UI command", id));
                    queue.enqueue(task);
                }
                TaskCommand::Cancel(task_id) => {
                    if let Some(task) = queue.get_mut(task_id) {
                        if matches!(task.status, TaskStatus::Pending) {
                            task.status = TaskStatus::Cancelled;
                            println!("[{}ms] \u{2713} Task #{} cancelled via UI", timestamp(), task_id);
                            logs::save_log("Scheduler", &format!("Task {} cancelled via UI", task_id));
                        } else {
                            println!("[{}ms] \u{2717} Task #{} cannot cancel (status: {:?})", timestamp(), task_id, task.status);
                        }
                    } else {
                        println!("[{}ms] \u{2717} Cancel: task #{} not found", timestamp(), task_id);
                    }
                }
                TaskCommand::SetPriority(task_id, priority) => {
                    if let Some(task) = queue.get_mut(task_id) {
                        if matches!(task.status, TaskStatus::Pending) {
                            let old = task.priority;
                            task.priority = priority;
                            println!("[{}ms] \u{2713} Task #{} priority: {:?} \u{2192} {:?}", timestamp(), task_id, old, priority);
                            logs::save_log("Scheduler", &format!("Task {} priority changed to {:?}", task_id, priority));
                        }
                    }
                }
                TaskCommand::MassAdd {
                    count,
                    dropoff_probability,
                } => {
                    if count == 0 {
                        logs::save_log("Scheduler", "Mass-add command ignored: count=0");
                        continue;
                    }

                    let requested = count;
                    let count = clamp_mass_add_count(requested);
                    let probability = effective_dropoff_probability(dropoff_probability);
                    let (created, skipped) = enqueue_mass_add_tasks(queue, map, count, probability);

                    if requested > count {
                        logs::save_log(
                            "Scheduler",
                            &format!(
                                "Mass-add capped from {} to {} tasks via UI",
                                requested,
                                count
                            ),
                        );
                    }

                    if created > 0 {
                        println!(
                            "[{}ms] + Mass-add queued {} tasks via UI (dropoff {:.1}%)",
                            timestamp(),
                            created,
                            probability * 100.0
                        );
                        logs::save_log(
                            "Scheduler",
                            &format!(
                                "Mass-add queued {} tasks via UI (dropoff {:.1}%, requested {})",
                                created,
                                probability * 100.0,
                                requested
                            ),
                        );
                    }

                    if skipped > 0 {
                        logs::save_log(
                            "Scheduler",
                            &format!(
                                "Mass-add skipped {} tasks due to unavailable destination candidates",
                                skipped
                            ),
                        );
                    }
                }
            }
        } else {
            logs::save_log(
                "Scheduler",
                &format!(
                    "Malformed TaskCommand payload on {} ({} bytes)",
                    topics::TASK_REQUESTS,
                    payload.len()
                ),
            );
        }
    }
}

async fn broadcast_task_list(
    publisher: &zenoh::pubsub::Publisher<'_>,
    queue: &dyn TaskQueue,
) {
    let snapshot = build_task_list_snapshot(queue);
    let _ = protocol::publish_json_logged(
        "Scheduler",
        "task list window snapshot",
        &snapshot,
        |payload| async move { publisher.put(payload).await.map(|_| ()) },
    )
    .await;
}

fn handle_robot_updates(
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    robots: &mut HashMap<u32, RobotInfo>,
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        let payload = sample.payload().to_bytes();
        if let Ok(batch) = from_slice::<RobotUpdateBatch>(&payload) {
            for update in &batch.updates {
                let entry = robots.entry(update.id).or_insert_with(|| RobotInfo::from(update));
                entry.position = update.position;
                entry.state = update.state.clone();
                entry.enabled = update.enabled;
                entry.battery = update.battery;
                // NOTE: Do NOT touch assigned_task here!
                // It's managed by handle_status_updates based on TaskStatusUpdate messages
            }
        } else {
            logs::save_log(
                "Scheduler",
                &format!(
                    "Malformed RobotUpdateBatch payload on {} ({} bytes)",
                    topics::ROBOT_UPDATES,
                    payload.len()
                ),
            );
        }
    }
}

fn handle_status_updates(
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    queue: &mut dyn TaskQueue,
    robots: &mut HashMap<u32, RobotInfo>,
    inventory: &mut ShelfInventory,
    retry_state: &mut HashMap<TaskId, TaskRetryState>,
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        let payload = sample.payload().to_bytes();
        if let Ok(update) = from_slice::<TaskStatusUpdate>(&payload) {
            if let Some(task) = queue.get_mut(update.task_id) {
                println!("[{}ms] ↻ Task {} status: {:?} → {:?}", timestamp(), task.id, task.status, update.status);
                logs::save_log("Scheduler", &format!("Task {} status changed to {:?}", task.id, update.status));

                match &update.status {
                    TaskStatus::Failed { reason } => {
                        if let Some(pickup) = task.pickup_location() {
                            inventory.undo_pickup(pickup);
                        }
                        if let Some(dropoff) = task.target_location() {
                            inventory.undo_dropoff(dropoff);
                        }
                        logs::save_log("Scheduler", &format!("Task {} failed: inventory reservations undone", task.id));

                        if is_disabled_failure_reason(reason) {
                            task.status = TaskStatus::Pending;
                            task.completed_at = None;
                            retry_state.remove(&task.id);
                            logs::save_log(
                                "Scheduler",
                                &format!(
                                    "Task {} requeued after disabled robot auto-unassign",
                                    task.id
                                ),
                            );
                        } else if is_retryable_no_path_failure(reason) {
                            let state = retry_state.entry(task.id).or_insert(TaskRetryState {
                                attempts: 0,
                                next_eligible_at: std::time::Instant::now(),
                            });

                            if state.attempts < sched_config::RETRYABLE_NO_PATH_MAX_ATTEMPTS {
                                state.attempts += 1;
                                let backoff_ms = retry_backoff_with_jitter_ms(state.attempts);
                                state.next_eligible_at =
                                    std::time::Instant::now() + std::time::Duration::from_millis(backoff_ms);

                                task.status = TaskStatus::Pending;
                                task.completed_at = None;

                                logs::save_log(
                                    "Scheduler",
                                    &format!(
                                        "Task {} retry scheduled after no-path failure (attempt {}/{}, backoff {}ms)",
                                        task.id,
                                        state.attempts,
                                        sched_config::RETRYABLE_NO_PATH_MAX_ATTEMPTS,
                                        backoff_ms
                                    ),
                                );
                            } else {
                                let exhausted_reason = format!(
                                    "{} (retry exhausted after {} attempts)",
                                    reason,
                                    state.attempts
                                );
                                task.status = TaskStatus::Failed {
                                    reason: exhausted_reason,
                                };
                                task.completed_at = Some(unix_timestamp_ms());
                                retry_state.remove(&task.id);
                            }
                        } else {
                            task.status = update.status.clone();
                            task.completed_at = Some(unix_timestamp_ms());
                            retry_state.remove(&task.id);
                        }
                    }
                    _ => {
                        task.status = update.status.clone();

                        if matches!(update.status, TaskStatus::Completed | TaskStatus::Cancelled) {
                            task.completed_at = Some(unix_timestamp_ms());
                        }

                        if matches!(
                            update.status,
                            TaskStatus::Assigned { .. }
                                | TaskStatus::InProgress { .. }
                                | TaskStatus::Completed
                                | TaskStatus::Cancelled
                        ) {
                            retry_state.remove(&task.id);
                        }
                    }
                }

                // Free robot on completion/failure
                if let Some(robot_id) = update.robot_id {
                    if let Some(robot) = robots.get_mut(&robot_id) {
                        if matches!(update.status, TaskStatus::Completed | TaskStatus::Failed { .. }) {
                            robot.assigned_task = None;
                        }
                    }
                }
            }
        } else {
            logs::save_log(
                "Scheduler",
                &format!(
                    "Malformed TaskStatusUpdate payload on {} ({} bytes)",
                    topics::TASK_STATUS,
                    payload.len()
                ),
            );
        }
    }
}

async fn allocate_tasks(
    queue: &mut dyn TaskQueue,
    allocator: &dyn Allocator,
    robots: &mut HashMap<u32, RobotInfo>,
    map: &GridMap,
    inventory: &mut ShelfInventory,
    retry_state: &mut HashMap<TaskId, TaskRetryState>,
    publisher: &zenoh::pubsub::Publisher<'_>,
    verbose: bool,
) {
    let pending_ids = queue.pending_task_ids_limited(sched_config::ALLOCATION_TASK_BUDGET_PER_TICK);
    let now = std::time::Instant::now();

    for task_id in pending_ids {
        if let Some(state) = retry_state.get(&task_id) {
            if now < state.next_eligible_at {
                continue;
            }
        }

        let Some(task) = queue.get(task_id).cloned() else { continue };

        // Shelf capacity enforcement: skip tasks that can't be fulfilled
        if let Some(pickup) = task.pickup_location() {
            if !inventory.can_pickup(pickup) {
                if verbose {
                    let stock = inventory.stock_at(pickup);
                    println!("[{}ms] ⏸ Task {} skipped: pickup shelf ({},{}) empty ({:?})",
                        timestamp(), task_id, pickup.0, pickup.1, stock);
                }
                continue;
            }
        }
        if let Some(dropoff) = task.target_location() {
            if !inventory.can_dropoff(dropoff) {
                if verbose {
                    let stock = inventory.stock_at(dropoff);
                    println!("[{}ms] ⏸ Task {} skipped: dropoff shelf ({},{}) full ({:?})",
                        timestamp(), task_id, dropoff.0, dropoff.1, stock);
                }
                continue;
            }
        }

        let Some(pickup) = task.pickup_location() else { continue };
        let mut reachable_robots: HashMap<u32, RobotInfo> = HashMap::new();
        for (id, robot) in robots.iter() {
            let Some(start) = world_to_grid(robot.position) else { continue; };
            if !is_within_pickup_horizon(start, pickup) {
                continue;
            }
            if is_reachable_on_map(map, start, pickup) {
                reachable_robots.insert(*id, robot.clone());
            }
        }

        let Some(robot_id) = allocator.allocate(&task, &reachable_robots) else { continue };

        // Mark robot assigned
        let previous_assignment = robots.get(&robot_id).and_then(|robot| robot.assigned_task);
        if let Some(robot) = robots.get_mut(&robot_id) {
            robot.assigned_task = Some(task_id);
        }

        // Reserve inventory: decrement pickup shelf, increment dropoff shelf
        if let Some(pickup_pos) = task.pickup_location() {
            inventory.pickup(pickup_pos);
        }
        if let Some(dropoff_pos) = task.target_location() {
            inventory.dropoff(dropoff_pos);
        }

        // Update task status
        if let Some(task) = queue.get_mut(task_id) {
            let previous_status = task.status.clone();
            let mut assigned_task = task.clone();
            assigned_task.status = TaskStatus::Assigned { robot_id };
            let assignment = TaskAssignment { task: assigned_task, robot_id };
            if protocol::publish_json_logged(
                "Scheduler",
                "task assignment",
                &assignment,
                |payload| async move { publisher.put(payload).await.map(|_| ()) },
            )
            .await
            {
                task.status = TaskStatus::Assigned { robot_id };
                if verbose {
                    println!("[{}ms] 📤 Task {} → Robot {}", timestamp(), task_id, robot_id);
                }
            } else {
                task.status = previous_status;
                if let Some(robot) = robots.get_mut(&robot_id) {
                    robot.assigned_task = previous_assignment;
                }
                if let Some(pickup_pos) = task.pickup_location() {
                    inventory.undo_pickup(pickup_pos);
                }
                if let Some(dropoff_pos) = task.target_location() {
                    inventory.undo_dropoff(dropoff_pos);
                }
                logs::save_log(
                    "Scheduler",
                    &format!(
                        "Task {} assignment publish failed, rolled back reservation/state",
                        task_id
                    ),
                );
            }
        }
    }
}

async fn broadcast_state(
    publisher: &zenoh::pubsub::Publisher<'_>,
    queue: &dyn TaskQueue,
    robots: &HashMap<u32, RobotInfo>,
) {
    let state = QueueState {
        pending: queue.pending_count(),
        total: queue.total_count(),
        robots_online: robots.len(),
    };

    let _ = protocol::publish_json_logged(
        "Scheduler",
        "queue state broadcast",
        &state,
        |payload| async move { publisher.put(payload).await.map(|_| ()) },
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::FifoQueue;

    #[test]
    fn disabled_reason_detection_is_case_insensitive() {
        assert!(is_disabled_failure_reason("Robot Disabled"));
        assert!(is_disabled_failure_reason("robot is disabled by operator"));
        assert!(!is_disabled_failure_reason("robot busy"));
    }

    #[test]
    fn retryable_no_path_detection_is_case_insensitive() {
        assert!(is_retryable_no_path_failure("no path to pickup (10,20)"));
        assert!(is_retryable_no_path_failure("No Path To Pickup"));
        assert!(!is_retryable_no_path_failure("invalid pickup/dropoff"));
    }

    #[test]
    fn retry_backoff_growth_is_capped() {
        let first = base_retry_backoff_ms(1);
        let second = base_retry_backoff_ms(2);
        let tenth = base_retry_backoff_ms(10);

        assert_eq!(first, sched_config::RETRYABLE_NO_PATH_BASE_BACKOFF_MS);
        assert!(second >= first);
        assert!(tenth <= sched_config::RETRYABLE_NO_PATH_MAX_BACKOFF_MS);
    }

    #[test]
    fn pickup_horizon_filter_rejects_impossible_distance() {
        let max_steps = whca_pickup_horizon_steps();
        assert!(max_steps > 0);
        assert!(is_within_pickup_horizon((10, 10), (10 + max_steps, 10)));
        assert!(!is_within_pickup_horizon(
            (10, 10),
            (10 + max_steps.saturating_add(1), 10)
        ));
    }

    fn make_completed_task(id: u64, completed_at: u64) -> Task {
        let mut task = Task::new(
            id,
            TaskType::PickAndDeliver {
                pickup: (1, 1),
                dropoff: (2, 2),
                cargo_id: None,
            },
            Priority::Normal,
        );
        task.status = TaskStatus::Completed;
        task.completed_at = Some(completed_at);
        task
    }

    #[test]
    fn recent_terminal_window_keeps_most_recent_entries() {
        let mut window = Vec::new();
        push_recent_terminal_task(&mut window, &make_completed_task(1, 100), 2);
        push_recent_terminal_task(&mut window, &make_completed_task(2, 200), 2);
        push_recent_terminal_task(&mut window, &make_completed_task(3, 150), 2);

        let mut ids: Vec<u64> = window.iter().map(|task| task.id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![2, 3]);
    }

    #[test]
    fn task_list_snapshot_tracks_totals_and_windows() {
        let mut queue = FifoQueue::new();

        // active task
        let active = Task::new(
            queue.next_task_id(),
            TaskType::PickAndDeliver {
                pickup: (1, 1),
                dropoff: (2, 2),
                cargo_id: None,
            },
            Priority::Normal,
        );
        queue.enqueue(active);

        // completed task
        let mut completed = Task::new(
            queue.next_task_id(),
            TaskType::PickAndDeliver {
                pickup: (1, 1),
                dropoff: (2, 2),
                cargo_id: None,
            },
            Priority::Normal,
        );
        completed.status = TaskStatus::Completed;
        completed.completed_at = Some(1000);
        queue.enqueue(completed);

        // failed task
        let mut failed = Task::new(
            queue.next_task_id(),
            TaskType::PickAndDeliver {
                pickup: (1, 1),
                dropoff: (2, 2),
                cargo_id: None,
            },
            Priority::Normal,
        );
        failed.status = TaskStatus::Failed {
            reason: "test".to_string(),
        };
        failed.completed_at = Some(1200);
        queue.enqueue(failed);

        // cancelled task
        let mut cancelled = Task::new(
            queue.next_task_id(),
            TaskType::PickAndDeliver {
                pickup: (1, 1),
                dropoff: (2, 2),
                cargo_id: None,
            },
            Priority::Normal,
        );
        cancelled.status = TaskStatus::Cancelled;
        cancelled.completed_at = Some(1400);
        queue.enqueue(cancelled);

        let snapshot = build_task_list_snapshot(&queue);

        assert_eq!(snapshot.active_total, 1);
        assert_eq!(snapshot.completed_total, 1);
        assert_eq!(snapshot.failed_total, 1);
        assert_eq!(snapshot.cancelled_total, 1);
        assert_eq!(snapshot.active_tasks.len(), 1);
        assert_eq!(snapshot.recent_terminal_tasks.len(), 3);
    }
}
