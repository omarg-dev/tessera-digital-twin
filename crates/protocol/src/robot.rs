//! Robot state types broadcast over Zenoh

use serde::{Deserialize, Serialize};

/// Robot state broadcast over Zenoh (firmware → coordinator, renderer, scheduler)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RobotUpdate {
    pub id: u32,
    pub position: [f32; 3],    // [x, y, z] world coordinates
    pub velocity: [f32; 3],    // Current velocity
    pub state: RobotState,
    pub battery: f32,          // 0.0 to 100.0
    pub carrying_cargo: Option<u32>,
    pub station_position: [f32; 3],  // Home charging station location
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RobotState {
    Idle,
    MovingToPickup,
    Picking,
    MovingToDrop,
    MovingToStation,
    LowBattery,
    Charging,
    Blocked,
    Faulted,
}

/// Batch of robot updates for efficient network transmission
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RobotUpdateBatch {
    pub updates: Vec<RobotUpdate>,
    pub tick: u64,  // Simulation tick for ordering
}

/// Path telemetry broadcast by coordinator for visualization.
/// Contains remaining waypoints from the robot's current position to its destination.
/// An empty `waypoints` vector signals path cleared (task done/failed/charging).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RobotPathTelemetry {
    pub robot_id: u32,
    /// Remaining waypoints in world coordinates [x, y, z]
    pub waypoints: Vec<[f32; 3]>,
}
