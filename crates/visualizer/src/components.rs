use bevy::prelude::*;
use protocol::RobotState;

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

#[derive(Component)]
pub struct Ground {

}

#[derive(Component)]
pub struct Wall {

}

#[derive(Component)]
pub struct Shelf {
    /// TODO: Display capacity in shelf tooltip/UI
    #[allow(dead_code)]
    pub capacity: u32,
}

#[derive(Component)]
pub struct Station {
    
}

#[derive(Component)]
pub struct Dropoff {
    
}
