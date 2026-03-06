//! Process management for orchestrator
//!
//! Handles spawning, killing, and monitoring of Hyper-Twin crates.

use std::process::{Command, Stdio};
use std::time::Duration;
use std::thread;
use std::collections::HashSet;
use protocol::config::orchestrator as orch_config;

/// List of all manageable crates in startup order
pub const CRATE_ORDER: &[&str] = &["coordinator", "mock_firmware", "scheduler", "visualizer"];

/// Managed child processes for orchestration
pub struct Processes {
    /// Track running process names
    running: Vec<String>,
    /// Crates whose output should be shown in a window (others run silently)
    show_output: HashSet<String>,
}

impl Processes {
    pub fn new() -> Self {
        Self {
            running: Vec::new(),
            show_output: HashSet::new(), // all crates silent by default
        }
    }

    /// Enable windowed output for a crate (takes effect on next spawn)
    pub fn show_output(&mut self, name: &str) {
        if name == "all" {
            for &crate_name in CRATE_ORDER {
                self.show_output.insert(crate_name.to_string());
            }
            println!("✓ Output window enabled for all crates (takes effect on next spawn)");
        } else if CRATE_ORDER.contains(&name) {
            self.show_output.insert(name.to_string());
            println!("✓ Output window enabled for {} (takes effect on next spawn)", name);
        } else {
            println!("⚠ Unknown crate '{}'. Valid: {:?}", name, CRATE_ORDER);
        }
    }

    /// Disable windowed output for a crate (takes effect on next spawn)
    pub fn hide_output(&mut self, name: &str) {
        if name == "all" {
            self.show_output.clear();
            println!("✓ Output window disabled for all crates (takes effect on next spawn)");
        } else if CRATE_ORDER.contains(&name) {
            self.show_output.remove(name);
            println!("✓ Output window disabled for {} (takes effect on next spawn)", name);
        } else {
            println!("⚠ Unknown crate '{}'. Valid: {:?}", name, CRATE_ORDER);
        }
    }

    /// Returns the current output visibility set (for status display)
    pub fn output_set(&self) -> &HashSet<String> {
        &self.show_output
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

        spawn_binary(name, self.show_output.contains(name))?;
        self.running.push(name.to_string());
        println!("✓ {} started", name);
        protocol::logs::save_log("Orchestrator", &format!("Process started: {}", name));
        Ok(())
    }

    /// Start all crates in order
    pub fn start_all(&mut self) -> Result<(), String> {
        protocol::logs::save_log("Orchestrator", "Startup sequence initiated");
        // First kill any existing processes
        if !self.running.is_empty() {
            self.kill_all();
        }

        println!("🔨 Building all crates...");
        let build_status = Command::new("cargo")
            .args(["build", "-p", "coordinator", "-p", "mock_firmware", "-p", "scheduler", "-p", "visualizer"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to build: {}", e))?;

        if !build_status.success() {
            return Err("Build failed".to_string());
        }

        println!("✓ Build complete");
        // play in background — don't stall the startup sequence
        thread::spawn(|| notifier::play_default());
        println!("🚀 Starting all crates in order...");

        // 1. Coordinator - must start first, broadcasts map hash
        println!("  1/4 Starting coordinator...");
        spawn_binary("coordinator", self.show_output.contains("coordinator"))?;
        self.running.push("coordinator".to_string());
        thread::sleep(Duration::from_millis(orch_config::COORDINATOR_STARTUP_DELAY_MS));

        // 2. Firmware (mock_firmware) - validates map hash, starts physics
        println!("  2/4 Starting mock_firmware (firmware)...");
        spawn_binary("mock_firmware", self.show_output.contains("mock_firmware"))?;
        self.running.push("mock_firmware".to_string());
        thread::sleep(Duration::from_millis(orch_config::FIRMWARE_STARTUP_DELAY_MS));

        // 3. Scheduler - task queue
        println!("  3/4 Starting scheduler...");
        spawn_binary("scheduler", self.show_output.contains("scheduler"))?;
        self.running.push("scheduler".to_string());
        thread::sleep(Duration::from_millis(orch_config::SCHEDULER_STARTUP_DELAY_MS));

        // 4. Renderer (visualizer) - Bevy window
        println!("  4/4 Starting visualizer (renderer)...");
        spawn_binary("visualizer", self.show_output.contains("visualizer"))?;
        self.running.push("visualizer".to_string());

        println!("✓ All crates started successfully");
        protocol::logs::save_log("Orchestrator", "All crates started successfully");
        Ok(())
    }

    /// Kill a specific crate
    pub fn kill(&mut self, name: &str) -> Result<(), String> {
        protocol::logs::save_log("Orchestrator", &format!("Killing process: {}", name));
        if !CRATE_ORDER.contains(&name) {
            return Err(format!("Unknown crate: '{}'. Valid: {:?}", name, CRATE_ORDER));
        }

        if kill_process_by_name(name) {
            self.running.retain(|n| n != name);
            println!("✓ {} killed", name);
            protocol::logs::save_log("Orchestrator", &format!("Process terminated: {}", name));
        } else {
            println!("⚠ {} was not running", name);
        }
        Ok(())
    }

    /// Kill all managed processes
    pub fn kill_all(&mut self) {
        protocol::logs::save_log("Orchestrator", "Killing all processes");
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
        protocol::logs::save_log("Orchestrator", "All processes terminated");
    }

    /// Restart all crates
    pub fn restart_all(&mut self) -> Result<(), String> {
        protocol::logs::save_log("Orchestrator", "Restart initiated");
        println!("🔄 Restarting all crates...");
        self.kill_all();
        thread::sleep(Duration::from_millis(orch_config::RESTART_DELAY_MS));
        self.start_all()?;
        protocol::logs::save_log("Orchestrator", "Restart completed");
        Ok(())
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
        if !self.running.is_empty() {
            self.kill_all();
        }
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

/// Spawn a pre-built binary, optionally in a visible window.
/// When `windowed` is true, opens in a new console window (user can see output).
/// When false, runs silently in the background with no window.
fn spawn_binary(name: &str, windowed: bool) -> Result<(), String> {
    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";

    #[cfg(windows)]
    {
        let binary = format!("target\\{}\\{}.exe", profile, name);
        if windowed {
            // open in a new console window so the user can see output
            Command::new("cmd")
                .args(["/c", "start", name, &binary])
                .spawn()
                .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        } else {
            // run silently: no window, stdio suppressed
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            Command::new(&binary)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let binary = format!("target/{}/{}", profile, name);
        if windowed {
            Command::new(&binary)
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        } else {
            Command::new(&binary)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_order_contains_all() {
        assert!(CRATE_ORDER.contains(&"coordinator"));
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
