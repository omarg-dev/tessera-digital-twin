//! Hyper-Twin Visualizer - Digital Twin Command Center
//!
//! This crate only visualizes the warehouse and robots.
//! All physics and logic happen in firmware and coordinator.
//! We subscribe to RobotUpdateBatch from firmware and render.
//!
//! ## Phase 5 UI Features
//! - [x] 4-panel egui layout (top HUD, left object list, right inspector, bottom logs)
//! - [x] Simulation controls (pause/play, speed 1x/10x/Max) → Zenoh broadcast
//! - [x] Layer toggles (Paths, Heatmap, IDs)
//! - [x] Robot/Shelf object browser with search
//! - [x] Context-sensitive inspector (battery bar, state, actions)
//! - [x] Robot control buttons (Kill/Restart/Enable) → Zenoh broadcast
//! - [x] Live task queue display from scheduler QueueState
//! - [x] Log console with auto-scroll (state changes, commands, UI actions)
//!
//! ## TODO: Future Improvements
//! - [ ] Keyboard shortcuts: P=pause, R=resume, Space=reset, Esc=quit
//! - [ ] Robot selection: click robot in 3D viewport to select
//! - [ ] 3D gizmos: path trails, heatmap overlay, debug grid
//! - [ ] Analytics dashboard (throughput graph, battery histograms)
//! - [ ] Timeline scrubber: replay simulation history

mod components;
mod resources;
mod systems;
mod ui;

#[cfg(test)]
mod tests;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use systems::{
    camera::{spawn_camera, camera_controls, camera_follow_selected},
    populate_scene::{populate_environment, populate_lighting, check_reload_environment},
    zenoh_receiver::{setup_zenoh_receiver, collect_robot_updates},
    sync_robots::sync_robots,
    commands::{setup_system_listener, setup_publishers, handle_system_commands, bridge_ui_commands},
    queue_receiver::{setup_queue_listener, collect_queue_state},
};

fn main() {
    println!("╔════════════════════════════════════════════╗");
    println!("║     VISUALIZER - Digital Twin Center       ║");
    println!("╚════════════════════════════════════════════╝");
    let session = resources::open_zenoh_session();

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        // Resources
        .insert_resource(session)
        .init_resource::<resources::RobotIndex>()
        .init_resource::<resources::RobotLastPositions>()
        .init_resource::<resources::UiState>()
        .init_resource::<resources::LogBuffer>()
        .init_resource::<resources::QueueStateData>()
        // Events
        .add_message::<resources::UiAction>()
        // Startup: scene, camera, Zenoh subscribers & publishers
        .add_systems(Startup, (
            populate_environment,
            populate_lighting,
            spawn_camera,
            setup_zenoh_receiver,
            setup_system_listener,
            setup_publishers,
            setup_queue_listener,
        ))
        // Update: poll Zenoh channels, sync state, bridge UI commands
        .add_systems(Update, (
            collect_robot_updates,
            sync_robots,
            collect_queue_state,
            handle_system_commands,
            bridge_ui_commands,
            camera_follow_selected,
            camera_controls,
        ))
        // UI runs inside the egui context pass (after Update, before rendering)
        .add_systems(EguiPrimaryContextPass, ui::draw_ui)
        // Run environment reload in PostUpdate to ensure despawn commands are applied first
        .add_systems(PostUpdate, check_reload_environment)
        .run();
}
