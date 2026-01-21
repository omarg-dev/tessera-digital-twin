//! Robot state tracking for the fleet server

use protocol::RobotUpdate;

/// Robot state tracked by the server
pub struct TrackedRobot {
    pub last_update: RobotUpdate,
    pub current_path: Vec<[f32; 3]>,
    pub path_index: usize,  // Current position in path
    pub current_task: Option<u64>,  // Assigned task ID
}

impl TrackedRobot {
    pub fn new(update: RobotUpdate) -> Self {
        TrackedRobot {
            last_update: update,
            current_path: Vec::new(),
            path_index: 0,
            current_task: None,
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
}
