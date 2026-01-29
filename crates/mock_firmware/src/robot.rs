//! Simulated robot with physics state

use protocol::{RobotState, RobotUpdate, PathCommand};
use protocol::config::{battery, physics};

/// A simulated robot with physics state
pub struct SimRobot {
    pub id: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub state: RobotState,
    pub battery: f32,
    pub carrying_cargo: Option<u32>,
    pub target: Option<[f32; 3]>,
    pub target_speed: f32, // Speed to use when moving to target
    pub station_position: [f32; 3], // Home charging station
}

impl SimRobot {
    pub fn new(id: u32, station_pos: [f32; 3]) -> Self {
        SimRobot {
            id,
            position: station_pos,
            velocity: [0.0, 0.0, 0.0],
            state: RobotState::Idle,
            battery: 100.0,
            carrying_cargo: None,
            target: None,
            target_speed: physics::ROBOT_SPEED,
            station_position: station_pos,
        }
    }
    
    /// Physics tick: pos += vel * dt
    pub fn update_physics(&mut self, dt: f32, paused: bool) {
        if paused {
            return;
        }
        
        // Update position
        self.position[0] += self.velocity[0] * dt;
        self.position[1] += self.velocity[1] * dt;
        self.position[2] += self.velocity[2] * dt;
        
        // Drain battery while moving
        let speed = (self.velocity[0].powi(2) + self.velocity[2].powi(2)).sqrt();
        if speed > 0.01 {
            self.battery -= battery::DRAIN_RATE * dt;
            self.battery = self.battery.max(0.0);
            
            // Check for low battery
            if self.battery <= battery::LOW_THRESHOLD 
                && self.state != RobotState::LowBattery 
                && self.state != RobotState::Charging 
            {
                self.state = RobotState::LowBattery;
                println!("⚠ Robot {} LOW BATTERY: {:.1}%", self.id, self.battery);
            }
        }
        
        // Simple target-seeking behavior
        if let Some(target) = self.target {
            let dx = target[0] - self.position[0];
            let dz = target[2] - self.position[2];
            let dist = (dx * dx + dz * dz).sqrt();
            
            if dist < physics::ARRIVAL_THRESHOLD {
                // Arrived at target
                self.velocity = [0.0, 0.0, 0.0];
                self.target = None;
                self.on_arrival();
            } else {
                // Move toward target at commanded speed
                self.velocity[0] = (dx / dist) * self.target_speed;
                self.velocity[2] = (dz / dist) * self.target_speed;
            }
        }
        
        // Charge battery when at station
        if self.state == RobotState::Charging {
            self.battery += battery::CHARGE_RATE * dt;
            self.battery = self.battery.min(100.0);
            if self.battery >= 100.0 {
                self.state = RobotState::Idle;
                println!("⚡ Robot {} fully charged", self.id);
            }
        }
    }
    
    /// Called when robot arrives at target
    fn on_arrival(&mut self) {
        match self.state {
            RobotState::MovingToPickup => self.state = RobotState::Picking,
            RobotState::MovingToDrop => self.state = RobotState::Idle,
            RobotState::MovingToStation => self.state = RobotState::Charging,
            _ => {}
        }
    }
    
    /// Convert to wire format
    pub fn to_update(&self) -> RobotUpdate {
        RobotUpdate {
            id: self.id,
            position: self.position,
            velocity: self.velocity,
            state: self.state.clone(),
            battery: self.battery,
            carrying_cargo: self.carrying_cargo,
        }
    }
    
