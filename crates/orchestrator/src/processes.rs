//! Process management for orchestrator
//!
//! Handles spawning, killing, and monitoring of Hyper-Twin crates.

use std::process::{Command, Stdio};
use std::time::Duration;
use std::thread;
use protocol::config::orchestrator as orch_config;

/// List of all manageable crates in startup order
pub const CRATE_ORDER: &[&str] = &["fleet_server", "mock_firmware", "scheduler", "visualizer"];

/// Managed child processes for orchestration
pub struct Processes {
    /// Track running process names
    running: Vec<String>,
}

impl Processes {
    pub fn new() -> Self {
        Self { running: Vec::new() }
    }

    /// Start a specific crate
    pub fn start(&mut self, name: &str) -> Result<(), String> {
        if !CRATE_ORDER.contains(&name) {
            return Err(format!("Unknown crate: '{}'. Valid: {:?}", name, CRATE_ORDER));
        }
        
        if self.running.contains(&name.to_string()) {
            println!("⚠ {} is already running", name);
            return Ok(());
        }

        // Build first
        println!("🔨 Building {}...", name);
        let build_status = Command::new("cargo")
            .args(["build", "-p", name])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to build {}: {}", name, e))?;

        if !build_status.success() {
            return Err(format!("Build failed for {}", name));
        }

        spawn_binary(name)?;
        self.running.push(name.to_string());
        println!("✓ {} started", name);
        Ok(())
    }

    /// Start all crates in order
    pub fn start_all(&mut self) -> Result<(), String> {
        // First kill any existing processes
        if !self.running.is_empty() {
            self.kill_all();
        }

        println!("🔨 Building all crates...");
        let build_status = Command::new("cargo")
            .args(["build", "-p", "fleet_server", "-p", "mock_firmware", "-p", "scheduler", "-p", "visualizer"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to build: {}", e))?;

        if !build_status.success() {
            return Err("Build failed".to_string());
        }

        println!("✓ Build complete");
        println!("🚀 Starting all crates in order...");

        // 1. Coordinator (fleet_server) - must start first, broadcasts map hash
        println!("  1/4 Starting fleet_server (coordinator)...");
        spawn_binary("fleet_server")?;
        self.running.push("fleet_server".to_string());
        thread::sleep(Duration::from_millis(orch_config::COORDINATOR_STARTUP_DELAY_MS));

        // 2. Firmware (mock_firmware) - validates map hash, starts physics
        println!("  2/4 Starting mock_firmware (firmware)...");
        spawn_binary("mock_firmware")?;
        self.running.push("mock_firmware".to_string());
        thread::sleep(Duration::from_millis(orch_config::FIRMWARE_STARTUP_DELAY_MS));

        // 3. Scheduler - task queue
        println!("  3/4 Starting scheduler...");
        spawn_binary("scheduler")?;
        self.running.push("scheduler".to_string());
        thread::sleep(Duration::from_millis(orch_config::SCHEDULER_STARTUP_DELAY_MS));

        // 4. Renderer (visualizer) - Bevy window
        println!("  4/4 Starting visualizer (renderer)...");
        spawn_binary("visualizer")?;
        self.running.push("visualizer".to_string());

        println!("✓ All crates started successfully");
        Ok(())
    }

    /// Kill a specific crate
    pub fn kill(&mut self, name: &str) -> Result<(), String> {
        if !CRATE_ORDER.contains(&name) {
            return Err(format!("Unknown crate: '{}'. Valid: {:?}", name, CRATE_ORDER));
        }

        if kill_process_by_name(name) {
            self.running.retain(|n| n != name);
            println!("✓ {} killed", name);
        } else {
            println!("⚠ {} was not running", name);
        }
        Ok(())
    }

    /// Kill all managed processes
    pub fn kill_all(&mut self) {
        if self.running.is_empty() {
            println!("No managed processes to kill.");
            return;
        }

        println!("🛑 Killing all managed processes...");

        // Kill in reverse order
        for name in self.running.iter().rev() {
            if kill_process_by_name(name) {
                println!("  ✓ {} killed", name);
            } else {
                println!("  ⚠ {} may not have been running", name);
            }
        }

        self.running.clear();
        println!("✓ All processes killed");
    }

    /// Restart all crates
    pub fn restart_all(&mut self) -> Result<(), String> {
        println!("🔄 Restarting all crates...");
        self.kill_all();
        thread::sleep(Duration::from_millis(orch_config::RESTART_DELAY_MS));
        self.start_all()
    }

    /// Get list of running processes
    pub fn running(&self) -> &[String] {
        &self.running
    }
}

impl Default for Processes {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Processes {
    fn drop(&mut self) {
        self.kill_all();
    }
}

/// Kill a process by name using platform-specific commands
fn kill_process_by_name(name: &str) -> bool {
    #[cfg(windows)]
    {
        let exe_name = format!("{}.exe", name);
        Command::new("taskkill")
            .args(["/F", "/IM", &exe_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        Command::new("pkill")
            .args(["-f", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Check if a process is running by name
pub fn is_process_running(name: &str) -> bool {
    #[cfg(windows)]
    {
        let exe_name = format!("{}.exe", name);
        let output = Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {}", exe_name)])
            .output();
        match output {
            Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&exe_name),
            Err(_) => false,
        }
    }

    #[cfg(not(windows))]
    {
        Command::new("pgrep")
            .args(["-f", name])
            .stdout(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Spawn a pre-built binary in its own terminal window
fn spawn_binary(name: &str) -> Result<(), String> {
    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";

    #[cfg(windows)]
    {
        let binary = format!("target\\{}\\{}.exe", profile, name);
        Command::new("cmd")
            .args(["/c", "start", name, &binary])
            .spawn()
            .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let binary = format!("target/{}/{}", profile, name);
        Command::new(&binary)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_order_contains_all() {
        assert!(CRATE_ORDER.contains(&"fleet_server"));
        assert!(CRATE_ORDER.contains(&"mock_firmware"));
        assert!(CRATE_ORDER.contains(&"scheduler"));
        assert!(CRATE_ORDER.contains(&"visualizer"));
    }

    #[test]
    fn test_processes_new_empty() {
        let p = Processes::new();
        assert!(p.running().is_empty());
    }
}
