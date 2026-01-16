mod components;
mod resources;
mod systems;

use bevy::prelude::*;
// use resources::WarehouseStats;
use systems::{
    camera::{spawn_camera, camera_controls},
    movement::move_robots,
    setup_scene::{setup_environment, setup_lighting},
    spawn_robot::spawn_robot,
};

fn main() {
    println!("Starting Hyper-Twin Warehouse Simulation...");
    App::new()
        .add_plugins(DefaultPlugins)
        // .add_plugins(EguiPlugin::default())
        // .init_resource::<WarehouseStats>()
        .add_systems(Startup, (setup_environment, setup_lighting, spawn_camera, spawn_robot))
        .add_systems(Update, (move_robots, camera_controls))
        .run();
}
