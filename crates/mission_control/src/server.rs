//! Mission Control server loop

use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time;
use zenoh::Session;
use serde_json::{from_slice, to_vec};

use protocol::config::mission_control as mc_config;
use protocol::{
    topics, GridMap, Priority, RobotUpdateBatch, Task, TaskAssignment,
    TaskRequest, TaskStatus, TaskStatusUpdate, TaskType,
};

use crate::allocator::{Allocator, ClosestIdleAllocator, RobotInfo};
use crate::cli::{print_status, print_shelves, print_dropoffs, print_stations, print_map, spawn_stdin_reader, StdinCmd};
use crate::commands::handle_system_commands;
use crate::queue::{FifoQueue, TaskQueue};

/// Run the mission control main loop
pub async fn run(session: Session) {
    // Load warehouse map for location info
    let map = GridMap::load_from_file("assets/data/layout.txt")
        .expect("Failed to load warehouse layout");
    println!("✓ Loaded map ({}x{}, {} shelves, {} dropoffs)",
        map.width, map.height,
        map.get_shelves().len(),
        map.get_dropoffs().len(),
    );

    // Publishers
    let assignment_pub = session.declare_publisher(topics::TASK_ASSIGNMENTS).await.unwrap();
    let queue_pub = session.declare_publisher(topics::QUEUE_STATE).await.unwrap();

    // Subscribers
    let task_sub = session.declare_subscriber(topics::TASK_REQUESTS).await.unwrap();
    let robot_sub = session.declare_subscriber(topics::ROBOT_UPDATES).await.unwrap();
    let status_sub = session.declare_subscriber(topics::TASK_STATUS).await.unwrap();
    let control_sub = session.declare_subscriber(topics::ADMIN_CONTROL).await.unwrap();

    // State
    let mut queue = FifoQueue::new();
    let allocator = ClosestIdleAllocator::new();
    let mut robots: HashMap<u32, RobotInfo> = HashMap::new();
    let mut paused = false;
    let mut verbose = true; // Default to verbose on

    // Stdin
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);

    println!("✓ Mission Control running");
    println!("Commands: status, add <px> <py> <dx> <dy>, help");
    println!("(System commands: run control_plane)");

    let mut last_broadcast = std::time::Instant::now();

    loop {
        // System commands (from Zenoh - control_plane)
        handle_system_commands(&control_sub, &mut paused, &mut verbose);
        
        // Stdin commands (local CLI)
        handle_stdin(&mut rx, &mut queue, &robots, &map, paused, verbose).await;
        
        // Task requests (from other crates)
        handle_task_requests(&task_sub, &mut queue);
        
        // Robot updates (from swarm_driver)
        handle_robot_updates(&robot_sub, &mut robots);
        
        // Task status updates (from fleet_server)
        handle_status_updates(&status_sub, &mut queue, &mut robots);

        // Allocate tasks
        if !paused {
            allocate_tasks(&mut queue, &allocator, &mut robots, &assignment_pub).await;
        }

        // Broadcast queue state
        if last_broadcast.elapsed() >= std::time::Duration::from_secs(mc_config::QUEUE_BROADCAST_SECS) {
            broadcast_state(&queue_pub, &queue, &robots).await;
            last_broadcast = std::time::Instant::now();
        }

        time::sleep(std::time::Duration::from_millis(mc_config::LOOP_INTERVAL_MS)).await;
    }
}

async fn handle_stdin(
    rx: &mut mpsc::Receiver<StdinCmd>,
    queue: &mut FifoQueue,
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
                        println!("✓ Task #{} added: ({},{}) → ({},{})", id, p.0, p.1, d.0, d.1);
                        queue.enqueue(task);
                    }
                    (None, _) => println!("✗ Invalid pickup location"),
                    (_, None) => println!("✗ Invalid dropoff location"),
                }
            }
            StdinCmd::ListShelves => print_shelves(map),
            StdinCmd::ListDropoffs => print_dropoffs(map),
            StdinCmd::ListStations => print_stations(map),
            StdinCmd::Map => print_map(map, robots),
            StdinCmd::Help => {} // Already printed by CLI
        }
    }
}

