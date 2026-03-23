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
//! - [x] Robot selection: click robot in 3D viewport to select
//!
//! ## TODO: Future Improvements
//! - [x] 3D gizmos: path trails
//! - [ ] 3D gizmos: heatmap overlay
//! - [ ] 3D gizmos: debug grid
//! - [ ] Analytics dashboard (throughput graph, battery histograms)
//! - [ ] Timeline scrubber: replay simulation history
//! - [ ] Keyboard shortcuts: P=pause, R=resume, Space=reset, Esc=quit

mod components;
mod resources;
mod systems;
mod ui;

#[cfg(test)]
mod tests;

use bevy::asset::AssetPlugin;
use bevy::ecs::schedule::common_conditions::resource_exists;
use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use bevy_mod_outline::{OutlinePlugin, AutoGenerateOutlineNormalsPlugin};
use protocol::config::visualizer::lighting;
use systems::{
    camera::{spawn_camera, camera_controls, camera_follow_selected, camera_follow_task, update_bloom_settings, apply_camera_preset, record_snapshot_markers},
    populate_scene::{populate_environment, populate_lighting, check_reload_environment, sync_shelf_boxes, propagate_tile_optimizations},
    receivers::{
        robot_updates::{setup_zenoh_receiver, collect_robot_updates},
        queue_state::{setup_queue_listener, collect_queue_state},
        task_list::{setup_task_listener, collect_task_list, sync_robot_tasks},
        path_telemetry::{setup_path_listener, collect_path_telemetry},
        whca_metrics::{setup_whca_metrics_listener, collect_whca_metrics},
    },
    sync_robots::sync_robots,
    interpolate_robots::interpolate_robots,
    commands::{setup_system_listener, handle_system_commands},
    command_bridge::{setup_publishers, bridge_ui_commands},
    outline::{on_pointer_over, on_pointer_out, on_pointer_click, sync_programmatic_outlines},
    draw_paths::{configure_gizmos, draw_robot_paths},
    luminance_hierarchy::{apply_luminance_hierarchy, LuminanceMaterialState},
    material_diagnostics::{diagnose_imported_materials, MaterialDiagnosticsState},
    sync_robot_cargo::sync_robot_cargo,
    congestion_overlays::{reset_render_perf_counters, update_congestion_overlay_data, draw_congestion_overlays},
    robot_labels::draw_robot_labels,
};

fn main() {
    println!("╔════════════════════════════════════════════╗");
    println!("║     VISUALIZER - Digital Twin Center       ║");
    println!("╚════════════════════════════════════════════╝");
    let session = resources::open_zenoh_session();

    // resolve workspace-root assets/ regardless of working directory
    let assets_dir = std::env::current_exe()
        .ok()
        .and_then(|p| {
            let mut dir = p.parent()?.to_path_buf();
            // walk up until we find the assets/ folder (handles target/debug, target/release, etc.)
            for _ in 0..10 {
                if dir.join("assets").is_dir() {
                    return Some(dir.join("assets").to_string_lossy().into_owned());
                }
                dir = dir.parent()?.to_path_buf();
            }
            None
        })
        .unwrap_or_else(|| "assets".to_string());

    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: assets_dir,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(MeshPickingPlugin)
        .add_plugins((OutlinePlugin, AutoGenerateOutlineNormalsPlugin::default()))
        // Outline interaction observers (hover, out, click)
        .add_observer(on_pointer_over)
        .add_observer(on_pointer_out)
        .add_observer(on_pointer_click)
        // Resources
        .insert_resource(session)
        .insert_resource(ClearColor(Color::srgb(
            lighting::BACKGROUND_COLOR.0,
            lighting::BACKGROUND_COLOR.1,
            lighting::BACKGROUND_COLOR.2,
        )))
        .init_resource::<resources::RobotIndex>()
        .init_resource::<resources::RobotLastPositions>()
        .init_resource::<resources::UiState>()
        .init_resource::<resources::VisualTuning>()
        .init_resource::<resources::LogBuffer>()
        .init_resource::<resources::QueueStateData>()
        .init_resource::<resources::TaskListData>()
        .init_resource::<resources::ActivePaths>()
        .init_resource::<resources::WhcaMetricsData>()
        .init_resource::<resources::RenderPerfCounters>()
        .init_resource::<resources::UiAnalyticsView>()
        .init_resource::<resources::UiFrameInputs>()
        .init_resource::<resources::CongestionOverlayData>()
        .init_resource::<resources::ScreenshotHarness>()
        .init_resource::<MaterialDiagnosticsState>()
        .init_resource::<LuminanceMaterialState>()
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
            setup_task_listener,
            setup_path_listener,
            setup_whca_metrics_listener,
            configure_gizmos,
        ))
        // Update: poll Zenoh channels, sync state, bridge UI commands
        .add_systems(Update, (
            reset_render_perf_counters,
            collect_robot_updates,
            sync_robots,
            interpolate_robots.after(sync_robots),
            collect_queue_state,
            collect_task_list,
            sync_robot_tasks.after(collect_task_list),
            collect_path_telemetry,
            update_congestion_overlay_data,
            collect_whca_metrics,
            handle_system_commands,
            bridge_ui_commands,
            update_bloom_settings.after(bridge_ui_commands),
            apply_camera_preset.after(camera_controls),
            record_snapshot_markers,
            camera_follow_selected,
            camera_follow_task.after(camera_follow_selected),
            camera_controls,
            draw_robot_paths,
            draw_congestion_overlays.after(draw_robot_paths),
        ))
        .add_systems(Update, (
            ui::sync_ui_frame_inputs,
            ui::sync_ui_analytics_view,
        ))
        // UI runs inside the egui context pass (after Update, before rendering)
        // labels use Order::Background so they render behind panels
        .add_systems(EguiPrimaryContextPass, draw_robot_labels)
        .add_systems(EguiPrimaryContextPass, ui::draw_ui)
        // PostUpdate: runs after EguiPrimaryContextPass so outline sync sees hovered_entity from draw_ui
        .add_systems(PostUpdate, (
            check_reload_environment.run_if(resource_exists::<systems::commands::ReloadEnvironment>),
            sync_shelf_boxes,
            sync_robot_cargo,
            sync_programmatic_outlines,
            propagate_tile_optimizations,
            diagnose_imported_materials,
            apply_luminance_hierarchy,
        ))
        .run();
}
