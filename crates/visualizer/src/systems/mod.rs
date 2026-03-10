//! Visualizer systems - Digital Twin Command Center
//! Robots are spawned dynamically by sync_robots when new IDs appear.

pub mod models;
pub mod populate_scene;
pub mod camera;
pub mod sync_robots;
pub mod zenoh_receiver;
pub mod commands;
pub mod queue_receiver;
pub mod task_receiver;
pub mod outline;
pub mod path_receiver;
pub mod draw_paths;
pub mod interpolate_robots;
pub mod robot_labels;
