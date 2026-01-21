//! Closest idle robot allocator

use super::{Allocator, RobotInfo};
use protocol::config::mission_control::MIN_BATTERY_FOR_TASK;
use protocol::{RobotState, Task};
use std::collections::HashMap;

/// Allocator that picks the closest idle robot
pub struct ClosestIdleAllocator {
    /// Minimum battery level required (percentage)
    pub min_battery: f32,
}

impl ClosestIdleAllocator {
    pub fn new() -> Self {
        ClosestIdleAllocator { min_battery: MIN_BATTERY_FOR_TASK }
    }

    fn distance_to_task(robot: &RobotInfo, task: &Task) -> f32 {
        let Some(pickup) = task.pickup_location() else {
            return f32::MAX;
        };

        let dx = (robot.position[0] - pickup.0 as f32).abs();
        let dz = (robot.position[2] - pickup.1 as f32).abs();
        dx + dz
    }
}

impl Default for ClosestIdleAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Allocator for ClosestIdleAllocator {
    fn allocate(&self, task: &Task, robots: &HashMap<u32, RobotInfo>) -> Option<u32> {
        let mut best_robot: Option<u32> = None;
        let mut best_distance = f32::MAX;

        for robot in robots.values() {
            // Skip unavailable robots
            if robot.state != RobotState::Idle { continue; }
            if robot.assigned_task.is_some() { continue; }
            if robot.battery < self.min_battery { continue; }

            let distance = Self::distance_to_task(robot, task);
            if distance < best_distance {
                best_distance = distance;
                best_robot = Some(robot.id);
            }
        }

        if let Some(robot_id) = best_robot {
            println!("→ Allocating Task {} to Robot {} (distance: {:.1})", 
                task.id, robot_id, best_distance);
        }

        best_robot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{Priority, Task, TaskType};

    fn make_robot(id: u32, x: f32, z: f32) -> RobotInfo {
        RobotInfo {
            id,
            position: [x, 0.25, z],
            state: RobotState::Idle,
            battery: 100.0,
            assigned_task: None,
        }
    }

    #[test]
    fn test_closest_robot_selected() {
        let allocator = ClosestIdleAllocator::new();
        let mut robots = HashMap::new();
        robots.insert(1, make_robot(1, 10.0, 10.0));
        robots.insert(2, make_robot(2, 2.0, 2.0));
        robots.insert(3, make_robot(3, 5.0, 5.0));

        let task = Task::new(1, TaskType::PickAndDeliver {
            pickup: (1, 1), dropoff: (5, 5), cargo_id: None,
        }, Priority::Normal);

        assert_eq!(allocator.allocate(&task, &robots), Some(2));
    }

    #[test]
    fn test_busy_robot_skipped() {
        let allocator = ClosestIdleAllocator::new();
        let mut robots = HashMap::new();
        robots.insert(1, RobotInfo {
            id: 1, position: [1.0, 0.25, 1.0],
            state: RobotState::MovingToPickup, battery: 100.0, assigned_task: None,
        });
        robots.insert(2, make_robot(2, 10.0, 10.0));

        let task = Task::new(1, TaskType::PickAndDeliver {
            pickup: (1, 1), dropoff: (5, 5), cargo_id: None,
        }, Priority::Normal);

        assert_eq!(allocator.allocate(&task, &robots), Some(2));
    }

    #[test]
    fn test_low_battery_skipped() {
        let allocator = ClosestIdleAllocator::new();
        let mut robots = HashMap::new();
        robots.insert(1, RobotInfo {
            id: 1, position: [1.0, 0.25, 1.0],
            state: RobotState::Idle, battery: 10.0, assigned_task: None,
        });

        let task = Task::new(1, TaskType::PickAndDeliver {
            pickup: (1, 1), dropoff: (5, 5), cargo_id: None,
        }, Priority::Normal);

        assert_eq!(allocator.allocate(&task, &robots), None);
    }
}
