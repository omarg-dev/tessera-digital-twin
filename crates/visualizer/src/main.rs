//! Hyper-Twin Visualizer - Render-Only Layer
//!
//! This crate only visualizes the warehouse and robots.
//! All physics and logic happen in swarm_driver and fleet_server.
//! We subscribe to RobotUpdateBatch from swarm_driver and render.

mod components;
mod resources;
mod systems;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use systems::{
    camera::{spawn_camera, camera_controls},
    populate_scene::{populate_environment, populate_lighting, check_reload_environment},
    zenoh_receiver::{setup_zenoh_receiver, collect_robot_updates},
    sync_robots::sync_robots,
    dashboard::debug_hud,
    commands::{setup_system_listener, handle_system_commands},
};

fn main() {
    println!("Starting Hyper-Twin Visualizer (Render-Only Layer)...");
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .init_resource::<resources::RobotIndex>()
        .init_resource::<resources::DebugHUD>()
        .init_resource::<resources::RobotLastPositions>()
        .add_systems(Startup, (
            populate_environment,
            populate_lighting,
            spawn_camera,
            setup_zenoh_receiver,
            setup_system_listener,
        ))
        .add_systems(Update, (
            collect_robot_updates,
            sync_robots,
            handle_system_commands,
            check_reload_environment,
            debug_hud,
            camera_controls,
        ))
        .run();
}

