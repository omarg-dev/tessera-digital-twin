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
    /// Stop immediately
    Stop,
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

/// Result of applying a system command - tells caller what changed
#[derive(Debug, Clone, PartialEq)]
pub enum SystemCommandEffect {
    /// Paused state changed
    Paused(bool),
    /// Verbose state changed  
    Verbose(bool),
    /// Chaos mode changed
    Chaos(bool),
    /// No state change needed for this crate
    None,
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
        
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: PathCmd = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.robot_id, 42);
        match parsed.command {
            PathCommand::MoveTo { target, speed } => {
                assert_eq!(target, [1.0, 0.25, 2.0]);
                assert_eq!(speed, 2.0);
            }
            _ => panic!("Wrong command type"),
        }
    }
}
