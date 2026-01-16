use bevy::prelude::*;
use protocol::RobotUpdate;
use tokio::sync::mpsc;
use std::collections::HashMap;

// #[derive(Resource, Default)]
// pub struct WarehouseStats {
//     pub package_throughput: u32,
//     pub active_robots: u32,
//     pub elapsed_seconds: f32,
// }

/// Receives robot updates from Zenoh backend
#[derive(Resource)]
pub struct ZenohReceiver(pub mpsc::Receiver<RobotUpdate>);

/// Stores latest robot updates for systems to consume
#[derive(Resource, Default)]
pub struct RobotUpdates {
    pub updates: Vec<RobotUpdate>,
}

#[derive(Resource)]
pub struct TileMap {
    pub tiles: Vec<(Entity, usize, usize)>, // (entity, x, y)
    pub width: usize,
    pub height: usize,
}

/// Fast lookup for robots by `id`
#[derive(Resource, Default)]
pub struct RobotIndex {
    pub by_id: HashMap<u32, Entity>,
}

/// Tracks the last seen position for each robot id
#[derive(Resource, Default)]
pub struct RobotLastPositions {
    pub by_id: HashMap<u32, [f32; 3]>,
}

/// Debug HUD data for UI display (latest status line)
#[derive(Resource, Default)]
pub struct DebugHUD {
    pub last_message: Option<String>,
}