//! Resources for the Visualizer crate
//! 
//! This is a RENDER-ONLY layer. No physics, no task logic.
//! We only subscribe to RobotUpdateBatch from firmware and display.

use bevy::prelude::*;
use protocol::RobotUpdate;
use tokio::sync::mpsc;
use std::collections::HashMap;

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

/// Debug HUD data for UI overlay
#[derive(Resource, Default)]
pub struct DebugHUD {
    pub last_message: Option<String>,
    // TODO: Wire this into the Bevy UI to render a visible "paused" overlay.
    pub _paused: bool,            // Show pause overlay when system paused.
    // TODO: Display this count in the HUD once robot connection tracking is implemented.
    pub _connected_robots: u32,   // Count from coordinator.
}