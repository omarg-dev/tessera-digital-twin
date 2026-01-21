//! Task and order types for mission control
//!
//! These types define the business-level entities that mission_control manages.
//! They are intentionally decoupled from robot-level operations.

use serde::{Deserialize, Serialize};

/// Unique identifier for a task
pub type TaskId = u64;

/// Priority level for tasks (higher = more urgent)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

/// Task status lifecycle
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is queued, waiting for assignment
    Pending,
    /// Task is assigned to a robot, waiting for execution
    Assigned { robot_id: u32 },
    /// Robot is executing this task
    InProgress { robot_id: u32 },
    /// Task completed successfully
    Completed,
    /// Task failed (robot error, obstacle, etc.)
    Failed { reason: String },
    /// Task was cancelled
    Cancelled,
}

/// The type of work to perform
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskType {
    /// Pick up cargo from a shelf and deliver to dropoff
    PickAndDeliver {
        /// Shelf grid position to pick from
        pickup: (usize, usize),
        /// Dropoff grid position
        dropoff: (usize, usize),
        /// Optional cargo ID (if known)
        cargo_id: Option<u32>,
    },
    /// Move cargo from one shelf to another (inventory reorg)
    Relocate {
        from: (usize, usize),
        to: (usize, usize),
    },
    /// Return robot to charging station
    ReturnToStation {
        robot_id: u32,
    },
}

/// A task represents a unit of work to be executed by a robot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: TaskId,
    /// What type of work this task represents
    pub task_type: TaskType,
    /// Priority level
    pub priority: Priority,
    /// Current status
    pub status: TaskStatus,
    /// Timestamp when task was created (Unix millis)
    pub created_at: u64,
}

impl Task {
    /// Create a new pending task
    pub fn new(id: TaskId, task_type: TaskType, priority: Priority) -> Self {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Task {
            id,
            task_type,
            priority,
            status: TaskStatus::Pending,
            created_at,
        }
    }

    /// Get the pickup location for this task (if applicable)
    pub fn pickup_location(&self) -> Option<(usize, usize)> {
        match &self.task_type {
            TaskType::PickAndDeliver { pickup, .. } => Some(*pickup),
            TaskType::Relocate { from, .. } => Some(*from),
            TaskType::ReturnToStation { .. } => None,
        }
    }

    /// Get the delivery/target location for this task
    pub fn target_location(&self) -> Option<(usize, usize)> {
        match &self.task_type {
            TaskType::PickAndDeliver { dropoff, .. } => Some(*dropoff),
            TaskType::Relocate { to, .. } => Some(*to),
            TaskType::ReturnToStation { .. } => None,
        }
    }
}

/// Assignment message: mission_control → fleet_server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    /// The task being assigned
    pub task: Task,
    /// Robot assigned to execute this task
    pub robot_id: u32,
}

/// Status update: fleet_server → mission_control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusUpdate {
    /// Task ID being updated
    pub task_id: TaskId,
    /// New status
    pub status: TaskStatus,
    /// Robot that reported this update (if any)
    pub robot_id: Option<u32>,
}

/// New task request: external system → mission_control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// What type of work
    pub task_type: TaskType,
    /// Priority level
    pub priority: Priority,
}
