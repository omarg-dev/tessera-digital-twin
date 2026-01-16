use serde::{Deserialize, Serialize};

// This is the packet of data that travels over "The Air" (Zenoh)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RobotUpdate {
    pub id: u32,
    pub position: [f32; 3], // [x, y, z]
    pub state: RobotState,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RobotState {
    Idle,
    Moving,
    Charging,
}
