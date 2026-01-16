use serde::{Deserialize, Serialize};
use bevy::prelude::Vec3;

// This is the packet of data that travels over "The Air" (Zenoh)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RobotUpdate {
    pub id: u32,
    pub position: Vec3,
    pub state: RobotState,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RobotState {
    Idle,
    Moving,
    Charging,
}