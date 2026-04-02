//! Subscribe to robot update batches from firmware via Zenoh.

use bevy::prelude::*;
use protocol::config::visualizer::network as net_cfg;
use protocol::{RobotUpdate, RobotUpdateBatch, topics};
use serde_json::from_slice;
use tokio::sync::mpsc;
use zenoh::sample::Sample;
use zenoh::Session;
use crate::resources::{BackpressureMetrics, RobotLastPositions, RobotUpdates, ZenohReceiver, ZenohSession};

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecodeFormat {
    Batch,
    Single,
}

/// Initializes Zenoh subscriber and creates a receiver channel
pub fn setup_zenoh_receiver(
    mut commands: Commands,
    session: Res<ZenohSession>,
    backpressure: Res<BackpressureMetrics>,
) {
    let (tx, rx) = mpsc::channel::<RobotUpdateBatch>(net_cfg::ROBOT_UPDATES_CHANNEL_CAPACITY);
    let sess = session.session.clone();
    let pressure = backpressure.robot_updates.handle();

    // spawn on the shared runtime (no extra thread or runtime)
    session.runtime.spawn(async move {
        if let Err(e) = run_zenoh_listener(sess, tx, pressure).await {
            eprintln!("Zenoh listener exited: {}", e);
        }
    });

    commands.insert_resource(ZenohReceiver(rx));
    commands.init_resource::<RobotUpdates>();
}

async fn run_zenoh_listener(
    session: Session,
    tx: mpsc::Sender<RobotUpdateBatch>,
    pressure: crate::resources::ChannelBackpressureHandle,
) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::ROBOT_UPDATES)
        .await
        .map_err(|e| format!("declare subscriber: {}", e))?;

    // subscriber ready (logged to UI via LogBuffer at startup)
    let mut last_decode_format: Option<DecodeFormat> = None;

    // Pump samples into the channel until the subscriber closes
    while let Ok(sample) = subscriber.recv_async().await {
        match handle_sample(&tx, sample, &pressure) {
            Ok(format) => {
                if last_decode_format != Some(format) {
                    let mode = match format {
                        DecodeFormat::Batch => "RobotUpdateBatch",
                        DecodeFormat::Single => "RobotUpdate",
                    };
                    eprintln!("Robot update decode mode: {}", mode);
                    last_decode_format = Some(format);
                }
            }
            Err(e) => {
            eprintln!("Sample handling error: {}", e);
            }
        }
    }

    Err("subscriber closed".into())
}

/// Handles an incoming Zenoh sample - now expects RobotUpdateBatch
fn handle_sample(
    tx: &mpsc::Sender<RobotUpdateBatch>,
    sample: Sample,
    pressure: &crate::resources::ChannelBackpressureHandle,
) -> Result<DecodeFormat, String> {
    let bytes = sample.payload().to_bytes();

    // Try to decode as batch first (new format)
    if let Ok(batch) = from_slice::<RobotUpdateBatch>(&bytes) {
        pressure.on_received();
        match tx.try_send(batch) {
            Ok(()) => pressure.on_enqueued(),
            Err(mpsc::error::TrySendError::Full(_)) => pressure.on_dropped_full(),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                pressure.on_blocked_send();
                return Err("channel closed while sending batch".into());
            }
        }
        return Ok(DecodeFormat::Batch);
    }

    // Fall back to single update (legacy/compatibility)
    if let Ok(update) = from_slice::<RobotUpdate>(&bytes) {
        pressure.on_received();
        let batch = RobotUpdateBatch {
            updates: vec![update],
            tick: 0,
        };
        match tx.try_send(batch) {
            Ok(()) => pressure.on_enqueued(),
            Err(mpsc::error::TrySendError::Full(_)) => pressure.on_dropped_full(),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                pressure.on_blocked_send();
                return Err("channel closed while sending batch".into());
            }
        }
        return Ok(DecodeFormat::Single);
    }

    Err("failed to decode as RobotUpdateBatch or RobotUpdate".into())
}

/// Polls the receiver channel and collects updates into RobotUpdates resource
pub fn collect_robot_updates(
    mut receiver: ResMut<ZenohReceiver>,
    mut robot_updates: ResMut<RobotUpdates>,
    mut last_positions: ResMut<RobotLastPositions>,
    mut backpressure: ResMut<BackpressureMetrics>,
) {
    robot_updates.updates.clear();
    robot_updates.last_batch_tick = None;
    backpressure.robot_updates.record_queue_depth(receiver.0.len());

    loop {
        match receiver.0.try_recv() {
            Ok(batch) => {
                robot_updates.last_batch_tick = Some(batch.tick);

                // always update last positions and push updates so that sync_robots
                // can refresh last_update_secs even for idle robots that haven't
                // moved or changed state.
                for update in batch.updates {
                    last_positions.by_id.insert(update.id, update.position);
                    last_positions.state_by_id.insert(update.id, update.state.clone());
                    robot_updates.updates.push(update);
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => break,
            Err(mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
}
