//! Scheduler server loop

use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time;
use zenoh::Session;
use serde_json::from_slice;
use rand::seq::SliceRandom;
use rand::thread_rng;

use protocol::config::scheduler as sched_config;
use protocol::{
    timestamp, logs, topics, GridMap, Priority, QueueState, RobotUpdateBatch, ShelfInventory,
    Task, TaskAssignment, TaskCommand, TaskListSnapshot, TaskStatus, TaskStatusUpdate, TaskType,
    is_reachable_on_map, world_to_grid,
};

use crate::allocator::{Allocator, AllocatorInstance, RobotInfo};
use crate::cli::{print_status, print_shelves, print_dropoffs, print_stations, print_map, print_history, spawn_stdin_reader, print_help, StdinCmd};
use crate::commands::handle_system_commands;
use crate::queue::{QueueInstance, TaskQueue};

/// Run the scheduler main loop
pub async fn run(session: Session) {
    // Load warehouse map for location info
    let layout_path = protocol::config::resolve_layout_path();
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
    let mut paused = false;
    let mut verbose = true; // Default to verbose on

    // Stdin
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);

    println!("[{}ms] ✓ Scheduler running", timestamp());
    println!("[{}ms] Commands: status, add <px> <py> <dx> <dy>, help", timestamp());
    println!("[{}ms] (System commands: run orchestrator)", timestamp());

    let mut last_broadcast = std::time::Instant::now();
    let mut chaos = protocol::config::chaos::ENABLED;

    loop {
        // System commands (from Zenoh - orchestrator)
        handle_system_commands(&control_sub, &mut paused, &mut verbose, &mut chaos);
        
        // Stdin commands (local CLI)
        handle_stdin(&mut rx, &mut queue, &robots, &map, paused, verbose).await;
        
        // Task requests (from other crates)
        handle_task_requests(&task_sub, &mut queue, &inventory);
        
        // Robot updates (from firmware)
        handle_robot_updates(&robot_sub, &mut robots);
        
        // Task status updates (from coordinator)
        handle_status_updates(&status_sub, &mut queue, &mut robots, &mut inventory);

        // Allocate tasks
        if !paused {
            allocate_tasks(&mut queue, &allocator, &mut robots, &map, &mut inventory, &assignment_pub, verbose).await;
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

fn handle_task_requests(
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    queue: &mut dyn TaskQueue,
    inventory: &ShelfInventory,
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
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let snapshot = TaskListSnapshot {
        tasks: queue.all_tasks().into_iter().cloned().collect(),
        timestamp_ms,
    };
    let _ = protocol::publish_json_logged(
        "Scheduler",
        "task list snapshot",
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
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        let payload = sample.payload().to_bytes();
        if let Ok(update) = from_slice::<TaskStatusUpdate>(&payload) {
            if let Some(task) = queue.get_mut(update.task_id) {
                println!("[{}ms] ↻ Task {} status: {:?} → {:?}", timestamp(), task.id, task.status, update.status);
                logs::save_log("Scheduler", &format!("Task {} status changed to {:?}", task.id, update.status));

                let requeue_disabled_failure = match &update.status {
                    TaskStatus::Failed { reason } => reason.to_ascii_lowercase().contains("disabled"),
                    _ => false,
                };

                // Undo inventory reservations on task failure
                if matches!(update.status, TaskStatus::Failed { .. }) {
                    if let Some(pickup) = task.pickup_location() {
                        inventory.undo_pickup(pickup);
                    }
                    if let Some(dropoff) = task.target_location() {
                        inventory.undo_dropoff(dropoff);
                    }
                    logs::save_log("Scheduler", &format!("Task {} failed: inventory reservations undone", task.id));
                }

                if requeue_disabled_failure {
                    task.status = TaskStatus::Pending;
                    task.completed_at = None;
                    logs::save_log(
                        "Scheduler",
                        &format!(
                            "Task {} requeued after disabled robot auto-unassign",
                            task.id
                        ),
                    );
                } else {
                    task.status = update.status.clone();

                    // stamp completion time for terminal transitions
                    if matches!(update.status, TaskStatus::Completed | TaskStatus::Failed { .. } | TaskStatus::Cancelled) {
                        task.completed_at = Some(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64
                        );
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
    publisher: &zenoh::pubsub::Publisher<'_>,
    verbose: bool,
) {
    let pending_ids: Vec<u64> = queue.pending_tasks().iter().map(|t| t.id).collect();

    for task_id in pending_ids {
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
