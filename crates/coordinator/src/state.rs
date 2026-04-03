//! Robot state tracking for the coordinator layer

use std::collections::VecDeque;
use std::time::Instant;
use protocol::{RobotState, RobotUpdate};

use crate::pathfinding::GridPos;

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
    /// Post-delivery repositioning - can be interrupted by new task assignment
    PostDelivery,
    /// Battery is low - critical, should not be interrupted
    LowBattery,
}

/// Robot state tracked by the server
pub struct TrackedRobot {
    pub last_update: RobotUpdate,
    pub last_tick: Option<u64>,
    pub recent_positions: VecDeque<GridPos>,
    pub current_path: Vec<[f32; 3]>,
    pub path_index: usize,  // Current position in path
    pub current_task: Option<u64>,  // Assigned task ID
    pub task_stage: TaskStage,  // Where in task execution
    pub pickup_location: Option<[f32; 3]>,  // Pickup in world coords
    pub dropoff_location: Option<[f32; 3]>,  // Dropoff in world coords
    pub pickup_grid: Option<(usize, usize)>,  // Pickup shelf grid position (for inventory)
    pub dropoff_grid: Option<(usize, usize)>,  // Dropoff shelf grid position (for inventory)
    pub return_reason: Option<ReturnReason>,  // Why returning to station (if any)
    pub skip_next_validation: bool,
    
    // Timeout tracking
    pub last_progress: Instant,  // Last time we saw progress on current task
    pub task_started: Option<Instant>,  // When current task was assigned

    // Delivery retry tracking
    pub delivery_retry_attempts: u32,
    pub last_drop_command_sent_at: Option<Instant>,

    // Fault handling
    pub blocked_since: Option<Instant>,  // When robot entered Blocked state
    pub faulted_since: Option<Instant>,  // When robot entered Faulted state
    pub replan_attempts: u32,  // Consecutive replans due to deviations/collisions

    // Reservation wait tracking (deadlock prevention)
    pub waiting_since: Option<Instant>,  // When robot started waiting on reservation
    pub waiting_for: Option<GridPos>,  // Grid cell we are waiting to enter

    /// true after FollowPath has been dispatched for the current path segment;
    /// cleared by set_path() so any new or replanned path triggers a fresh dispatch.
    pub path_sent: bool,

    /// last time stationary reservations were refreshed for this robot.
    pub last_stationary_reservation_refresh: Option<Instant>,
    /// last grid position used for stationary reservation refresh.
    pub last_stationary_reservation_pos: Option<GridPos>,
    /// last task stage observed when refreshing stationary reservations.
    pub last_stationary_reservation_stage: Option<TaskStage>,
    /// whether robot was faulted/blocked at last stationary reservation refresh.
    pub last_stationary_reservation_faulted_or_blocked: bool,
    /// signature of recent stationary reservation history at last refresh.
    pub last_stationary_reservation_history_sig: u64,
}

