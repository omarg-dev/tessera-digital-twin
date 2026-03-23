//! Mock Firmware - Firmware Layer Simulation
//!
//! Responsibilities:
//! - Simulate robot physics (pos += vel * dt)
//! - Publish RobotUpdateBatch to coordinator and renderer
//! - Subscribe to PathCmd from coordinator
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
    println!("║     MOCK FIRMWARE - Firmware Layer     ║");
    println!("╚════════════════════════════════════════╝");
    
    // Load and validate map
    let layout_path = protocol::layout::resolve_layout_path();
    let map = GridMap::load_from_file(&layout_path)
        .expect("Failed to load map");
    println!("✓ Map loaded from {}: {}x{} (hash: {:016x})", layout_path, map.width, map.height, map.hash);
    
    let session = open(Config::default())
        .await
        .expect("Failed to open Zenoh session");
    
    // Validate map hash with coordinator
    validate_map_hash(&session, &map).await;
    
    driver::run(session, map).await;
}

async fn validate_map_hash(session: &Session, map: &GridMap) {
    use protocol::config::coordinator::MAP_VALIDATION_TIMEOUT_SECS;
    
    let subscriber = session
        .declare_subscriber(topics::MAP_VALIDATION)
        .await
        .expect("Failed to subscribe to MAP_VALIDATION");
    
    println!("⏳ Waiting for map hash validation from coordinator...");
    
    let timeout_duration = std::time::Duration::from_secs(MAP_VALIDATION_TIMEOUT_SECS);
    
    let result = tokio::time::timeout(timeout_duration, async {
        loop {
            if let Ok(Some(sample)) = subscriber.try_recv() {
                let bytes = sample.payload().to_bytes();
                match from_slice::<MapValidation>(&bytes) {
                    Ok(validation) if validation.sender == topics::SENDER_COORDINATOR => {
                        return Some(validation);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        protocol::logs::save_log(
                            "Firmware",
                            &format!("Ignoring malformed MAP_VALIDATION payload: {}", e),
                        );
                    }
                }
            }
            time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }).await;
    
    match result {
        Ok(Some(validation)) => {
            if validation.map_hash == map.hash {
                println!("✓ Map hash validated with coordinator");
            } else {
                eprintln!("✗ MAP HASH MISMATCH! Coordinator: {:016x}, Local: {:016x}", 
                    validation.map_hash, map.hash);
                eprintln!("  This could cause 'Ghost Wall' bugs. Exiting.");
                std::process::exit(1);
            }
        }
        _ => {
            println!("⚠ No coordinator response within {}s. Proceeding with local map.", 
                MAP_VALIDATION_TIMEOUT_SECS);
        }
    }
}
