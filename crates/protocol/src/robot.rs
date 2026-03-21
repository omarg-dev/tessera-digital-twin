//! Robot telemetry types shared across firmware, coordinator, scheduler, and renderer.

use serde::{Deserialize, Serialize};

/// Robot state broadcast over Zenoh (firmware → coordinator, renderer, scheduler)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RobotUpdate {
    pub id: u32,
    /// [x, y, z] world coordinates
    pub position: [f32; 3],
    /// Current velocity in world units per second
    pub velocity: [f32; 3],
    pub state: RobotState,
    /// Battery percentage in range 0.0..=100.0
    pub battery: f32,
    pub carrying_cargo: Option<u32>,
    /// Home charging station location
    pub station_position: [f32; 3],
    /// Whether this robot can accept and execute commands
    pub enabled: bool,
}

/// Runtime robot states produced by firmware and consumed by higher layers.
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
    /// Simulation tick for receiver-side ordering and de-duplication
    pub tick: u64,
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

/// WHCA runtime metrics telemetry broadcast by coordinator for analytics UI.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct WhcaMetricsTelemetry {
    /// Reporting window size in seconds for delta fields.
    pub window_secs: u64,
    pub searches_total: u64,
    pub searches_succeeded: u64,
    pub searches_failed: u64,
    pub nodes_expanded_total: u64,
    pub reservation_probe_calls_total: u64,
    pub edge_collision_checks_total: u64,
    pub wait_actions_added_total: u64,
    pub avg_search_time_us: u64,
    pub last_search_time_us: u64,
    pub open_set_peak_observed: u64,
    pub reservation_entries_peak: u64,
}
