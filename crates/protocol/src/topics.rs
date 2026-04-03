//! Zenoh topic constants - Single source of truth for all topic strings
//!
//! Topics are organized by data flow direction. Abstraction layers (top to bottom):
//! - **orchestrator** - Process management and global commands (orchestrator crate)
//! - **renderer** - Visualization layer (visualizer crate)
//! - **scheduler** - Task queue and robot allocation (scheduler crate)
//! - **coordinator** - Path planning and task execution (coordinator crate)
//! - **firmware** - Robot physics simulation (mock_firmware crate)

/// robots broadcast state here (firmware -> coordinator, scheduler, renderer)
pub const ROBOT_UPDATES: &str = "warehouse/robots";

/// coordinator sends path commands here (coordinator -> firmware)
pub const PATH_COMMANDS: &str = "warehouse/commands";

/// firmware sends command responses here (firmware -> coordinator)
pub const COMMAND_RESPONSES: &str = "warehouse/commands/responses";

/// admin control broadcasts (orchestrator or renderer -> all runtime services)
pub const ADMIN_CONTROL: &str = "warehouse/admin/control";

/// robot lifecycle control (coordinator or renderer -> firmware)
pub const ROBOT_CONTROL: &str = "warehouse/admin/robots";

/// startup map hash validation handshake (coordinator -> firmware)
pub const MAP_VALIDATION: &str = "warehouse/admin/map_hash";

// ============ Task/Mission Topics ============

/// task requests from renderer and external producers (renderer/external -> scheduler)
pub const TASK_REQUESTS: &str = "warehouse/tasks/requests";

/// task assignments (scheduler -> coordinator)
pub const TASK_ASSIGNMENTS: &str = "warehouse/tasks/assignments";

/// task status updates (coordinator -> scheduler)
pub const TASK_STATUS: &str = "warehouse/tasks/status";

/// queue state broadcast for monitoring (scheduler -> coordinator, renderer)
pub const QUEUE_STATE: &str = "warehouse/tasks/queue";

/// full task list snapshot for per-task display (scheduler -> renderer)
pub const TASK_LIST: &str = "warehouse/tasks/list";

// ============ Telemetry Topics ============

/// path telemetry for visualization (coordinator -> renderer)
pub const TELEMETRY_PATHS: &str = "warehouse/telemetry/paths";

/// whca runtime metrics telemetry (coordinator -> renderer)
pub const TELEMETRY_WHCA_METRICS: &str = "warehouse/telemetry/whca_metrics";

// ============ Sender Identifiers ============
// Used in MapValidation.sender to identify the source of broadcasts

/// sender identifier for coordinator layer
pub const SENDER_COORDINATOR: &str = "coordinator";
