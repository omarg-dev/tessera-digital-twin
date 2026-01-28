use bevy::prelude::*;
use protocol::{RobotUpdate, RobotUpdateBatch, topics};
use serde_json::from_slice;
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zenoh::sample::Sample;
use std::time::{Duration, Instant};

use crate::resources::{RobotUpdates, ZenohReceiver, RobotLastPositions, DebugHUD};

/// Initializes Zenoh subscriber and creates a receiver channel
pub fn setup_zenoh_receiver(mut commands: Commands) {
    let (tx, rx) = mpsc::channel::<RobotUpdate>(256); // Buffer for batch updates

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
    let session = zenoh::open(zenoh::Config::default())
        .await
        .map_err(|e| format!("open session: {}", e))?;

    let subscriber = session
        .declare_subscriber(topics::ROBOT_UPDATES)
        .await
        .map_err(|e| format!("declare subscriber: {}", e))?;

    println!("✓ Zenoh subscriber initialized on {}", topics::ROBOT_UPDATES);

    // Pump samples into the channel until the subscriber closes
    while let Ok(sample) = subscriber.recv_async().await {
        if let Err(e) = handle_sample(&tx, sample).await {
            eprintln!("Sample handling error: {}", e);
        }
    }

    Err("subscriber closed".into())
}

/// Handles an incoming Zenoh sample - now expects RobotUpdateBatch
async fn handle_sample(tx: &mpsc::Sender<RobotUpdate>, sample: Sample) -> Result<(), String> {
    let bytes = sample.payload().to_bytes();
    
    // Try to decode as batch first (new format)
    if let Ok(batch) = from_slice::<RobotUpdateBatch>(&bytes) {
        for update in batch.updates {
            tx.send(update)
                .await
                .map_err(|_| "channel closed while sending update")?;
        }
        return Ok(());
    }
    
    // Fall back to single update (legacy/compatibility)
    if let Ok(update) = from_slice::<RobotUpdate>(&bytes) {
        tx.send(update)
            .await
            .map_err(|_| "channel closed while sending update")?;
        return Ok(());
    }
    
    Err("failed to decode as RobotUpdateBatch or RobotUpdate".into())
}

/// Polls the receiver channel and collects updates into RobotUpdates resource
pub fn collect_robot_updates(
    mut receiver: ResMut<ZenohReceiver>,
    mut robot_updates: ResMut<RobotUpdates>,
    mut last_positions: ResMut<RobotLastPositions>,
    _debug_hud: ResMut<DebugHUD>,
    mut last_log: Local<Option<Instant>>,
) {
    // Clear previous updates
    robot_updates.updates.clear();

    // Poll all available updates from the channel (non-blocking)
    let mut received_count = 0;
    let mut applied_count = 0;
    
    loop {
        match receiver.0.try_recv() {
            Ok(update) => {
                received_count += 1;
                
                // Only store the update if the position changed vs last seen for this id
                let last = last_positions.by_id.get(&update.id);
                let moved = match last {
                    Some(prev) => prev != &update.position,
                    None => true,
                };
                
                if moved {
                    applied_count += 1;
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
    
    // Periodically log summary (less noisy than per-frame logging)
    let should_log = last_log
        .map(|t| t.elapsed() >= Duration::from_secs(3))
        .unwrap_or(false);
    
    if should_log && received_count > 0 {
        println!("📊 Sync: {}/{} updates applied", applied_count, received_count);
        *last_log = Some(Instant::now());
    }
}
