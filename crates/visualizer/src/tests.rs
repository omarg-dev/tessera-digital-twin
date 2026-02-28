//! Visualizer Tests
//!
//! Tests for components, resources, and system logic.
//! Note: Full Bevy system tests require a headless app context.

use super::components::*;
use super::resources::*;
use bevy::prelude::*;
use protocol::RobotState;

#[cfg(test)]
mod component_tests {
    use super::*;

    #[test]
    fn test_robot_component_creation() {
        let robot = Robot {
            id: 42,
            state: RobotState::Idle,
            position: Vec3::new(5.0, 0.25, 3.0),
            battery: 85.0,
            current_task: Some(100),
            carrying_cargo: None,
        };

        assert_eq!(robot.id, 42);
        assert_eq!(robot.state, RobotState::Idle);
        assert_eq!(robot.battery, 85.0);
        assert_eq!(robot.current_task, Some(100));
        assert!(robot.carrying_cargo.is_none());
    }

    #[test]
    fn test_robot_with_cargo() {
        let robot = Robot {
            id: 1,
            state: RobotState::MovingToDrop,
            position: Vec3::ZERO,
            battery: 50.0,
            current_task: Some(5),
            carrying_cargo: Some(999),
        };

        assert_eq!(robot.carrying_cargo, Some(999));
        assert_eq!(robot.state, RobotState::MovingToDrop);
    }

    #[test]
    fn test_shelf_with_capacity() {
        let shelf = Shelf { cargo: 5 };
        assert_eq!(shelf.cargo, 5);
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use protocol::RobotUpdate;

    #[test]
    fn test_robot_index_operations() {
        let index = RobotIndex::default();
        
        // Verify default is empty
        assert!(index.by_id.is_empty());
        
        // Note: We can't create Entity without a World in Bevy 0.17
        // This test verifies the HashMap operations work correctly
        assert_eq!(index.by_id.get(&999), None);
    }

    #[test]
    fn test_robot_last_positions() {
        let mut positions = RobotLastPositions::default();

        positions.by_id.insert(1, [1.0, 0.25, 2.0]);
        positions.by_id.insert(2, [5.0, 0.25, 5.0]);

        assert_eq!(positions.by_id.get(&1), Some(&[1.0, 0.25, 2.0]));
        assert_eq!(positions.by_id.get(&2), Some(&[5.0, 0.25, 5.0]));
    }

    #[test]
    fn test_robot_updates_default() {
        let updates = RobotUpdates::default();
        assert!(updates.updates.is_empty());
    }

    #[test]
    fn test_robot_updates_collect() {
        let mut updates = RobotUpdates::default();
        
        updates.updates.push(RobotUpdate {
            id: 1,
            position: [1.0, 0.25, 1.0],
            velocity: [0.0, 0.0, 0.0],
            state: RobotState::Idle,
            battery: 100.0,
            carrying_cargo: None,
            station_position: [0.0, 0.25, 0.0],
        });

        assert_eq!(updates.updates.len(), 1);
        assert_eq!(updates.updates[0].id, 1);
    }

    #[test]
    fn test_movement_detection_logic() {
        // Test the logic used for movement detection
        let old_pos: [f32; 3] = [1.0, 0.25, 2.0];
        let new_pos: [f32; 3] = [1.001, 0.25, 2.001]; // Very small movement
        let threshold = 0.01;

        let dx = (new_pos[0] - old_pos[0]).abs();
        let dz = (new_pos[2] - old_pos[2]).abs();
        let moved = dx > threshold || dz > threshold;

        assert!(!moved, "Small movement should not trigger update");

        let big_move: [f32; 3] = [2.0, 0.25, 2.0];
        let dx2 = (big_move[0] - old_pos[0]).abs();
        let moved2 = dx2 > threshold;

        assert!(moved2, "Large movement should trigger update");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use protocol::RobotState;

    /// Test that robot state transitions are properly represented
    #[test]
    fn test_robot_state_lifecycle() {
        let states = [
            RobotState::Idle,
            RobotState::MovingToPickup,
            RobotState::Picking,
            RobotState::MovingToDrop,
            RobotState::MovingToStation,
            RobotState::Charging,
        ];

        for state in states {
            let robot = Robot {
                id: 1,
                state: state.clone(),
                position: Vec3::ZERO,
                battery: 50.0,
                current_task: None,
                carrying_cargo: None,
            };
            
            assert_eq!(robot.state, state);
        }
    }

    /// Test robot position tracking across updates
    #[test]
    fn test_position_tracking_update() {
        let mut positions = RobotLastPositions::default();
        
        // Initial position
        positions.by_id.insert(1, [0.0, 0.25, 0.0]);
        
        // Robot moves
        let new_pos = [5.0, 0.25, 5.0];
        positions.by_id.insert(1, new_pos);
        
        assert_eq!(positions.by_id.get(&1), Some(&new_pos));
    }

    /// Test multi-robot position tracking
    #[test]
    fn test_multi_robot_position_tracking() {
        let mut positions = RobotLastPositions::default();
        
        // Track 10 robots
        for i in 0..10u32 {
            positions.by_id.insert(i, [i as f32, 0.25, i as f32]);
        }

        assert_eq!(positions.by_id.len(), 10);
        
        // Verify all are retrievable
        for i in 0..10u32 {
            assert!(positions.by_id.contains_key(&i));
            assert_eq!(positions.by_id.get(&i), Some(&[i as f32, 0.25, i as f32]));
        }
    }
}
