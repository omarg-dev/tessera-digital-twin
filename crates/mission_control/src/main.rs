//! Mission Control - The Boss
//!
//! Manages the task queue and robot allocation.
//! Receives orders, queues them, and assigns to available robots.

mod allocator;
mod cli;
mod queue;

use allocator::{Allocator, ClosestIdleAllocator, RobotInfo};
use cli::{print_status, spawn_stdin_reader, StdinCmd};
use protocol::config::mission_control as mc_config;
use protocol::{
    topics, Priority, RobotUpdateBatch, SystemCommand, Task, TaskAssignment,
    TaskRequest, TaskStatus, TaskStatusUpdate, TaskType,
};
use queue::{FifoQueue, TaskQueue};
use serde_json::{from_slice, to_vec};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time;
use zenoh::Session;

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════╗");
    println!("║       MISSION CONTROL - The Boss       ║");
    println!("╚════════════════════════════════════════╝");

    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open Zenoh session");

    println!("✓ Zenoh session established");
    run(session).await;
}

async fn run(session: Session) {
    // Publishers
    let assignment_pub = session.declare_publisher(topics::TASK_ASSIGNMENTS).await.unwrap();
    let queue_pub = session.declare_publisher(topics::QUEUE_STATE).await.unwrap();
    let control_pub = session.declare_publisher(topics::ADMIN_CONTROL).await.unwrap();

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

    // Stdin
    let (tx, mut rx) = mpsc::channel::<StdinCmd>(16);
    spawn_stdin_reader(tx);

    println!("✓ Mission Control running");
    println!("Commands: status, add <px> <py> <dx> <dy>, pause, resume, reset, kill");

    let mut last_broadcast = std::time::Instant::now();

    loop {
        // System commands (from Zenoh - other crates)
        handle_control(&control_sub, &mut paused, &mut queue, &mut robots);
        
        // Stdin commands (local CLI)
        handle_stdin(&mut rx, &mut queue, &robots, paused, &control_pub).await;
        
        // Task requests
        handle_task_requests(&task_sub, &mut queue);
        
        // Robot updates
        handle_robot_updates(&robot_sub, &mut robots);
        
        // Task status updates
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

fn handle_control(
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<zenoh::sample::Sample>>,
    paused: &mut bool,
    queue: &mut FifoQueue,
    robots: &mut HashMap<u32, RobotInfo>,
) {
    while let Ok(Some(sample)) = sub.try_recv() {
        if let Ok(cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
            apply_system_command(cmd, paused, queue, robots);
        }
    }
}

fn apply_system_command(
    cmd: SystemCommand,
    paused: &mut bool,
    queue: &mut FifoQueue,
    robots: &mut HashMap<u32, RobotInfo>,
) {
    match cmd {
        SystemCommand::Pause => { *paused = true; println!("⏸ PAUSED"); }
        SystemCommand::Resume => { *paused = false; println!("▶ RESUMED"); }
        SystemCommand::Reset => {
            *queue = FifoQueue::new();
            robots.clear();
            *paused = true;
            println!("🔄 RESET");
        }
        SystemCommand::Kill => {
            println!("☠ KILL");
            std::process::exit(0);
        }
    }
}

async fn handle_stdin(
    rx: &mut mpsc::Receiver<StdinCmd>,
    queue: &mut FifoQueue,
    robots: &HashMap<u32, RobotInfo>,
    paused: bool,
    control_pub: &zenoh::pubsub::Publisher<'_>,
) {
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            StdinCmd::Status => print_status(queue, robots, paused),
            StdinCmd::AddTask { pickup, dropoff } => {
                let id = queue.next_task_id();
                let task = Task::new(id, TaskType::PickAndDeliver {
                    pickup, dropoff, cargo_id: None,
                }, Priority::Normal);
                queue.enqueue(task);
            }
            StdinCmd::System(sys_cmd) => {
                // Broadcast to other crates
                if let Ok(payload) = to_vec(&sys_cmd) {
                    control_pub.put(payload).await.ok();
                }
            }
        }
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
