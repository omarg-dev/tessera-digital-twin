//! Subscribe to the scheduler's QueueState broadcasts via Zenoh.

use bevy::prelude::*;
use protocol::config::visualizer::network as net_cfg;
use protocol::{QueueState, topics};
use serde_json::from_slice;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{BackpressureMetrics, QueueStateData, QueueStateReceiver, ZenohSession};

/// Initialize background Zenoh subscriber for task queue state
pub fn setup_queue_listener(
    mut commands: Commands,
    session: Res<ZenohSession>,
    backpressure: Res<BackpressureMetrics>,
) {
    let (tx, rx) = mpsc::channel::<QueueState>(net_cfg::QUEUE_STATE_CHANNEL_CAPACITY);
    let sess = session.session.clone();
    let pressure = backpressure.queue_state.handle();

    session.runtime.spawn(async move {
        if let Err(e) = run_queue_listener(sess, tx, pressure).await {
            eprintln!("Queue state listener exited: {}", e);
        }
    });

    commands.insert_resource(QueueStateReceiver(rx));
}

async fn run_queue_listener(
    session: Session,
    tx: mpsc::Sender<QueueState>,
    pressure: crate::resources::ChannelBackpressureHandle,
) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::QUEUE_STATE)
        .await
        .map_err(|e| format!("declare queue subscriber: {}", e))?;

    while let Ok(sample) = subscriber.recv_async().await {
        if let Ok(state) = from_slice::<QueueState>(&sample.payload().to_bytes()) {
            pressure.on_received();
            if tx.send(state).await.is_err() {
                pressure.on_blocked_send();
                return Err("queue state receiver dropped".into());
            }
            pressure.on_enqueued();
        }
    }

    Err("subscriber closed".into())
}

/// Poll the QueueState channel and update the shared resource
pub fn collect_queue_state(
    receiver: Option<ResMut<QueueStateReceiver>>,
    mut data: ResMut<QueueStateData>,
    mut backpressure: ResMut<BackpressureMetrics>,
) {
    let Some(mut receiver) = receiver else { return };

    backpressure.queue_state.record_queue_depth(receiver.0.len());

    // Drain channel, keep only the latest state
    while let Ok(state) = receiver.0.try_recv() {
        data.pending = state.pending;
        data.total = state.total;
        data.robots_online = state.robots_online;
    }
}
