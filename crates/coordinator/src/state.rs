//! Robot state tracking for the coordinator layer

use std::time::Instant;
use protocol::RobotUpdate;

/// Tracks where the robot is in the task execution lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStage {
    /// No task assigned
    Idle,
    /// Moving to pickup location
    MovingToPickup,
    /// Picking up cargo
    Picking,
    /// Moving to dropoff location
    MovingToDropoff,
    /// Delivering cargo (future: wait for unload confirmation)
    Delivering,
    /// Returning to charging station (future: low battery handling)
    ReturningToStation,
}

/// Reason for returning to station
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnReason {
    /// No pending tasks - can be interrupted by new task assignment
    NoPendingTasks,
    /// Battery is low - critical, should not be interrupted
    LowBattery,
}

/// Robot state tracked by the server
pub struct TrackedRobot {
    pub last_update: RobotUpdate,
    pub current_path: Vec<[f32; 3]>,
    pub path_index: usize,  // Current position in path
    pub current_task: Option<u64>,  // Assigned task ID
    pub task_stage: TaskStage,  // Where in task execution
    pub pickup_location: Option<[f32; 3]>,  // Pickup in world coords
    pub dropoff_location: Option<[f32; 3]>,  // Dropoff in world coords
    pub return_reason: Option<ReturnReason>,  // Why returning to station (if any)
    
    // Timeout tracking
    pub last_progress: Instant,  // Last time we saw progress on current task
    pub task_started: Option<Instant>,  // When current task was assigned

    // Fault handling
    pub blocked_since: Option<Instant>,  // When robot entered Blocked state
    pub faulted_since: Option<Instant>,  // When robot entered Faulted state
    pub replan_attempts: u32,  // Consecutive replans due to deviations/collisions
}

impl TrackedRobot {
    pub fn new(update: RobotUpdate) -> Self {
        TrackedRobot {
            last_update: update,
            current_path: Vec::new(),
            path_index: 0,
            current_task: None,
            task_stage: TaskStage::Idle,
            pickup_location: None,
            dropoff_location: None,
            return_reason: None,
            last_progress: Instant::now(),
            task_started: None,
            blocked_since: None,
            faulted_since: None,
            replan_attempts: 0,
        }
    }
    
    /// Check if robot has reached end of assigned path
    pub fn path_complete(&self) -> bool {
        self.current_path.is_empty() || self.path_index >= self.current_path.len()
    }
    
    /// Get next waypoint in path
    pub fn next_waypoint(&self) -> Option<[f32; 3]> {
        self.current_path.get(self.path_index).copied()
    }
    
    /// Advance to next waypoint
    pub fn advance_path(&mut self) {
        if self.path_index < self.current_path.len() {
            self.path_index += 1;
        }
    }
    
    /// Assign a new path
    pub fn set_path(&mut self, path: Vec<[f32; 3]>) {
        self.current_path = path;
        self.path_index = 0;
    }
    
    /// Mark progress on current task (resets timeout)
    pub fn mark_progress(&mut self) {
        self.last_progress = Instant::now();
    }
    
    /// Check if task has timed out (no progress for too long)
    pub fn is_task_timed_out(&self, timeout_secs: u64) -> bool {
        self.current_task.is_some() && 
        self.last_progress.elapsed().as_secs() >= timeout_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{RobotState, RobotUpdate};
    use std::time::Duration;

    fn make_update(id: u32) -> RobotUpdate {
        RobotUpdate {
            id,
            position: [0.0, 0.25, 0.0],
            velocity: [0.0, 0.0, 0.0],
            state: RobotState::Idle,
            battery: 100.0,
            carrying_cargo: None,
            station_position: [15.0, 0.25, 1.0],
        }
    }

    #[test]
    fn test_timeout_not_triggered_without_task() {
        let robot = TrackedRobot::new(make_update(1));
        // No task assigned, should not timeout
        assert!(!robot.is_task_timed_out(1));
    }

    #[test]
    fn test_timeout_not_triggered_immediately() {
        let mut robot = TrackedRobot::new(make_update(1));
        robot.current_task = Some(1);
        robot.mark_progress();
        // Just assigned, should not timeout
        assert!(!robot.is_task_timed_out(30));
    }

    #[test]
    fn test_timeout_triggered_after_delay() {
        let mut robot = TrackedRobot::new(make_update(1));
        robot.current_task = Some(1);
        // Manually set last_progress to 2 seconds ago
        robot.last_progress = Instant::now() - Duration::from_secs(2);
        // Should timeout after 1 second
        assert!(robot.is_task_timed_out(1));
    }

    #[test]
    fn test_mark_progress_resets_timeout() {
        let mut robot = TrackedRobot::new(make_update(1));
        robot.current_task = Some(1);
        // Set progress to old time
        robot.last_progress = Instant::now() - Duration::from_secs(100);
        assert!(robot.is_task_timed_out(1));
        
        // Mark progress resets timeout
        robot.mark_progress();
        assert!(!robot.is_task_timed_out(1));
    }
}