/// Resolve a location - either already coordinates or named (S1=10001, D1=20001, etc.)
fn resolve_location(loc: (usize, usize), map: &GridMap) -> Option<(usize, usize)> {
    let (x, y) = loc;
    
    // Check if it's a named location marker
    if x >= 10000 && x < 20000 {
        // Shelf: S1 = 10001, so index = x - 10000
        let idx = x - 10000;
        let shelves = map.get_shelves();
        if idx >= 1 && idx <= shelves.len() {
            let shelf = shelves[idx - 1];
            return Some((shelf.x, shelf.y));
        }
        println!("✗ Shelf S{} not found (valid: S1-S{})", idx, shelves.len());
        return None;
    } else if x >= 20000 {
        // Dropoff: D1 = 20001, so index = x - 20000
        let idx = x - 20000;
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
    queue: &mut FifoQueue,
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        if let Ok(req) = from_slice::<TaskRequest>(&sample.payload().to_bytes()) {
            let id = queue.next_task_id();
            let task = Task::new(id, req.task_type, req.priority);
            queue.enqueue(task);
        }
    }
}

fn handle_robot_updates(
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    robots: &mut HashMap<u32, RobotInfo>,
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        if let Ok(batch) = from_slice::<RobotUpdateBatch>(&sample.payload().to_bytes()) {
            for update in &batch.updates {
                let entry = robots.entry(update.id).or_insert_with(|| RobotInfo::from(update));
                entry.position = update.position;
                entry.state = update.state.clone();
                entry.battery = update.battery;
            }
        }
    }
}

fn handle_status_updates(
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    queue: &mut dyn TaskQueue,
    robots: &mut HashMap<u32, RobotInfo>,
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        if let Ok(update) = from_slice::<TaskStatusUpdate>(&sample.payload().to_bytes()) {
            if let Some(task) = queue.get_mut(update.task_id) {
                println!("↻ Task {} status: {:?} → {:?}", task.id, task.status, update.status);
                task.status = update.status.clone();

                // Free robot on completion/failure
                if let Some(robot_id) = update.robot_id {
                    if let Some(robot) = robots.get_mut(&robot_id) {
                        if matches!(update.status, TaskStatus::Completed | TaskStatus::Failed { .. }) {
                            robot.assigned_task = None;
                        }
                    }
                }
            }
        }
    }
}

async fn allocate_tasks(
    queue: &mut dyn TaskQueue,
    allocator: &dyn Allocator,
    robots: &mut HashMap<u32, RobotInfo>,
    publisher: &zenoh::pubsub::Publisher<'_>,
) {
    let pending_ids: Vec<u64> = queue.pending_tasks().iter().map(|t| t.id).collect();

    for task_id in pending_ids {
        let Some(task) = queue.get(task_id).cloned() else { continue };
        let Some(robot_id) = allocator.allocate(&task, robots) else { continue };

        // Mark robot assigned
        if let Some(robot) = robots.get_mut(&robot_id) {
            robot.assigned_task = Some(task_id);
        }

        // Update task status
        if let Some(task) = queue.get_mut(task_id) {
            task.status = TaskStatus::Assigned { robot_id };

            let assignment = TaskAssignment { task: task.clone(), robot_id };
            if let Ok(payload) = to_vec(&assignment) {
                publisher.put(payload).await.ok();
                println!("📤 Task {} → Robot {}", task_id, robot_id);
            }
        }
    }
}

async fn broadcast_state(
    publisher: &zenoh::pubsub::Publisher<'_>,
    queue: &dyn TaskQueue,
    robots: &HashMap<u32, RobotInfo>,
) {
    #[derive(serde::Serialize)]
    struct QueueState { pending: usize, total: usize, robots_online: usize }

    let state = QueueState {
        pending: queue.pending_count(),
        total: queue.total_count(),
        robots_online: robots.len(),
    };

    if let Ok(payload) = to_vec(&state) {
        publisher.put(payload).await.ok();
    }
}
