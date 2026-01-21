//! Protocol crate - Shared types for Zenoh communication
//! All types used for inter-crate communication are defined here.
//! This ensures all crates agree on message formats.

pub mod commands;
pub mod config;
pub mod grid_map;
pub mod robot;
pub mod tasks;
pub mod topics;

// Re-export for convenience
pub use commands::{PathCmd, PathCommand, SystemCommand};
pub use config::LAYOUT_FILE_PATH;
pub use grid_map::{GridMap, MapValidation, Tile, TileType};
pub use robot::{RobotState, RobotUpdate, RobotUpdateBatch};
pub use tasks::{Priority, Task, TaskAssignment, TaskId, TaskRequest, TaskStatus, TaskStatusUpdate, TaskType};
