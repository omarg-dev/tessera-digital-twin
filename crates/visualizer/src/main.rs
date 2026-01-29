//! Hyper-Twin Visualizer - Render-Only Layer
//!
//! This crate only visualizes the warehouse and robots.
//! All physics and logic happen in firmware and coordinator.
//! We subscribe to RobotUpdateBatch from firmware and render.
//!
//! ## TODO: UI Improvements (Phase 5+)
//! - [ ] Integrated control panel (egui sidebar) for pause/resume/reset
//! - [ ] Real-time dashboard: robot count, task queue depth, system state
//! - [ ] Keyboard shortcuts: P=pause, R=resume, Space=reset, Esc=quit
//! - [ ] Robot selection: click robot to show details, assigned task, path
//! - [ ] Task visualization: show pickup/dropoff markers, path preview
//! - [ ] Heatmap overlay: show traffic density, congestion zones
//! - [ ] Timeline scrubber: replay simulation history

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
            // handle_system_commands despawns entities, check_reload_environment respawns
            // They must run in separate frames to allow Bevy's deferred commands to apply
            handle_system_commands,
            camera_controls,
        ))
        // Run environment reload in PostUpdate to ensure despawn commands are applied first
        .add_systems(PostUpdate, check_reload_environment)
        .add_systems(Update, debug_hud)
        .run();
}

