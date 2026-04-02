//! Subscribe to coordinator path telemetry and update ActivePaths resource.

use bevy::prelude::*;
use protocol::config::visualizer::{network as net_cfg, path::PATH_Y_OFFSET};
use protocol::{RobotPathTelemetry, topics};
use serde_json::from_slice;
use std::collections::HashMap;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{ActivePaths, BackpressureMetrics, PathTelemetryReceiver, RenderPerfCounters, ZenohSession};

/// Initialize background Zenoh subscriber for path telemetry
pub fn setup_path_listener(
    mut commands: Commands,
    session: Res<ZenohSession>,
    backpressure: Res<BackpressureMetrics>,
) {
    let (tx, rx) = mpsc::channel::<RobotPathTelemetry>(net_cfg::PATH_TELEMETRY_CHANNEL_CAPACITY);
    let sess = session.session.clone();
    let pressure = backpressure.path_telemetry.handle();

    session.runtime.spawn(async move {
        if let Err(e) = run_path_listener(sess, tx, pressure).await {
            eprintln!("Path telemetry listener exited: {}", e);
        }
    });

    commands.insert_resource(PathTelemetryReceiver(rx));
}

async fn run_path_listener(
    session: Session,
    tx: mpsc::Sender<RobotPathTelemetry>,
    pressure: crate::resources::ChannelBackpressureHandle,
) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::TELEMETRY_PATHS)
        .await
        .map_err(|e| format!("declare path telemetry subscriber: {}", e))?;

    let mut coalesced: HashMap<u32, RobotPathTelemetry> = HashMap::new();
    let mut last_flush = std::time::Instant::now();
    let flush_window = std::time::Duration::from_millis(net_cfg::PATH_TELEMETRY_COALESCE_WINDOW_MS);

    while let Ok(sample) = subscriber.recv_async().await {
        if let Ok(telemetry) = from_slice::<RobotPathTelemetry>(&sample.payload().to_bytes()) {
            pressure.on_received();
            coalesced.insert(telemetry.robot_id, telemetry);

            if coalesced.len() >= net_cfg::PATH_TELEMETRY_COALESCE_FLUSH_MAX
                || last_flush.elapsed() >= flush_window
            {
                flush_coalesced(&tx, &mut coalesced, &pressure)?;
                last_flush = std::time::Instant::now();
            }
        }
    }

    Err("subscriber closed".into())
}

fn flush_coalesced(
    tx: &mpsc::Sender<RobotPathTelemetry>,
    coalesced: &mut HashMap<u32, RobotPathTelemetry>,
    pressure: &crate::resources::ChannelBackpressureHandle,
) -> Result<(), String> {
    for (_, telemetry) in coalesced.drain() {
        match tx.try_send(telemetry) {
            Ok(()) => pressure.on_enqueued(),
            Err(mpsc::error::TrySendError::Full(_)) => pressure.on_dropped_full(),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                pressure.on_blocked_send();
                return Err("path telemetry receiver dropped".into());
            }
        }
    }
    Ok(())
}

/// Drain the path telemetry channel and update the ActivePaths resource.
/// Empty waypoint vectors clear the robot's entry from the map.
pub fn collect_path_telemetry(
    receiver: Option<ResMut<PathTelemetryReceiver>>,
    mut active_paths: ResMut<ActivePaths>,
    mut perf: ResMut<RenderPerfCounters>,
    mut backpressure: ResMut<BackpressureMetrics>,
) {
    perf.path_telemetry_messages_processed = 0;
    perf.path_telemetry_unique_robots = 0;
    perf.path_telemetry_total_waypoints = 0;
    perf.path_telemetry_max_waypoints_single = 0;

    let Some(mut receiver) = receiver else { return };

    backpressure
        .path_telemetry
        .record_queue_depth(receiver.0.len());

    let mut latest_by_robot: HashMap<u32, RobotPathTelemetry> = HashMap::new();
    let mut total_waypoints = 0_u32;
    let mut max_waypoints = 0_u32;
    let mut messages = 0_u32;

    while let Ok(telemetry) = receiver.0.try_recv() {
        messages += 1;
        let waypoints = telemetry.waypoints.len() as u32;
        total_waypoints = total_waypoints.saturating_add(waypoints);
        max_waypoints = max_waypoints.max(waypoints);
        latest_by_robot.insert(telemetry.robot_id, telemetry);
    }

    for telemetry in latest_by_robot.into_values() {
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

    perf.path_telemetry_messages_processed = messages;
    perf.path_telemetry_unique_robots = active_paths.0.len() as u32;
    perf.path_telemetry_total_waypoints = total_waypoints;
    perf.path_telemetry_max_waypoints_single = max_waypoints;
}
