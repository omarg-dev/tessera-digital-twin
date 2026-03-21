//! Subscribe to coordinator path telemetry and update ActivePaths resource.

use bevy::prelude::*;
use protocol::config::visual::path::PATH_Y_OFFSET;
use protocol::{RobotPathTelemetry, topics};
use serde_json::from_slice;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{ActivePaths, PathTelemetryReceiver, ZenohSession};

/// Initialize background Zenoh subscriber for path telemetry
pub fn setup_path_listener(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<RobotPathTelemetry>(64);
    let sess = session.session.clone();

    session.runtime.spawn(async move {
        if let Err(e) = run_path_listener(sess, tx).await {
            eprintln!("Path telemetry listener exited: {}", e);
        }
    });

    commands.insert_resource(PathTelemetryReceiver(rx));
}

async fn run_path_listener(
    session: Session,
    tx: mpsc::Sender<RobotPathTelemetry>,
) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::TELEMETRY_PATHS)
        .await
        .map_err(|e| format!("declare path telemetry subscriber: {}", e))?;

    while let Ok(sample) = subscriber.recv_async().await {
        if let Ok(telemetry) = from_slice::<RobotPathTelemetry>(&sample.payload().to_bytes()) {
            if tx.send(telemetry).await.is_err() {
                return Err("path telemetry receiver dropped".into());
            }
        }
    }

    Err("subscriber closed".into())
}

/// Drain the path telemetry channel and update the ActivePaths resource.
/// Empty waypoint vectors clear the robot's entry from the map.
pub fn collect_path_telemetry(
    receiver: Option<ResMut<PathTelemetryReceiver>>,
    mut active_paths: ResMut<ActivePaths>,
) {
    let Some(mut receiver) = receiver else { return };

    while let Ok(telemetry) = receiver.0.try_recv() {
        if telemetry.waypoints.is_empty() {
            active_paths.0.remove(&telemetry.robot_id);
        } else {
            // convert [x, y, z] network coords to bevy Vec3 above the floor
            let points: Vec<Vec3> = telemetry
                .waypoints
                .iter()
                .map(|&[x, _y, z]| Vec3::new(x, PATH_Y_OFFSET, z))
                .collect();
            active_paths.0.insert(telemetry.robot_id, points);
        }
    }
}
