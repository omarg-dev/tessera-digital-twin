//! Resources for the Visualizer crate
//! 
//! This is a RENDER-ONLY layer. No physics, no task logic.
//! We only subscribe to RobotUpdateBatch from firmware and display.

use bevy::prelude::*;
use protocol::RobotUpdate;
use std::collections::HashMap;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zenoh::Session;

/// Shared Zenoh session for all visualizer subscribers
#[derive(Resource, Clone)]
pub struct ZenohSession(pub Session);

/// Open a single Zenoh session for the visualizer (blocking startup).
///
/// This avoids multiple sessions per process and keeps the visualizer lean.
pub fn open_zenoh_session() -> ZenohSession {
    let rt = Runtime::new().expect("Failed to create Tokio runtime for Zenoh session");
    let session = rt.block_on(async {
        zenoh::open(zenoh::Config::default())
            .await
            .expect("Failed to open Zenoh session")
    });

    ZenohSession(session)
}

/// Receives robot updates from Zenoh (firmware publishes, we display)
#[derive(Resource)]
pub struct ZenohReceiver(pub mpsc::Receiver<RobotUpdate>);

/// Stores latest robot updates for systems to consume
#[derive(Resource, Default)]
pub struct RobotUpdates {
    pub updates: Vec<RobotUpdate>,
}

/// Fast lookup for robot entities by ID
#[derive(Resource, Default)]
pub struct RobotIndex {
    pub by_id: HashMap<u32, Entity>,
}

/// Tracks the last seen position for each robot (for movement detection in zenoh_receiver)
/// Prevents processing duplicate updates when robot hasn't moved
#[derive(Resource, Default)]
pub struct RobotLastPositions {
    pub by_id: HashMap<u32, [f32; 3]>,
}
