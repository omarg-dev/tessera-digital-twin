//! Subscribe to coordinator WHCA metrics telemetry via Zenoh.

use bevy::prelude::*;
use protocol::{WhcaMetricsTelemetry, topics};
use serde_json::from_slice;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{WhcaMetricsData, WhcaMetricsReceiver, ZenohSession};

/// Initialize background Zenoh subscriber for WHCA metrics telemetry.
pub fn setup_whca_metrics_listener(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<WhcaMetricsTelemetry>(16);
    let sess = session.session.clone();

    session.runtime.spawn(async move {
        if let Err(e) = run_whca_metrics_listener(sess, tx).await {
            eprintln!("WHCA metrics listener exited: {}", e);
        }
    });

    commands.insert_resource(WhcaMetricsReceiver(rx));
}

async fn run_whca_metrics_listener(
    session: Session,
    tx: mpsc::Sender<WhcaMetricsTelemetry>,
) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::TELEMETRY_WHCA_METRICS)
        .await
        .map_err(|e| format!("declare WHCA metrics subscriber: {}", e))?;

    while let Ok(sample) = subscriber.recv_async().await {
        if let Ok(metrics) = from_slice::<WhcaMetricsTelemetry>(&sample.payload().to_bytes()) {
            tx.send(metrics).await.ok();
        }
    }

    Err("subscriber closed".into())
}

/// Poll WHCA metrics channel and update shared UI resource.
pub fn collect_whca_metrics(
    receiver: Option<ResMut<WhcaMetricsReceiver>>,
    mut data: ResMut<WhcaMetricsData>,
    time: Res<Time>,
) {
    let Some(mut receiver) = receiver else { return };

    while let Ok(metrics) = receiver.0.try_recv() {
        data.latest = Some(metrics);
        data.last_updated_secs = time.elapsed_secs_f64();
    }
}
