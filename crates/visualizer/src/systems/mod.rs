//! Visualizer systems - RENDER ONLY
//! No physics, no task logic, no pathfinding.
//! Robots are spawned dynamically by sync_robots when new IDs appear.

pub mod populate_scene;
pub mod camera;
pub mod sync_robots;
pub mod zenoh_receiver;
pub mod commands;
