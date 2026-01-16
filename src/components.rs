use bevy::prelude::*;

#[derive(Component)]
pub struct Robot {
    // pub cargo_capacity: u32,
    // pub battery_level: f32,
}

#[derive(Component)]
pub struct Ground {

}

#[derive(Component)]
pub struct Wall {

}

#[derive(Component)]
pub struct Shelf {
    pub _capacity: u32,
}

#[derive(Component)]
pub struct Station {
    
}

#[derive(Component)]
pub struct Dropoff {
    
}