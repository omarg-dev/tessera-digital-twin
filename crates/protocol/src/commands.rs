//! Command types sent between crates

use serde::{Deserialize, Serialize};

/// Path command from coordinator to a specific robot
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathCmd {
    pub cmd_id: u64,
    pub robot_id: u32,
    pub command: PathCommand,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PathCommand {
    /// Move to target position with given velocity
    MoveTo { target: [f32; 3], speed: f32 },
    /// Move specifically to a pickup location (sets state accordingly)
    MoveToPickup { target: [f32; 3], speed: f32 },
    /// Move specifically to a dropoff location (sets state accordingly)
    MoveToDropoff { target: [f32; 3], speed: f32 },
    /// Follow a sequence of waypoints continuously without stopping between them.
    /// Firmware advances through waypoints internally; coordinator sends the full
    /// remaining path in one shot. State is inferred from cargo (same as MoveTo).
    FollowPath { waypoints: Vec<[f32; 3]>, speed: f32 },
    /// Follow a sequence of waypoints back to the home charging station.
    /// Sets RobotState::MovingToStation so the visualizer can show the correct label.
    ReturnToStation { waypoints: Vec<[f32; 3]>, speed: f32 },
    /// Stop immediately and clear any queued waypoints
    Stop,
    /// Mark the robot as faulted: stop all movement and set RobotState::Faulted.
    /// Sent by the coordinator when a robot exceeds blocked/replan thresholds.
    Fault,
    /// Pick up cargo at current location
    Pickup { cargo_id: u32 },
    /// Drop cargo at current location
    Drop,
    /// Go to charging station
    ReturnToCharge,
}

/// Response from firmware after applying a command
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandResponse {
    pub cmd_id: u64,
    pub robot_id: u32,
    pub status: CommandStatus,
}

impl CommandResponse {
    /// Create a successful command response.
    pub fn accepted(cmd_id: u64, robot_id: u32) -> Self {
        Self {
            cmd_id,
            robot_id,
            status: CommandStatus::Accepted,
        }
    }

    /// Create a rejected command response with a reason.
    pub fn rejected(cmd_id: u64, robot_id: u32, reason: impl Into<String>) -> Self {
        Self {
            cmd_id,
            robot_id,
            status: CommandStatus::Rejected {
                reason: reason.into(),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum CommandStatus {
    /// Command accepted and executing
    Accepted,
    /// Command rejected (with reason)
    Rejected { reason: String },
}

/// System-wide control commands (orchestrator → all)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SystemCommand {
    Pause,
    Resume,
    /// Set verbose mode globally
    Verbose(bool),
    /// Toggle chaos engineering mode
    Chaos(bool),
    /// Set simulation time scale (1.0 = real-time, 2.0 = 2x speed, etc.)
    /// Clamped to 0.1..1000.0 by consumers.
    SetTimeScale(f32),
}

/// Individual robot control (orchestrator → firmware)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RobotControl {
    /// Disable a robot (stops publishing updates, ignores commands)
    Down(u32),
    /// Enable a disabled robot
    Up(u32),
    /// Restart a robot (reset to initial state at station)
    Restart(u32),
}

impl RobotControl {
    /// Return the target robot id for this control command.
    pub fn id(&self) -> u32 {
        match self {
            RobotControl::Down(id) | RobotControl::Up(id) | RobotControl::Restart(id) => *id,
        }
    }
}

/// Result of applying a system command - tells caller what changed
#[derive(Debug, Clone, PartialEq)]
pub enum SystemCommandEffect {
    /// Paused state changed
    Paused(bool),
    /// Verbose state changed  
    Verbose(bool),
    /// Chaos mode changed
    Chaos(bool),
    /// Time scale changed
    TimeScale(f32),
    /// No state change needed for this crate
    None,
}
impl PathCommand {
    /// Validate a movement target and speed payload.
    pub fn is_valid_target(target: [f32; 3], speed: f32) -> bool {
        target[0].is_finite() && target[2].is_finite() && speed.is_finite() && speed > 0.0
    }
}

impl SystemCommand {
    /// Apply the command and return what effect it had.
    /// Caller provides mutable refs to the state they care about.
    /// Pass None for state you don't track (e.g., visualizer doesn't track verbose).
    pub fn apply(
        &self,
        paused: Option<&mut bool>,
        verbose: Option<&mut bool>,
        chaos: Option<&mut bool>,
    ) -> SystemCommandEffect {
        match self {
            SystemCommand::Pause => {
                if let Some(p) = paused {
                    *p = true;
                }
                SystemCommandEffect::Paused(true)
            }
            SystemCommand::Resume => {
                if let Some(p) = paused {
                    *p = false;
                }
                SystemCommandEffect::Paused(false)
            }
            SystemCommand::Verbose(v) => {
                if let Some(verb) = verbose {
                    *verb = *v;
                }
                SystemCommandEffect::Verbose(*v)
            }
            SystemCommand::Chaos(c) => {
                if let Some(ch) = chaos {
                    *ch = *c;
                }
                SystemCommandEffect::Chaos(*c)
            }
            // time scale is handled separately by consumers that track it
            SystemCommand::SetTimeScale(s) => SystemCommandEffect::TimeScale(*s),
        }
    }

    /// Convenience: apply and print a standard message with crate context
    pub fn apply_with_log(
        &self,
        crate_name: &str,
        paused: Option<&mut bool>,
        verbose: Option<&mut bool>,
        chaos: Option<&mut bool>,
    ) -> SystemCommandEffect {
        let effect = self.apply(paused, verbose, chaos);
        match &effect {
            SystemCommandEffect::Paused(true) => println!("⏸ {} PAUSED", crate_name),
            SystemCommandEffect::Paused(false) => println!("▶ {} RESUMED", crate_name),
            SystemCommandEffect::Verbose(true) => println!("🔊 {} verbose ON", crate_name),
            SystemCommandEffect::Verbose(false) => println!("🔇 {} verbose OFF", crate_name),
            SystemCommandEffect::Chaos(true) => println!("💥 {} chaos ON", crate_name),
            SystemCommandEffect::Chaos(false) => println!("✨ {} chaos OFF", crate_name),
            SystemCommandEffect::TimeScale(s) => println!("⏱️ {} time scale: {:.1}x", crate_name, s),
            SystemCommandEffect::None => {}
        }
        effect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_command_pause() {
        let mut paused = false;
        let effect = SystemCommand::Pause.apply(Some(&mut paused), None, None);
        
        assert!(paused);
        assert_eq!(effect, SystemCommandEffect::Paused(true));
    }

    #[test]
    fn test_system_command_resume() {
        let mut paused = true;
        let effect = SystemCommand::Resume.apply(Some(&mut paused), None, None);
        
        assert!(!paused);
        assert_eq!(effect, SystemCommandEffect::Paused(false));
    }

    #[test]
    fn test_system_command_verbose_on() {
        let mut verbose = false;
        let effect = SystemCommand::Verbose(true).apply(None, Some(&mut verbose), None);
        
        assert!(verbose);
        assert_eq!(effect, SystemCommandEffect::Verbose(true));
    }

    #[test]
    fn test_system_command_verbose_off() {
        let mut verbose = true;
        let effect = SystemCommand::Verbose(false).apply(None, Some(&mut verbose), None);
        
        assert!(!verbose);
        assert_eq!(effect, SystemCommandEffect::Verbose(false));
    }

    #[test]
    fn test_system_command_chaos_on() {
        let mut chaos = false;
        let effect = SystemCommand::Chaos(true).apply(None, None, Some(&mut chaos));
        
        assert!(chaos);
        assert_eq!(effect, SystemCommandEffect::Chaos(true));
    }

    #[test]
    fn test_system_command_chaos_off() {
        let mut chaos = true;
        let effect = SystemCommand::Chaos(false).apply(None, None, Some(&mut chaos));
        
        assert!(!chaos);
        assert_eq!(effect, SystemCommandEffect::Chaos(false));
    }

    #[test]
    fn test_system_command_with_none_state() {
        // When no state is passed, command still returns effect but doesn't mutate
        let effect = SystemCommand::Pause.apply(None, None, None);
        assert_eq!(effect, SystemCommandEffect::Paused(true));
    }

    #[test]
    fn test_path_command_serialization() {
        let cmd = PathCmd {
            robot_id: 42,
            cmd_id: 1,
            command: PathCommand::MoveTo { target: [1.0, 0.25, 2.0], speed: 2.0 },
        };
        
        let json = serde_json::to_string(&cmd).expect("PathCmd should serialize");
        let parsed: PathCmd = serde_json::from_str(&json).expect("PathCmd should deserialize");
        
        assert_eq!(parsed.robot_id, 42);
        assert!(matches!(
            parsed.command,
            PathCommand::MoveTo { target: [1.0, 0.25, 2.0], speed: 2.0 }
        ));
    }
}
