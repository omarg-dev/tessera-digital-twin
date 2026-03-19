//! Visualizer systems - Digital Twin Command Center
//! Robots are spawned dynamically by sync_robots when new IDs appear.

pub mod models;
pub mod populate_scene;
pub mod camera;
pub mod sync_robots;
pub mod commands;
pub mod command_bridge;
pub mod receivers;
pub mod outline;
pub mod draw_paths;
pub mod interpolate_robots;
pub mod robot_labels;
pub mod luminance_hierarchy;
pub mod congestion_overlays;
pub mod material_diagnostics;
pub mod robot_cargo_visibility;
