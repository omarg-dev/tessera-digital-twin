//! Zenoh topic constants - Single source of truth for all topic strings

/// Robots broadcast their state here (swarm_driver → fleet_server, visualizer)
pub const ROBOT_UPDATES: &str = "factory/robots";

/// Fleet server sends path commands here (fleet_server → swarm_driver)
pub const PATH_COMMANDS: &str = "factory/commands";

/// Control plane for pause/resume/reset (fleet_server → all)
pub const ADMIN_CONTROL: &str = "factory/admin/control";

/// Map hash validation on startup (fleet_server → all)
pub const MAP_VALIDATION: &str = "factory/admin/map_hash";

// ============ Task/Mission Topics ============

/// New task requests (external → mission_control)
pub const TASK_REQUESTS: &str = "factory/tasks/requests";

/// Task assignments (mission_control → fleet_server)
pub const TASK_ASSIGNMENTS: &str = "factory/tasks/assignments";

/// Task status updates (fleet_server → mission_control)
pub const TASK_STATUS: &str = "factory/tasks/status";

/// Queue state broadcast for monitoring (mission_control → visualizer)
pub const QUEUE_STATE: &str = "factory/tasks/queue";
