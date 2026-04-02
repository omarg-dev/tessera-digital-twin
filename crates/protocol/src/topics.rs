//! Zenoh topic constants - Single source of truth for all topic strings
//!
//! Topics are organized by data flow direction. Abstraction layers (top to bottom):
//! - **orchestrator** - Process management and global commands (orchestrator crate)
//! - **renderer** - Visualization layer (visualizer crate)
//! - **scheduler** - Task queue and robot allocation (scheduler crate)
//! - **coordinator** - Path planning and task execution (coordinator crate)
//! - **firmware** - Robot physics simulation (mock_firmware crate)

/// robots broadcast state here (firmware -> coordinator, scheduler, renderer)
pub const ROBOT_UPDATES: &str = "factory/robots";

/// coordinator sends path commands here (coordinator -> firmware)
pub const PATH_COMMANDS: &str = "factory/commands";

/// firmware sends command responses here (firmware -> coordinator)
pub const COMMAND_RESPONSES: &str = "factory/commands/responses";

/// admin control broadcasts (orchestrator or renderer -> all runtime services)
pub const ADMIN_CONTROL: &str = "factory/admin/control";

/// robot lifecycle control (coordinator or renderer -> firmware)
pub const ROBOT_CONTROL: &str = "factory/admin/robots";

/// startup map hash validation handshake (coordinator -> firmware)
pub const MAP_VALIDATION: &str = "factory/admin/map_hash";

// ============ Task/Mission Topics ============

/// task requests from renderer and external producers (renderer/external -> scheduler)
pub const TASK_REQUESTS: &str = "factory/tasks/requests";

/// task assignments (scheduler -> coordinator)
pub const TASK_ASSIGNMENTS: &str = "factory/tasks/assignments";

/// task status updates (coordinator -> scheduler)
pub const TASK_STATUS: &str = "factory/tasks/status";

/// queue state broadcast for monitoring (scheduler -> coordinator, renderer)
pub const QUEUE_STATE: &str = "factory/tasks/queue";

/// full task list snapshot for per-task display (scheduler -> renderer)
pub const TASK_LIST: &str = "factory/tasks/list";

// ============ Telemetry Topics ============

/// path telemetry for visualization (coordinator -> renderer)
pub const TELEMETRY_PATHS: &str = "factory/telemetry/paths";

/// whca runtime metrics telemetry (coordinator -> renderer)
pub const TELEMETRY_WHCA_METRICS: &str = "factory/telemetry/whca_metrics";

// ============ Sender Identifiers ============
// Used in MapValidation.sender to identify the source of broadcasts

/// sender identifier for coordinator layer
pub const SENDER_COORDINATOR: &str = "coordinator";