    /// Apply a path command from coordinator
    pub fn apply_command(&mut self, cmd: &PathCommand) {
        match cmd {
            PathCommand::MoveTo { target, speed } => {
                self.target = Some(*target);
                self.target_speed = *speed;
                // Default movement with unknown intent
                if self.carrying_cargo.is_some() {
                    self.state = RobotState::MovingToDrop;
                } else {
                    self.state = RobotState::MovingToPickup;
                }
            }
            PathCommand::MoveToPickup { target, speed } => {
                self.target = Some(*target);
                self.target_speed = *speed;
                self.state = RobotState::MovingToPickup;
            }
            PathCommand::MoveToDropoff { target, speed } => {
                self.target = Some(*target);
                self.target_speed = *speed;
                self.state = RobotState::MovingToDrop;
            }
            PathCommand::Stop => {
                self.velocity = [0.0, 0.0, 0.0];
                self.target = None;
            }
            PathCommand::Pickup { cargo_id } => {
                self.carrying_cargo = Some(*cargo_id);
                self.state = RobotState::MovingToDrop;
            }
            PathCommand::Drop => {
                self.carrying_cargo = None;
                self.state = RobotState::Idle;
            }
            PathCommand::ReturnToCharge => {
                self.target = Some(self.station_position);
                self.target_speed = physics::ROBOT_SPEED; // Default speed for return
                self.state = RobotState::MovingToStation;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_robot() -> SimRobot {
        SimRobot::new(1, [5.0, 0.25, 5.0])
    }

    #[test]
    fn test_new_robot_initial_state() {
        let robot = make_robot();
        assert_eq!(robot.id, 1);
        assert_eq!(robot.position, [5.0, 0.25, 5.0]);
        assert_eq!(robot.velocity, [0.0, 0.0, 0.0]);
        assert_eq!(robot.state, RobotState::Idle);
        assert_eq!(robot.battery, 100.0);
        assert!(robot.carrying_cargo.is_none());
        assert!(robot.target.is_none());
    }

    #[test]
    fn test_physics_update_moves_robot() {
        let mut robot = make_robot();
        robot.target = Some([10.0, 0.25, 5.0]); // 5 units to the right
        robot.target_speed = 2.0;
        robot.state = RobotState::MovingToPickup;
        
        // First tick sets velocity toward target
        robot.update_physics(0.1, false);
        assert!((robot.velocity[0] - 2.0).abs() < 0.01, "Velocity should be set toward target");
        
        // Second tick moves position using velocity
        robot.update_physics(0.5, false);
        assert!(robot.position[0] > 5.0, "Position should have moved right");
    }

    #[test]
    fn test_physics_paused_no_movement() {
        let mut robot = make_robot();
        robot.target = Some([10.0, 0.25, 5.0]);
        robot.target_speed = 2.0;
        robot.state = RobotState::MovingToPickup;
        
        robot.update_physics(1.0, true); // Paused
        
        // Robot should not have moved
        assert_eq!(robot.position, [5.0, 0.25, 5.0]);
    }

    #[test]
    fn test_arrival_changes_state() {
        let mut robot = make_robot();
        robot.target = Some([5.05, 0.25, 5.0]); // Very close target
        robot.target_speed = 2.0;
        robot.state = RobotState::MovingToPickup;
        
        robot.update_physics(0.1, false);
        
        // Should have arrived and transitioned to Picking
        assert!(robot.target.is_none());
        assert_eq!(robot.state, RobotState::Picking);
    }

    #[test]
    fn test_battery_drains_while_moving() {
        let mut robot = make_robot();
        robot.velocity = [2.0, 0.0, 0.0]; // Moving
        
        let initial_battery = robot.battery;
        robot.update_physics(1.0, false);
        
        assert!(robot.battery < initial_battery);
    }

    #[test]
    fn test_battery_charges_at_station() {
        let mut robot = make_robot();
        robot.battery = 50.0;
        robot.state = RobotState::Charging;
        
        robot.update_physics(1.0, false);
        
        assert!(robot.battery > 50.0);
    }

    #[test]
    fn test_apply_move_command_uses_speed() {
        let mut robot = make_robot();
        
        robot.apply_command(&PathCommand::MoveTo { 
            target: [10.0, 0.25, 5.0], 
            speed: 3.5 
        });
        
        assert_eq!(robot.target, Some([10.0, 0.25, 5.0]));
        assert_eq!(robot.target_speed, 3.5);
    }

    #[test]
    fn test_apply_pickup_command() {
        let mut robot = make_robot();
        
        robot.apply_command(&PathCommand::Pickup { cargo_id: 42 });
        
        assert_eq!(robot.carrying_cargo, Some(42));
        assert_eq!(robot.state, RobotState::MovingToDrop);
    }

    #[test]
    fn test_apply_drop_command() {
        let mut robot = make_robot();
        robot.carrying_cargo = Some(42);
        
        robot.apply_command(&PathCommand::Drop);
        
        assert!(robot.carrying_cargo.is_none());
        assert_eq!(robot.state, RobotState::Idle);
    }

    #[test]
    fn test_apply_stop_command() {
        let mut robot = make_robot();
        robot.velocity = [2.0, 0.0, 1.0];
        robot.target = Some([10.0, 0.25, 10.0]);
        
        robot.apply_command(&PathCommand::Stop);
        
        assert_eq!(robot.velocity, [0.0, 0.0, 0.0]);
        assert!(robot.target.is_none());
    }

    #[test]
    fn test_return_to_charge() {
        let mut robot = make_robot();
        robot.position = [10.0, 0.25, 10.0]; // Away from station
        
        robot.apply_command(&PathCommand::ReturnToCharge);
        
        assert_eq!(robot.target, Some([5.0, 0.25, 5.0])); // Station position
        assert_eq!(robot.state, RobotState::MovingToStation);
    }

    #[test]
    fn test_to_update_wire_format() {
        let mut robot = make_robot();
        robot.battery = 75.0;
        robot.carrying_cargo = Some(99);
        robot.state = RobotState::MovingToDrop;
        
        let update = robot.to_update();
        
        assert_eq!(update.id, 1);
        assert_eq!(update.position, [5.0, 0.25, 5.0]);
        assert_eq!(update.battery, 75.0);
        assert_eq!(update.carrying_cargo, Some(99));
        assert_eq!(update.state, RobotState::MovingToDrop);
    }
}
