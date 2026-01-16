use bevy::prelude::*;
use protocol::RobotUpdate;
use serde_json::from_slice;
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zenoh::sample::Sample;
use std::time::{Duration, Instant};

use crate::resources::{RobotUpdates, ZenohReceiver, RobotLastPositions, DebugHUD};

/// Initializes Zenoh subscriber and creates a receiver channel
pub fn setup_zenoh_receiver(mut commands: Commands) {
    let (tx, rx) = mpsc::channel::<RobotUpdate>(100); // Buffer up to 100 updates

    // Spawn a background thread with its own Tokio runtime for Zenoh async work
    thread::spawn(move || {
        let rt = Runtime::new().expect("Failed to create Tokio runtime for Zenoh receiver");
        rt.block_on(async move {
            if let Err(e) = run_zenoh_listener(tx).await {
                eprintln!("Zenoh listener exited: {}", e);
            }
        });
    });

    // Store the receiving end in Bevy so frame systems can poll it
    commands.insert_resource(ZenohReceiver(rx));
    commands.init_resource::<RobotUpdates>();
}

async fn run_zenoh_listener(tx: mpsc::Sender<RobotUpdate>) -> Result<(), String> {
    // Owns the Zenoh session and subscriber inside the background thread
    let session = zenoh::open(zenoh::Config::default())
        .await
        .map_err(|e| format!("open session: {}", e))?;

    let subscriber = session
        .declare_subscriber("factory/robots")
        .await
        .map_err(|e| format!("declare subscriber: {}", e))?;

    println!("✓ Zenoh subscriber initialized on factory/robots");

    // Pump samples into the channel until the subscriber closes
    while let Ok(sample) = subscriber.recv_async().await {
        if let Err(e) = handle_sample(&tx, sample).await {
            eprintln!("Sample handling error: {}", e);
        }
    }

    Err("subscriber closed".into())
}

// Handles an incoming Zenoh sample by decoding and sending it through the channel
async fn handle_sample(tx: &mpsc::Sender<RobotUpdate>, sample: Sample) -> Result<(), String> {
    let update = decode_update(sample)?;
    tx.send(update)
        .await
        .map_err(|_| "channel closed while sending update".into())
}

// Decodes a RobotUpdate from a Zenoh sample payload
fn decode_update(sample: Sample) -> Result<RobotUpdate, String> {
    from_slice::<RobotUpdate>(&sample.payload().to_bytes())
        .map_err(|e| format!("deserialize RobotUpdate: {}", e))
}

/// Polls the receiver channel and collects updates into RobotUpdates resource
pub fn collect_robot_updates(
    mut receiver: ResMut<ZenohReceiver>,
    mut robot_updates: ResMut<RobotUpdates>,
    mut last_positions: ResMut<RobotLastPositions>,
    mut debug_hud: ResMut<DebugHUD>,
    mut last_log: Local<Option<Instant>>,
) {
    // Clear previous updates
    robot_updates.updates.clear();

    // Poll all available updates from the channel (non-blocking)
    loop {
        match receiver.0.try_recv() {
            Ok(update) => {
                // Only store the update if the position changed vs last seen for this id
                let last = last_positions.by_id.get(&update.id);
                let moved = match last {
                    Some(prev) => prev != &update.position,
                    None => true,
                };
                
                // Check if enough time passed since last log
                let should_log = last_log
                    .map(|t| t.elapsed() >= Duration::from_secs(1))
                    .unwrap_or(true);
                
                if moved && should_log {
                    // Update HUD text for UI display
                    debug_hud.last_message = Some(format!(
                        "Received RobotUpdate_ID: {:?}, State: {:?}, Position: {:?}",
                        update.id, update.state, update.position
                    ));
                    *last_log = Some(Instant::now());
                    last_positions.by_id.insert(update.id, update.position);
                    robot_updates.updates.push(update);
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                break;
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                eprintln!("Zenoh receiver channel disconnected!");
                break;
            }
        }
    }
}
