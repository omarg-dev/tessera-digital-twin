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
                // Move toward target
                self.velocity[0] = (dx / dist) * physics::ROBOT_SPEED;
                self.velocity[2] = (dz / dist) * physics::ROBOT_SPEED;
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
    
    /// Apply a path command from fleet_server
    pub fn apply_command(&mut self, cmd: &PathCommand) {
        match cmd {
            PathCommand::MoveTo { target, speed: _ } => {
                self.target = Some(*target);
                // Default movement with unknown intent
                if self.carrying_cargo.is_some() {
                    self.state = RobotState::MovingToDrop;
                } else {
                    self.state = RobotState::MovingToPickup;
                }
            }
            PathCommand::MoveToPickup { target, speed: _ } => {
                self.target = Some(*target);
                self.state = RobotState::MovingToPickup;
            }
            PathCommand::MoveToDropoff { target, speed: _ } => {
                self.target = Some(*target);
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
                self.state = RobotState::MovingToStation;
            }
        }
    }
}
