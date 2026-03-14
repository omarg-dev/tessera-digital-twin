//! Subscribe to robot update batches from firmware via Zenoh.

use bevy::prelude::*;
use protocol::{RobotUpdate, RobotUpdateBatch, topics};
use serde_json::from_slice;
use tokio::sync::mpsc;
use zenoh::sample::Sample;
use zenoh::Session;
use crate::resources::{RobotUpdates, ZenohReceiver, RobotLastPositions, ZenohSession};

/// Initializes Zenoh subscriber and creates a receiver channel
pub fn setup_zenoh_receiver(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<RobotUpdate>(256);
    let sess = session.session.clone();

    // spawn on the shared runtime (no extra thread or runtime)
    session.runtime.spawn(async move {
        if let Err(e) = run_zenoh_listener(sess, tx).await {
            eprintln!("Zenoh listener exited: {}", e);
        }
    });

    commands.insert_resource(ZenohReceiver(rx));
    commands.init_resource::<RobotUpdates>();
}

async fn run_zenoh_listener(session: Session, tx: mpsc::Sender<RobotUpdate>) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::ROBOT_UPDATES)
        .await
        .map_err(|e| format!("declare subscriber: {}", e))?;

    // subscriber ready (logged to UI via LogBuffer at startup)

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
            if tx.send(update).await.is_err() {
                return Err("channel closed while sending update".into());
            }
        }
        return Ok(());
    }

    // Fall back to single update (legacy/compatibility)
    if let Ok(update) = from_slice::<RobotUpdate>(&bytes) {
        if tx.send(update).await.is_err() {
            return Err("channel closed while sending update".into());
        }
        return Ok(());
    }

    Err("failed to decode as RobotUpdateBatch or RobotUpdate".into())
}

/// Polls the receiver channel and collects updates into RobotUpdates resource
pub fn collect_robot_updates(
    mut receiver: ResMut<ZenohReceiver>,
    mut robot_updates: ResMut<RobotUpdates>,
    mut last_positions: ResMut<RobotLastPositions>,
) {
    robot_updates.updates.clear();

    loop {
        match receiver.0.try_recv() {
            Ok(update) => {
                // always update last positions and push the update so that
                // sync_robots can refresh last_update_secs even for idle
                // robots that haven't moved or changed state.
                last_positions.by_id.insert(update.id, update.position);
                last_positions.state_by_id.insert(update.id, update.state.clone());
                robot_updates.updates.push(update);
            }
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}
