use bevy::prelude::*;
use protocol::RobotState;

/// Robot entity - visual representation of a robot in the warehouse
#[derive(Component)]
pub struct Robot {
    pub id: u32,
    pub state: RobotState,
    pub position: Vec3,
    pub battery: f32,
    /// TODO: Wire to task assignment display in dashboard
    #[allow(dead_code)]
    pub current_task: Option<u32>,
    pub carrying_cargo: Option<u32>,
}

/// Ground tile marker
#[derive(Component)]
pub struct Ground;

/// Wall tile marker
#[derive(Component)]
pub struct Wall;

/// Shelf tile with storage capacity
#[derive(Component)]
pub struct Shelf {
    /// TODO: Display capacity in shelf tooltip/UI
    #[allow(dead_code)]
    pub capacity: u32,
}

/// Charging station marker
#[derive(Component)]
pub struct Station;

/// Dropoff zone marker
#[derive(Component)]
pub struct Dropoff;
