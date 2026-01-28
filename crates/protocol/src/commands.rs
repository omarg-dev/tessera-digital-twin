//! Command types sent between crates

use serde::{Deserialize, Serialize};

/// Path command from fleet_server to a specific robot
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathCmd {
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

/// System-wide control commands (control_plane → all)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SystemCommand {
    Pause,
    Resume,
    /// Set verbose mode globally
    Verbose(bool),
}

/// Result of applying a system command - tells caller what changed
#[derive(Debug, Clone, PartialEq)]
pub enum SystemCommandEffect {
    /// Paused state changed
    Paused(bool),
    /// Verbose state changed  
    Verbose(bool),
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
        }
    }

    /// Convenience: apply and print a standard message with crate context
    pub fn apply_with_log(
        &self,
        crate_name: &str,
        paused: Option<&mut bool>,
        verbose: Option<&mut bool>,
    ) -> SystemCommandEffect {
        let effect = self.apply(paused, verbose);
        match &effect {
            SystemCommandEffect::Paused(true) => println!("⏸ {} PAUSED", crate_name),
            SystemCommandEffect::Paused(false) => println!("▶ {} RESUMED", crate_name),
            SystemCommandEffect::Verbose(true) => println!("🔊 {} verbose ON", crate_name),
            SystemCommandEffect::Verbose(false) => println!("🔇 {} verbose OFF", crate_name),
            SystemCommandEffect::None => {}
        }
        effect
    }
}
