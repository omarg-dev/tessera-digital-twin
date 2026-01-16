mod components;
mod resources;
mod systems;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
// use resources::WarehouseStats;
use systems::{
    camera::{spawn_camera, camera_controls},
    populate_scene::{populate_environment, populate_lighting},
    spawn_robot::spawn_robot,
    zenoh_receiver::{setup_zenoh_receiver, collect_robot_updates},
    sync_robots::sync_robots,
    index_robots::{build_robot_index, index_new_robots},
    dashboard::debug_hud,
};

fn main() {
    println!("Starting Hyper-Twin Warehouse Simulation...");
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        // .init_resource::<WarehouseStats>()
        .init_resource::<resources::RobotIndex>()
        .init_resource::<resources::DebugHUD>()
        .init_resource::<resources::RobotLastPositions>()
        .add_systems(Startup, (
            populate_environment,
            populate_lighting,
            spawn_camera,
            spawn_robot,
            setup_zenoh_receiver,
            build_robot_index
        ))
        .add_systems(Update, (
            index_new_robots,
            collect_robot_updates,
            debug_hud,
            sync_robots,
            camera_controls
        )).run();
}
