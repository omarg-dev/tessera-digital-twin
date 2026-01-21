//! Command types sent between crates

use serde::{Deserialize, Serialize};

/// Path command from fleet_server to a specific robot
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathCmd {
    pub robot_id: u32,
    pub command: PathCommand,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PathCommand {
    /// Move to target position with given velocity
    MoveTo { target: [f32; 3], speed: f32 },
    /// Stop immediately
    Stop,
    /// Pick up cargo at current location
    Pickup { cargo_id: u32 },
    /// Drop cargo at current location
    Drop,
    /// Go to charging station
    ReturnToCharge,
}

/// System-wide control commands (control_plane → all)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SystemCommand {
    Pause,
    Resume,
}
