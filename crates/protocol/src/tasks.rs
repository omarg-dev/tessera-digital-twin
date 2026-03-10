//! Task and order types for inter-layer communication
//!
//! These types define the business-level entities for task scheduling and execution.
//! They are intentionally decoupled from robot-level operations (firmware layer)
//! and path planning details (coordinator layer).

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
    /// Timestamp when task was completed/failed/cancelled (Unix millis)
    #[serde(default)]
    pub completed_at: Option<u64>,
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
            completed_at: None,
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

/// Assignment message: scheduler → coordinator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    /// The task being assigned
    pub task: Task,
    /// Robot assigned to execute this task
    pub robot_id: u32,
}

/// Status update: coordinator → scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusUpdate {
    /// Task ID being updated
    pub task_id: TaskId,
    /// New status
    pub status: TaskStatus,
    /// Robot that reported this update (if any)
    pub robot_id: Option<u32>,
}

/// New task request: external system → scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// What type of work
    pub task_type: TaskType,
    /// Priority level
    pub priority: Priority,
}

/// Command sent to the scheduler via TASK_REQUESTS topic.
///
/// Extends beyond simple new-task requests to support full task management
/// (creation, cancellation, and priority mutation) from external systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskCommand {
    /// Create a new task
    New { task_type: TaskType, priority: Priority },
    /// Cancel an existing pending task
    Cancel(TaskId),
    /// Change the priority of an existing pending task
    SetPriority(TaskId, Priority),
}

/// Full task list snapshot: scheduler → renderer, broadcast periodically.
///
/// Allows the renderer to display individual task details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListSnapshot {
    /// all tasks currently tracked by the scheduler
    pub tasks: Vec<Task>,
    /// Unix milliseconds when this snapshot was taken
    pub timestamp_ms: u64,
}

/// Queue status snapshot: scheduler broadcasts this periodically
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueState {
    /// Number of pending (unassigned) tasks
    pub pending: usize,
    /// Total tasks in queue
    pub total: usize,
    /// Number of online robots
    pub robots_online: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new(
            1,
            TaskType::PickAndDeliver { pickup: (5, 5), dropoff: (10, 10), cargo_id: None },
            Priority::Normal,
        );
        
        assert_eq!(task.id, 1);
        assert_eq!(task.priority, Priority::Normal);
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.created_at > 0);
    }

    #[test]
    fn test_task_pickup_location() {
        let pick_deliver = Task::new(
            1,
            TaskType::PickAndDeliver { pickup: (5, 5), dropoff: (10, 10), cargo_id: None },
            Priority::Normal,
        );
        assert_eq!(pick_deliver.pickup_location(), Some((5, 5)));

        let relocate = Task::new(
            2,
            TaskType::Relocate { from: (3, 3), to: (7, 7) },
            Priority::High,
        );
        assert_eq!(relocate.pickup_location(), Some((3, 3)));

        let return_station = Task::new(
            3,
            TaskType::ReturnToStation { robot_id: 1 },
            Priority::Low,
        );
        assert_eq!(return_station.pickup_location(), None);
    }

    #[test]
    fn test_task_target_location() {
        let pick_deliver = Task::new(
            1,
            TaskType::PickAndDeliver { pickup: (5, 5), dropoff: (10, 10), cargo_id: None },
            Priority::Normal,
        );
        assert_eq!(pick_deliver.target_location(), Some((10, 10)));

        let relocate = Task::new(
            2,
            TaskType::Relocate { from: (3, 3), to: (7, 7) },
            Priority::High,
        );
        assert_eq!(relocate.target_location(), Some((7, 7)));
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_task_status_serialization() {
        let statuses = vec![
            TaskStatus::Pending,
            TaskStatus::Assigned { robot_id: 5 },
            TaskStatus::InProgress { robot_id: 5 },
            TaskStatus::Completed,
            TaskStatus::Failed { reason: "test error".to_string() },
            TaskStatus::Cancelled,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }
}