impl TrackedRobot {
    pub fn new(update: RobotUpdate) -> Self {
        TrackedRobot {
            last_update: update,
            last_tick: None,
            recent_positions: VecDeque::new(),
            current_path: Vec::new(),
            path_index: 0,
            current_task: None,
            task_stage: TaskStage::Idle,
            pickup_location: None,
            dropoff_location: None,
            pickup_grid: None,
            dropoff_grid: None,
            return_reason: None,
            skip_next_validation: false,
            last_progress: Instant::now(),
            task_started: None,
            delivery_retry_attempts: 0,
            last_drop_command_sent_at: None,
            blocked_since: None,
            faulted_since: None,
            replan_attempts: 0,
            waiting_since: None,
            waiting_for: None,
            path_sent: false,
            last_stationary_reservation_refresh: None,
            last_stationary_reservation_pos: None,
            last_stationary_reservation_stage: None,
            last_stationary_reservation_faulted_or_blocked: false,
            last_stationary_reservation_history_sig: 0,
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
        self.clear_wait();
    }
    
    /// Assign a new path and mark it as unsent so coordinator dispatches FollowPath
    pub fn set_path(&mut self, path: Vec<[f32; 3]>) {
        self.current_path = path;
        self.path_index = 0;
        self.path_sent = false;
        self.clear_wait();
    }
    
    /// Mark progress on current task (resets timeout)
    pub fn mark_progress(&mut self) {
        self.last_progress = Instant::now();
    }

    /// Mark that a drop command was sent to firmware.
    pub fn mark_drop_command_sent(&mut self) {
        self.last_drop_command_sent_at = Some(Instant::now());
    }

    /// Clear delivery retry bookkeeping.
    pub fn reset_delivery_tracking(&mut self) {
        self.delivery_retry_attempts = 0;
        self.last_drop_command_sent_at = None;
    }

    /// Mark robot as waiting on a reserved cell
    pub fn set_wait(&mut self, target: GridPos) {
        if self.waiting_for != Some(target) {
            self.waiting_for = Some(target);
            self.waiting_since = Some(Instant::now());
        } else if self.waiting_since.is_none() {
            self.waiting_since = Some(Instant::now());
        }
    }

    /// Clear reservation wait state
    pub fn clear_wait(&mut self) {
        self.waiting_since = None;
        self.waiting_for = None;
    }

    /// How long have we been waiting on a reservation (seconds)
    pub fn wait_elapsed_secs(&self) -> Option<u64> {
        self.waiting_since.map(|since| since.elapsed().as_secs())
    }
    
    /// Check if task has timed out (no progress for too long)
    pub fn is_task_timed_out(&self, timeout_secs: u64) -> bool {
        self.current_task.is_some() && 
        self.last_progress.elapsed().as_secs() >= timeout_secs
    }

    fn is_faulted_or_blocked(&self) -> bool {
        matches!(self.last_update.state, RobotState::Faulted | RobotState::Blocked)
    }

    /// decide if stationary reservations should be refreshed now.
    pub fn stationary_refresh_due(
        &self,
        now: Instant,
        interval: std::time::Duration,
        current_pos: GridPos,
        history_sig: u64,
    ) -> bool {
        let Some(last_refresh) = self.last_stationary_reservation_refresh else {
            return true;
        };

        if self.last_stationary_reservation_pos != Some(current_pos) {
            return true;
        }

        if self.last_stationary_reservation_stage != Some(self.task_stage) {
            return true;
        }

        if self.last_stationary_reservation_faulted_or_blocked != self.is_faulted_or_blocked() {
            return true;
        }

        if self.last_stationary_reservation_history_sig != history_sig {
            return true;
        }

        now.duration_since(last_refresh) >= interval
    }

    /// record metadata after refreshing stationary reservations.
    pub fn record_stationary_reservation_refresh(
        &mut self,
        now: Instant,
        current_pos: GridPos,
        history_sig: u64,
    ) {
        self.last_stationary_reservation_refresh = Some(now);
        self.last_stationary_reservation_pos = Some(current_pos);
        self.last_stationary_reservation_stage = Some(self.task_stage);
        self.last_stationary_reservation_faulted_or_blocked = self.is_faulted_or_blocked();
        self.last_stationary_reservation_history_sig = history_sig;
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
            enabled: true,
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

    #[test]
    fn test_stationary_refresh_due_initially() {
        let robot = TrackedRobot::new(make_update(1));
        let now = Instant::now();

        assert!(robot.stationary_refresh_due(
            now,
            Duration::from_millis(500),
            (1, 1),
            123,
        ));
    }

    #[test]
    fn test_stationary_refresh_not_due_without_changes() {
        let mut robot = TrackedRobot::new(make_update(1));
        let now = Instant::now();
        robot.record_stationary_reservation_refresh(now, (2, 2), 555);

        assert!(!robot.stationary_refresh_due(
            now + Duration::from_millis(250),
            Duration::from_millis(500),
            (2, 2),
            555,
        ));
    }

    #[test]
    fn test_stationary_refresh_due_on_position_or_stage_change() {
        let mut robot = TrackedRobot::new(make_update(1));
        let now = Instant::now();
        robot.record_stationary_reservation_refresh(now, (2, 2), 555);

        assert!(robot.stationary_refresh_due(
            now + Duration::from_millis(100),
            Duration::from_millis(500),
            (3, 2),
            555,
        ));

        robot.record_stationary_reservation_refresh(now, (2, 2), 555);
        robot.task_stage = TaskStage::Picking;
        assert!(robot.stationary_refresh_due(
            now + Duration::from_millis(100),
            Duration::from_millis(500),
            (2, 2),
            555,
        ));
    }

    #[test]
    fn test_mark_drop_command_sent_sets_timestamp() {
        let mut robot = TrackedRobot::new(make_update(1));
        assert!(robot.last_drop_command_sent_at.is_none());

        robot.mark_drop_command_sent();

        assert!(robot.last_drop_command_sent_at.is_some());
    }

    #[test]
    fn test_reset_delivery_tracking_clears_retry_state() {
        let mut robot = TrackedRobot::new(make_update(1));
        robot.delivery_retry_attempts = 2;
        robot.mark_drop_command_sent();

        robot.reset_delivery_tracking();

        assert_eq!(robot.delivery_retry_attempts, 0);
        assert!(robot.last_drop_command_sent_at.is_none());
    }
}
