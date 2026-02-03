//! Simulated robot with physics state

use protocol::{RobotState, RobotUpdate, PathCommand, CommandStatus};
use protocol::config::{battery, physics};
use rand::Rng;


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
    pub pickup_timer: f32, // Time remaining for pickup operation
    pub drop_timer: f32, // Time remaining for dropoff operation
    pub enabled: bool, // Whether robot is active (can be disabled by orchestrator)
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
            pickup_timer: 0.0,
            drop_timer: 0.0,
            enabled: true,
        }
    }
    
    /// Reset robot to initial state at its home station
    pub fn restart(&mut self) {
        self.position = self.station_position;
        self.velocity = [0.0, 0.0, 0.0];
        self.state = RobotState::Idle;
        self.battery = 100.0;
        self.carrying_cargo = None;
        self.target = None;
        self.target_speed = physics::ROBOT_SPEED;
        self.pickup_timer = 0.0;
        self.drop_timer = 0.0;
        self.enabled = true;
    }
    
    /// Physics tick: pos += vel * dt
    pub fn update_physics(&mut self, dt: f32, paused: bool, chaos: bool) {
        if paused || !self.enabled {
            return;
        }
        
        // Update position
        self.position[0] += self.velocity[0] * dt;
        self.position[1] += self.velocity[1] * dt;
        self.position[2] += self.velocity[2] * dt;
        
        // Chaos: add position drift (simulates odometry errors)
        let (dx, dz) = protocol::chaos::get_position_drift(chaos);
        if dx != 0.0 || dz != 0.0 {
            self.position[0] += dx;
            self.position[2] += dz;
            protocol::chaos::log_chaos_event(
                &format!("Robot {} position drift: ({:.3}, {:.3})", self.id, dx, dz),
                "Firmware",
            );
        }
        
        // Drain battery while moving
        let speed = (self.velocity[0].powi(2) + self.velocity[2].powi(2)).sqrt();
        if speed > 0.01 {
            // Add random variation to drain rate (±40% variation)
            let mut rng = rand::thread_rng();
            let drain_rate = rng.gen_range(battery::DRAIN_RATE_RANGE.0..=battery::DRAIN_RATE_RANGE.1);
            self.battery -= drain_rate * dt;
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
        
        // Handle pickup delay (simulate cargo loading time)
        if self.state == RobotState::Picking && self.pickup_timer > 0.0 {
            self.pickup_timer -= dt;
            if self.pickup_timer <= 0.0 {
                self.pickup_timer = 0.0;
                self.state = RobotState::MovingToDrop;
            }
        }

        // Handle dropoff delay (simulate cargo unload time)
        if self.drop_timer > 0.0 {
            self.drop_timer -= dt;
            if self.drop_timer <= 0.0 {
                self.drop_timer = 0.0;
                self.state = RobotState::Idle;
            }
        }
    }
    
    /// Called when robot arrives at target
    /// 
    /// For MovingToPickup/MovingToDrop: robot stays in same state, waiting for
    /// next waypoint or explicit Pickup/Drop command from coordinator.
    /// The coordinator is responsible for detecting arrival at final destination
    /// and sending the appropriate command.
    fn on_arrival(&mut self) {
        match self.state {
            // Don't auto-transition - wait for explicit Pickup command
            RobotState::MovingToPickup => {
                // Stay in MovingToPickup, coordinator will send next waypoint or Pickup
            }
            // Don't auto-transition - wait for explicit Drop command  
            RobotState::MovingToDrop => {
                // Stay in MovingToDrop, coordinator will send next waypoint or Drop
            }
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
            station_position: self.station_position,
        }
    }
    
    /// Apply a path command from coordinator
    /// Returns CommandStatus indicating acceptance or rejection with reason
    pub fn apply_command(&mut self, command: &PathCommand) -> CommandStatus {
        use protocol::CommandStatus;
        
        match command {
            PathCommand::MoveTo { target, speed } => {
                if target[0].is_nan() || target[2].is_nan() || *speed <= 0.0 {
                    return CommandStatus::Rejected { 
                        reason: "Invalid target".to_string() 
                    };
                }
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
                if target[0].is_nan() || target[2].is_nan() || *speed <= 0.0 {
                    return CommandStatus::Rejected { 
                        reason: "Invalid pickup target".to_string() 
                    };
                }
                self.target = Some(*target);
                self.target_speed = *speed;
                self.state = RobotState::MovingToPickup;
            }
            PathCommand::MoveToDropoff { target, speed } => {
                if target[0].is_nan() || target[2].is_nan() || *speed <= 0.0 {
                    return CommandStatus::Rejected { 
                        reason: "Invalid dropoff target".to_string() 
                    };
                }
                self.target = Some(*target);
                self.target_speed = *speed;
                self.state = RobotState::MovingToDrop;
            }
            PathCommand::Stop => {
                self.velocity = [0.0, 0.0, 0.0];
                self.target = None;
            }
            PathCommand::Pickup { cargo_id } => {
                if self.carrying_cargo.is_some() {
                    return CommandStatus::Rejected { 
                        reason: "Robot already carrying cargo".to_string() 
                    };
                }
                self.carrying_cargo = Some(*cargo_id);
                self.state = RobotState::Picking;
                // Start pickup timer (will transition to MovingToDrop when complete)
                self.pickup_timer = protocol::config::coordinator::PICKUP_DELAY_SECS;
            }
            PathCommand::Drop => {
                if self.carrying_cargo.is_none() {
                    return CommandStatus::Rejected { 
                        reason: "Robot not carrying cargo".to_string() 
                    };
                }
                self.carrying_cargo = None;
                // Start dropoff timer (will transition to Idle when complete)
                self.drop_timer = protocol::config::coordinator::DROPOFF_DELAY_SECS;
                // Keep state non-idle while unloading
                if self.state == RobotState::MovingToDrop {
                    // no-op, already in the expected state
                } else {
                    self.state = RobotState::MovingToDrop;
                }
            }
            PathCommand::ReturnToCharge => {
                // Coordinator pathfinds back to station and sends MoveTo commands.
                // This command just signals "at station" → start charging.
                if self.is_at_station() {
                    self.state = RobotState::Charging;
                } else {
                    // Robot is away from station - stay idle until coordinator routes us
                    self.state = RobotState::Idle;
                }
                self.target = None;
                self.velocity = [0.0, 0.0, 0.0];
            }
        }
        
        CommandStatus::Accepted
    }
    
    /// Check if robot is near its home station
    fn is_at_station(&self) -> bool {
        let dx = self.position[0] - self.station_position[0];
        let dz = self.position[2] - self.station_position[2];
        (dx * dx + dz * dz).sqrt() < 0.5
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
        robot.update_physics(0.1, false, false);
        assert!((robot.velocity[0] - 2.0).abs() < 0.01, "Velocity should be set toward target");
        
        // Second tick moves position using velocity
        robot.update_physics(0.5, false, false);
        assert!(robot.position[0] > 5.0, "Position should have moved right");
    }

    #[test]
    fn test_physics_paused_no_movement() {
        let mut robot = make_robot();
        robot.target = Some([10.0, 0.25, 5.0]);
        robot.target_speed = 2.0;
        robot.state = RobotState::MovingToPickup;
        
        robot.update_physics(1.0, true, false); // Paused
        
        // Robot should not have moved
        assert_eq!(robot.position, [5.0, 0.25, 5.0]);
    }

    #[test]
    fn test_arrival_clears_target_but_keeps_state() {
        // After fixing the waypoint bug: arrival at waypoint should NOT
        // auto-transition to Picking. The coordinator sends explicit Pickup command.
        let mut robot = make_robot();
        robot.target = Some([5.05, 0.25, 5.0]); // Very close target
        robot.target_speed = 2.0;
        robot.state = RobotState::MovingToPickup;
        
        robot.update_physics(0.1, false, false);
        
        // Should have arrived (target cleared) but stay in MovingToPickup
        // until coordinator sends Pickup command
        assert!(robot.target.is_none());
        assert_eq!(robot.state, RobotState::MovingToPickup);
    }
    
    #[test]
    fn test_station_arrival_transitions_to_charging() {
        // MovingToStation should still auto-transition to Charging
        let mut robot = make_robot();
        robot.battery = 50.0; // Not full so it stays in Charging
        robot.target = Some([5.05, 0.25, 5.0]);
        robot.target_speed = 2.0;
        robot.state = RobotState::MovingToStation;
        
        robot.update_physics(0.1, false, false);
        
        assert!(robot.target.is_none());
        assert_eq!(robot.state, RobotState::Charging);
    }

    #[test]
    fn test_battery_drains_while_moving() {
        let mut robot = make_robot();
        robot.velocity = [2.0, 0.0, 0.0]; // Moving
        
        let initial_battery = robot.battery;
        robot.update_physics(1.0, false, false);
        
        assert!(robot.battery < initial_battery);
    }

    #[test]
    fn test_battery_charges_at_station() {
        let mut robot = make_robot();
        robot.battery = 50.0;
        robot.state = RobotState::Charging;
        
        robot.update_physics(1.0, false, false);
        
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
        
        // Pickup command transitions to Picking state with pickup_timer
        assert_eq!(robot.carrying_cargo, Some(42));
        assert_eq!(robot.state, RobotState::Picking);
        assert!(robot.pickup_timer > 0.0);
    }

    #[test]
    fn test_apply_drop_command() {
        let mut robot = make_robot();
        robot.carrying_cargo = Some(42);
        robot.state = RobotState::MovingToDrop;
        
        robot.apply_command(&PathCommand::Drop);
        
        // Cargo should be dropped
        assert!(robot.carrying_cargo.is_none());
        // Drop timer should be set
        assert!(robot.drop_timer > 0.0);
        // State remains MovingToDrop until timer expires
        assert_eq!(robot.state, RobotState::MovingToDrop);
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
    fn test_return_to_charge_when_away() {
        // When robot is away from station, ReturnToCharge just makes it idle
        // (coordinator should pathfind back if needed)
        let mut robot = make_robot();
        robot.position = [10.0, 0.25, 10.0]; // Away from station
        
        robot.apply_command(&PathCommand::ReturnToCharge);
        
        assert!(robot.target.is_none()); // No direct movement
        assert_eq!(robot.state, RobotState::Idle); // Becomes idle
    }
    
    #[test]
    fn test_return_to_charge_when_at_station() {
        // When robot is at station, ReturnToCharge starts charging
        let mut robot = make_robot();
        robot.battery = 50.0;
        robot.position = [5.0, 0.25, 5.0]; // At station
        
        robot.apply_command(&PathCommand::ReturnToCharge);
        
        assert!(robot.target.is_none());
        assert_eq!(robot.state, RobotState::Charging);
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
