//! Coordinator - Path Planning and Task Execution
//!
//! The coordinator layer manages path planning and task execution.
//! It receives task assignments from the scheduler and sends movement
//! commands to the firmware layer.
//!
//! ## Responsibilities
//! - Receive task assignments from scheduler
//! - Calculate paths using a pathfinding algorithm
//! - Send PathCmd packets to firmware layer
//! - Track task execution lifecycle
//! - Broadcast map hash for validation

mod state;
mod server;
mod pathfinding;
mod commands;
mod task_manager;

use zenoh::*;
use protocol::GridMap;

#[tokio::main]
async fn main() {
    println!("╔═════════════════════════════════════════════╗");
    println!("║       COORDINATOR - Path Planning           ║");
    println!("╚═════════════════════════════════════════════╝");
    
    // Load and validate map
    let layout_path = protocol::config::resolve_layout_path();
    let map = GridMap::load_from_file(&layout_path)
        .expect("Failed to load map");
    println!("✓ Map loaded from {}: {}x{} (hash: {:016x})", layout_path, map.width, map.height, map.hash);
    
    let session = open(Config::default())
        .await
        .expect("Failed to open Zenoh session");
    
    server::run(session, map).await;
}
