//! Zenoh topic constants - Single source of truth for all topic strings
//!
//! Topics are organized by data flow direction. Abstraction layers (top to bottom):
//! - **orchestrator** - Process management and global commands (orchestrator crate)
//! - **renderer** - Visualization layer (visualizer crate)
//! - **scheduler** - Task queue and robot allocation (scheduler crate)
//! - **coordinator** - Path planning and task execution (fleet_server crate)
//! - **firmware** - Robot physics simulation (mock_firmware crate)

/// Robots broadcast their state here (firmware → coordinator, renderer, scheduler)
pub const ROBOT_UPDATES: &str = "factory/robots";

/// Coordinator sends path commands here (coordinator → firmware)
pub const PATH_COMMANDS: &str = "factory/commands";

/// Orchestrator broadcasts pause/resume/reset (orchestrator → all layers)
pub const ADMIN_CONTROL: &str = "factory/admin/control";

/// Map hash validation on startup (coordinator → all layers)
pub const MAP_VALIDATION: &str = "factory/admin/map_hash";

// ============ Task/Mission Topics ============

/// New task requests (external systems → scheduler)
pub const TASK_REQUESTS: &str = "factory/tasks/requests";

/// Task assignments (scheduler → coordinator)
pub const TASK_ASSIGNMENTS: &str = "factory/tasks/assignments";

/// Task status updates (coordinator → scheduler)
pub const TASK_STATUS: &str = "factory/tasks/status";

/// Queue state broadcast for monitoring (scheduler → renderer)
pub const QUEUE_STATE: &str = "factory/tasks/queue";

// ============ Sender Identifiers ============
// Used in MapValidation.sender to identify the source of broadcasts

/// Sender identifier for coordinator layer (fleet_server)
pub const SENDER_COORDINATOR: &str = "coordinator";
