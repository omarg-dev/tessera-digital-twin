//! Fleet Server - Central Brain for the Digital Twin
//!
//! Responsibilities:
//! - Receive RobotUpdate from swarm_driver
//! - Calculate paths and assign tasks (A* pathfinding)
//! - Send PathCmd to swarm_driver
//! - Broadcast SystemCommand for control plane
//! - Validate map hash on startup

mod state;
mod server;
mod pathfinding;
mod commands;

use zenoh::*;
use protocol::{GridMap, LAYOUT_FILE_PATH};

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════╗");
    println!("║       FLEET SERVER - Central Brain     ║");
    println!("╚════════════════════════════════════════╝");
    
    // Load and validate map
    let map = GridMap::load_from_file(LAYOUT_FILE_PATH)
        .expect("Failed to load map");
    println!("✓ Map loaded: {}x{} (hash: {:016x})", map.width, map.height, map.hash);
    
    let session = open(Config::default())
        .await
        .expect("Failed to open Zenoh session");
    
    server::run(session, map).await;
}
