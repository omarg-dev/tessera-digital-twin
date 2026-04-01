//! # Protocol Crate - Shared Types for Hyper-Twin
//!
//! This crate defines all types used for inter-layer communication via Zenoh.
//! It ensures all layers agree on message formats and configuration constants.
//!
//! ## Abstraction Layers
//!
//! The system is organized into abstraction layers (top to bottom):
//!
//! | Layer                 | Crate             | Responsibility                            |
//! |-----------------------|-------------------|-------------------------------------------|
//! | **Orchestrator**      | `orchestrator`    | Process management, pause/resume, reset   |
//! | **Renderer**          | `visualizer`      | 3D visualization, HUD, camera controls    |
//! | **Scheduler**         | `scheduler`       | Task queue, robot allocation, management  |
//! | **Coordinator**       | `coordinator`     | Path planning, task execution, A* routing |
//! | **Firmware**          | `mock_firmware`   | Robot physics, battery, movement          |
//! 
//! ## Modules
//!
//! - [`commands`] - Path commands (MoveTo, Stop) and system commands (Pause, Resume, Verbose)
//! - [`config`] - Central configuration constants (firmware, coordinator, scheduler, visualizer)
//! - [`grid_map`] - Warehouse map parsing and tile types
//! - [`layout`] - Runtime layout selector and path resolution helpers
//! - [`robot`] - Robot state updates broadcast over Zenoh
//! - [`tasks`] - Task definitions for inter-layer communication
//! - [`topics`] - Zenoh topic string constants
//!
//! ## Dependencies
//!
//! This crate has minimal dependencies (`serde`, `chrono`, `rand`) to keep it lightweight.
//! All other crates depend on this one for shared types.


pub mod commands;
pub mod config;
pub mod grid_map;
pub mod layout;
pub mod robot;
pub mod tasks;
pub mod topics;
pub mod logs;
pub mod chaos;
pub mod util;
pub mod publish;

// Re-export for convenience
pub use commands::{PathCmd, PathCommand, RobotControl, SystemCommand, SystemCommandEffect, CommandResponse, CommandStatus};
pub use layout::{
	LAYOUT_FILE_EXTENSION,
	LAYOUT_FILE_PATH,
	LAYOUT_OVERRIDE_ENV,
	LAYOUTS_DIR,
	LayoutEntry,
	discover_layout_entries,
	layout_path_from_selector,
	resolve_layout_path,
};
pub use grid_map::{GridMap, MapValidation, ShelfInventory, Tile, TileType};
pub use robot::{RobotPathTelemetry, RobotState, RobotUpdate, RobotUpdateBatch, WhcaMetricsTelemetry};
pub use tasks::{Priority, QueueState, Task, TaskAssignment, TaskCommand, TaskId, TaskListSnapshot, TaskRequest, TaskStatus, TaskStatusUpdate, TaskType, task_status_label};
pub use logs::{timestamp, save_log};
pub use publish::{publish_json_logged, publish_json_logged_sync};
pub use util::{
	distance_sq_xz,
	distance_xz,
	grid_to_world,
	is_finite_position,
	is_reachable_on_map,
	manhattan_distance_xz,
	world_to_grid,
};