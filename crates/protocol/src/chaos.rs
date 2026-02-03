//! Chaos engineering helpers for testing system resilience
//! 
//! These functions inject controlled faults into the system.
//! All functions check `config::chaos::ENABLED` before activating.
//! 
//! # Usage
//! ```ignore
//! use protocol::chaos;
//! 
//! // Before publishing a message
//! if chaos::should_drop_packet() {
//!     return; // Skip this publish
//! }
//! 
//! // Before processing a command
//! if chaos::should_reject_command() {
//!     return; // Ignore this command
//! }
//! 
//! // Add latency to message processing
//! chaos::maybe_delay().await;
//! ```

use crate::config::chaos as cfg;
use rand::Rng;

/// Helper to generate random f32 in 0.0..1.0
fn random_chance() -> f32 {
    rand::random::<f32>()
}

/// Check if we should drop this packet (simulates network loss)
/// 
/// Returns `true` if the packet should be dropped.
/// Pass the runtime `chaos_enabled` flag from your crate state.
pub fn should_drop_packet(enabled: bool) -> bool {
    if !enabled || !cfg::PACKET_LOSS_ENABLED {
        return false;
    }
    random_chance() < cfg::PACKET_LOSS_RATE
}

/// Check if we should reject this command (simulates firmware issues)
/// 
/// Returns `true` if the command should be ignored.
/// Pass the runtime `chaos_enabled` flag from your crate state.
pub fn should_reject_command(enabled: bool) -> bool {
    if !enabled || !cfg::COMMAND_REJECT_ENABLED {
        return false;
    }
    random_chance() < cfg::COMMAND_REJECT_RATE
}

/// Check if we should send stale state (simulates desync)
/// 
/// Returns `true` if stale data should be sent instead of current.
/// Pass the runtime `chaos_enabled` flag from your crate state.
pub fn should_send_stale_state(enabled: bool) -> bool {
    if !enabled || !cfg::STALE_STATE_ENABLED {
        return false;
    }
    random_chance() < cfg::STALE_STATE_RATE
}

/// Check if battery sensor should glitch
/// 
/// Returns `true` if a false low battery reading should occur.
/// Pass the runtime `chaos_enabled` flag from your crate state.
pub fn should_battery_glitch(enabled: bool) -> bool {
    if !enabled || !cfg::BATTERY_GLITCH_ENABLED {
        return false;
    }
    random_chance() < cfg::BATTERY_GLITCH_RATE
}

/// Check if process should crash (use sparingly!)
/// 
/// Returns `true` if the process should terminate.
/// Pass the runtime `chaos_enabled` flag from your crate state.
pub fn should_crash(enabled: bool) -> bool {
    if !enabled || !cfg::CRASH_ENABLED {
        return false;
    }
    random_chance() < cfg::CRASH_PROBABILITY
}

/// Get random message delay in milliseconds
/// 
/// Returns 0 if chaos is disabled.
pub fn get_message_delay_ms(enabled: bool) -> u64 {
    if !enabled || !cfg::MESSAGE_DELAY_ENABLED {
        return 0;
    }
    rand::thread_rng().gen_range(cfg::MESSAGE_DELAY_MS.0..=cfg::MESSAGE_DELAY_MS.1)
}

/// Get random position drift for odometry errors
/// 
/// Returns (dx, dz) drift values in world units.
/// Returns (0.0, 0.0) if chaos is disabled.
pub fn get_position_drift(enabled: bool) -> (f32, f32) {
    if !enabled || !cfg::POSITION_DRIFT_ENABLED {
        return (0.0, 0.0);
    }
    let mut rng = rand::thread_rng();
    let dx = rng.gen_range(-cfg::POSITION_DRIFT_RANGE..=cfg::POSITION_DRIFT_RANGE);
    let dz = rng.gen_range(-cfg::POSITION_DRIFT_RANGE..=cfg::POSITION_DRIFT_RANGE);
    (dx, dz)
}

/// Log a chaos event (for debugging)
pub fn log_chaos_event(event: &str, crate_name: &str) {
    if cfg::ENABLED {
        println!("💥 [CHAOS] {}: {}", crate_name, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_disabled_returns_safe_defaults() {
        // When enabled=false, all chaos functions return safe/no-op values
        assert!(!should_drop_packet(false));
        assert!(!should_reject_command(false));
        assert!(!should_send_stale_state(false));
        assert!(!should_battery_glitch(false));
        assert!(!should_crash(false));
        assert_eq!(get_message_delay_ms(false), 0);
        assert_eq!(get_position_drift(false), (0.0, 0.0));
    }
    
    #[test]
    fn test_log_chaos_event_does_not_panic() {
        // Just verify it doesn't crash
        log_chaos_event("test event", "test_crate");
    }
}
