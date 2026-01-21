//! Swarm Driver - Mock Robot Firmware (Physics Simulation)
//!
//! Responsibilities:
//! - Simulate robot physics (pos += vel * dt)
//! - Publish RobotUpdateBatch to fleet_server and visualizer
//! - Subscribe to PathCmd from fleet_server
//! - Subscribe to SystemCommand for pause/resume
//! - Validate map hash on startup

mod robot;
mod driver;
mod commands;

use zenoh::*;
use tokio::time;
use protocol::*;
use serde_json::from_slice;

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════╗");
    println!("║     SWARM DRIVER - Robot Firmware      ║");
    println!("╚════════════════════════════════════════╝");
    
    // Load and validate map
    let map = GridMap::load_from_file(LAYOUT_FILE_PATH)
        .expect("Failed to load map");
    println!("✓ Map loaded: {}x{} (hash: {:016x})", map.width, map.height, map.hash);
    
    let session = open(Config::default())
        .await
        .expect("Failed to open Zenoh session");
    
    // Validate map hash with fleet_server
    validate_map_hash(&session, &map).await;
    
    driver::run(session, map).await;
}

async fn validate_map_hash(session: &Session, map: &GridMap) {
    use protocol::config::server::MAP_VALIDATION_TIMEOUT_SECS;
    
    let subscriber = session
        .declare_subscriber(topics::MAP_VALIDATION)
        .await
        .expect("Failed to subscribe to MAP_VALIDATION");
    
    println!("⏳ Waiting for map hash validation from fleet_server...");
    
    let timeout_duration = std::time::Duration::from_secs(MAP_VALIDATION_TIMEOUT_SECS);
    
    let result = tokio::time::timeout(timeout_duration, async {
        loop {
            if let Ok(Some(sample)) = subscriber.try_recv() {
                if let Ok(validation) = from_slice::<MapValidation>(&sample.payload().to_bytes()) {
                    if validation.sender == "fleet_server" {
                        return Some(validation);
                    }
                }
            }
            time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }).await;
    
    match result {
        Ok(Some(validation)) => {
            if validation.map_hash == map.hash {
                println!("✓ Map hash validated with fleet_server");
            } else {
                eprintln!("✗ MAP HASH MISMATCH! Server: {:016x}, Local: {:016x}", 
                    validation.map_hash, map.hash);
                eprintln!("  This could cause 'Ghost Wall' bugs. Exiting.");
                std::process::exit(1);
            }
        }
        _ => {
            println!("⚠ No fleet_server response within {}s. Proceeding with local map.", 
                MAP_VALIDATION_TIMEOUT_SECS);
        }
    }
}
