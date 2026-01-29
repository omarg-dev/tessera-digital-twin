//! Scheduler - Task Queue and Robot Allocation
//!
//! The scheduler layer manages the task queue and robot allocation.
//! Receives orders, queues them, and assigns to available robots.
//!
//! ## Responsibilities
//! - Maintain task queue with priority support
//! - Allocate tasks to idle robots based on distance/battery
//! - Track task lifecycle (Pending → Assigned → InProgress → Completed)
//! - Broadcast queue state for monitoring
//!
//! ## TODO: UI Improvements (Phase 5+)
//! - [ ] Web dashboard for task management (REST API + React/Vue frontend)
//! - [ ] Real-time queue visualization in Bevy visualizer
//! - [ ] Task priority adjustment UI
//! - [ ] Order batch import (CSV/JSON file upload)
//! - [ ] Analytics: task completion rates, robot utilization, wait times

mod allocator;
mod cli;
mod commands;
mod queue;
mod server;

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════════╗");
    println!("║       SCHEDULER - Task Management          ║");
    println!("╚════════════════════════════════════════════╝");

    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open Zenoh session");

    println!("✓ Zenoh session established");
    server::run(session).await;
}
