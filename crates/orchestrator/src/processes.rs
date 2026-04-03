//! Process management for orchestrator
//!
//! Handles spawning, killing, and monitoring of Tessera crates.

use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::cli::RunMode;
use protocol::config::orchestrator as orch_config;
use protocol::layout::{resolve_layout_path, LAYOUT_OVERRIDE_ENV};

/// List of all manageable crates in startup order
pub const CRATE_ORDER: &[&str] = &["coordinator", "mock_firmware", "scheduler", "visualizer"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    NotStarted,
    Running,
    Exited,
}

impl ProcessState {
    fn from_observation(previous: Self, observed_running: bool) -> Self {
        if observed_running {
            Self::Running
        } else {
            match previous {
                Self::NotStarted => Self::NotStarted,
                Self::Running | Self::Exited => Self::Exited,
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "⚫ not started",
            Self::Running => "🟢 running",
            Self::Exited => "🔴 exited",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessStatusEntry {
    pub name: &'static str,
    pub state: ProcessState,
    pub windowed: bool,
}

/// Managed child processes for orchestration
pub struct Processes {
    /// Tracked lifecycle state for each managed crate.
    states: HashMap<String, ProcessState>,
    /// Crates whose output should be shown in a window (others run silently)
    show_output: HashSet<String>,
    /// Active layout path used for spawning crates.
    active_layout: String,
    /// Active build profile used for spawning crates.
    active_mode: RunMode,
}

impl Processes {
    pub fn new() -> Self {
        let states = CRATE_ORDER
            .iter()
            .map(|name| (name.to_string(), ProcessState::NotStarted))
            .collect();

        Self {
            states,
            show_output: HashSet::new(), // all crates silent by default
            active_layout: resolve_layout_path(),
            active_mode: RunMode::Release,
        }
    }

    /// Enable windowed output for a crate (takes effect on next spawn)
    pub fn show_output(&mut self, name: &str) {
        self.set_output_visibility(name, true);
    }

    /// Disable windowed output for a crate (takes effect on next spawn)
    pub fn hide_output(&mut self, name: &str) {
        self.set_output_visibility(name, false);
    }

    fn set_output_visibility(&mut self, name: &str, show: bool) {
        if name == "all" {
            if show {
                for &crate_name in CRATE_ORDER {
                    self.show_output.insert(crate_name.to_string());
                }
                println!("✓ Output window enabled for all crates (takes effect on next spawn)");
            } else {
                self.show_output.clear();
                println!("✓ Output window disabled for all crates (takes effect on next spawn)");
            }
            return;
        }

        if !CRATE_ORDER.contains(&name) {
            println!("⚠ Unknown crate '{}'. Valid: {:?}", name, CRATE_ORDER);
            return;
        }

        if show {
            self.show_output.insert(name.to_string());
            println!("✓ Output window enabled for {} (takes effect on next spawn)", name);
        } else {
            self.show_output.remove(name);
            println!("✓ Output window disabled for {} (takes effect on next spawn)", name);
        }
    }

    /// Start a specific crate
    pub fn start(&mut self, name: &str, layout_path: Option<&str>, mode: RunMode) -> Result<(), String> {
        if !CRATE_ORDER.contains(&name) {
            return Err(format!("Unknown crate: '{}'. Valid: {:?}", name, CRATE_ORDER));
        }

        if let Some(path) = layout_path {
            self.active_layout = path.to_string();
        }
        self.active_mode = mode;

        match self.reconcile_process(name) {
            ProcessState::Running => {
                println!("⚠ {} is already running", name);
                return Ok(());
            }
            ProcessState::Exited => {
                println!("↻ {} had exited, starting a fresh instance", name);
            }
            ProcessState::NotStarted => {}
        }

        // build first
        println!("🔨 Building {} ({})...", name, mode.label());
        let mut build_args = vec!["build"];
        if mode == RunMode::Release {
            build_args.push("--release");
        }
        build_args.extend(["-p", name]);

        let build_status = Command::new("cargo")
            .args(build_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to build {}: {}", name, e))?;

        if !build_status.success() {
            return Err(format!("Build failed for {}", name));
        }

        spawn_binary(name, self.show_output.contains(name), &self.active_layout, mode)?;
        self.set_state(name, ProcessState::Running);
        println!("✓ {} started", name);
        println!("  Layout: {}", self.active_layout);
        println!("  Mode: {}", mode.label());
        protocol::logs::save_log("Orchestrator", &format!("Process started: {}", name));
        Ok(())
    }

    /// Start all crates in order
    pub fn start_all(&mut self, layout_path: Option<&str>, mode: RunMode) -> Result<(), String> {
        protocol::logs::save_log("Orchestrator", "Startup sequence initiated");

        if let Some(path) = layout_path {
            self.active_layout = path.to_string();
        }
        self.active_mode = mode;

        // reconcile first so command decisions use current process truth.
        self.reconcile_all();

        // kill currently running crates before a full startup sequence.
        if self.any_running() {
            self.kill_all();
        } else {
            // remove stale exited markers from previous runs.
            self.clear_exited_states();
        }

        println!("🔨 Building all crates ({})...", mode.label());

        // build all four crates in a single invocation so Cargo can unify the dependency
        // graph (ring/rustls/quinn) in one pass. two separate `cargo build` calls cause
        // ring's build script to re-run on the second call because the target/ directory
        // was modified by the first, marking its fingerprint dirty.
        let mut build_args = vec!["build"];
        if mode == RunMode::Release {
            build_args.push("--release");
        }
        build_args.extend([
            "-p",
            "coordinator",
            "-p",
            "mock_firmware",
            "-p",
            "scheduler",
            "-p",
            "visualizer",
        ]);

        let build_status = Command::new("cargo")
            .args(build_args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to build: {}", e))?;

        if !build_status.success() {
            return Err("Build failed".to_string());
        }

        println!("✓ Build complete");
        println!("📦 Active layout: {}", self.active_layout);
        println!("⚙ Active mode: {}", mode.label());
        // play in background without risking startup failure
        play_startup_sound();
        println!("🚀 Starting all crates in order...");

        // 1. Coordinator - must start first, broadcasts map hash
        println!("  1/4 Starting coordinator...");
        spawn_binary(
            "coordinator",
            self.show_output.contains("coordinator"),
            &self.active_layout,
            mode,
        )?;
        self.set_state("coordinator", ProcessState::Running);
        thread::sleep(Duration::from_millis(orch_config::COORDINATOR_STARTUP_DELAY_MS));

        // 2. Firmware (mock_firmware) - validates map hash, starts physics
        println!("  2/4 Starting mock_firmware...");
        spawn_binary(
            "mock_firmware",
            self.show_output.contains("mock_firmware"),
            &self.active_layout,
            mode,
        )?;
        self.set_state("mock_firmware", ProcessState::Running);
        thread::sleep(Duration::from_millis(orch_config::FIRMWARE_STARTUP_DELAY_MS));

        // 3. Scheduler - task queue
        println!("  3/4 Starting scheduler...");
        spawn_binary(
            "scheduler",
            self.show_output.contains("scheduler"),
            &self.active_layout,
            mode,
        )?;
        self.set_state("scheduler", ProcessState::Running);
        thread::sleep(Duration::from_millis(orch_config::SCHEDULER_STARTUP_DELAY_MS));

        // 4. Renderer (visualizer) - Bevy window
        println!("  4/4 Starting visualizer...");
        spawn_binary(
            "visualizer",
            self.show_output.contains("visualizer"),
            &self.active_layout,
            mode,
        )?;
        self.set_state("visualizer", ProcessState::Running);

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

        match self.reconcile_process(name) {
            ProcessState::Running => {
                if kill_process_by_name(name) {
                    self.set_state(name, ProcessState::NotStarted);
                    println!("✓ {} killed", name);
                    protocol::logs::save_log("Orchestrator", &format!("Process terminated: {}", name));
                    return Ok(());
                }

                match self.reconcile_process(name) {
                    ProcessState::Running => {
                        return Err(format!("Failed to kill {}", name));
                    }
                    ProcessState::Exited | ProcessState::NotStarted => {
                        self.set_state(name, ProcessState::NotStarted);
                        println!("✓ {} was already stopped", name);
                    }
                }
            }
            ProcessState::Exited => {
                self.set_state(name, ProcessState::NotStarted);
                println!("✓ {} was already stopped", name);
            }
            ProcessState::NotStarted => {
                println!("⚠ {} was not running", name);
            }
        }
        Ok(())
    }

    /// Kill all managed processes
    pub fn kill_all(&mut self) {
        protocol::logs::save_log("Orchestrator", "Killing all processes");
        self.reconcile_all();

        let mut running = Vec::new();
        for &name in CRATE_ORDER {
            if self.process_state(name) == ProcessState::Running {
                running.push(name);
            }
        }

        if running.is_empty() {
            let cleared_exited = self.clear_exited_states();
            if cleared_exited > 0 {
                println!(
                    "No running managed processes. Cleared {} exited state entr{}.",
                    cleared_exited,
                    if cleared_exited == 1 { "y" } else { "ies" }
                );
            } else {
                println!("No managed processes to kill.");
            }
            return;
        }

        println!("🛑 Killing all managed processes...");

        // kill in reverse startup order.
        for &name in running.iter().rev() {
            if kill_process_by_name(name) {
                self.set_state(name, ProcessState::NotStarted);
                println!("  ✓ {} killed", name);
            } else {
                // if it disappeared between status and kill, treat as already stopped.
                let reconciled = self.reconcile_process(name);
                if reconciled == ProcessState::Running {
                    println!("  ⚠ failed to kill {}", name);
                } else {
                    self.set_state(name, ProcessState::NotStarted);
                    println!("  ✓ {} was already stopped", name);
                }
            }
        }

        let cleared_exited = self.clear_exited_states();
        if cleared_exited > 0 {
            println!(
                "  ✓ cleared {} exited state entr{}",
                cleared_exited,
                if cleared_exited == 1 { "y" } else { "ies" }
            );
        }

        println!("✓ All processes killed");
        protocol::logs::save_log("Orchestrator", "All processes terminated");
    }

    /// Restart all crates
    pub fn restart_all(&mut self) -> Result<(), String> {
        protocol::logs::save_log("Orchestrator", "Restart initiated");
        println!("🔄 Restarting all crates...");
        self.kill_all();
        thread::sleep(Duration::from_millis(orch_config::RESTART_DELAY_MS));
        let layout = self.active_layout.clone();
        let mode = self.active_mode;
        self.start_all(Some(&layout), mode)?;
        protocol::logs::save_log("Orchestrator", "Restart completed");
        Ok(())
    }

    /// Build a status snapshot for all managed processes.
    pub fn status_snapshot(&mut self) -> Vec<ProcessStatusEntry> {
        self.reconcile_all();
        CRATE_ORDER
            .iter()
            .map(|&name| ProcessStatusEntry {
                name,
                state: self.process_state(name),
                windowed: self.show_output.contains(name),
            })
            .collect()
    }

    fn process_state(&self, name: &str) -> ProcessState {
        self.states
            .get(name)
            .copied()
            .unwrap_or(ProcessState::NotStarted)
    }

    fn set_state(&mut self, name: &str, state: ProcessState) {
        if let Some(slot) = self.states.get_mut(name) {
            *slot = state;
        }
    }

    fn reconcile_process(&mut self, name: &str) -> ProcessState {
        let previous = self.process_state(name);
        let observed_running = is_process_running(name);
        let next = ProcessState::from_observation(previous, observed_running);
        self.set_state(name, next);
        next
    }

    fn reconcile_all(&mut self) {
        for &name in CRATE_ORDER {
            self.reconcile_process(name);
        }
    }

    fn any_running(&self) -> bool {
        self.states
            .values()
            .any(|state| *state == ProcessState::Running)
    }

    fn clear_exited_states(&mut self) -> usize {
        let mut cleared = 0;
        for state in self.states.values_mut() {
            if *state == ProcessState::Exited {
                *state = ProcessState::NotStarted;
                cleared += 1;
            }
        }
        cleared
    }
}

fn play_startup_sound() {
    thread::spawn(|| {
        let result = std::panic::catch_unwind(notifier::play_default);
        if result.is_err() {
            eprintln!("⚠ Startup sound failed");
        }
    });
}

impl Default for Processes {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Processes {
    fn drop(&mut self) {
        if self
            .states
            .values()
            .any(|state| *state != ProcessState::NotStarted)
        {
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
            .args(["/FI", &format!("IMAGENAME eq {}", exe_name), "/FO", "CSV", "/NH"])
            .output();
        match output {
            Ok(o) => {
                let target = format!("\"{}\"", exe_name.to_ascii_lowercase());
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|line| line.to_ascii_lowercase().starts_with(&target))
            }
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
fn spawn_binary(name: &str, windowed: bool, layout_path: &str, mode: RunMode) -> Result<(), String> {
    let profile = match mode {
        RunMode::Release => "release",
        RunMode::Dev => "debug",
    };

    #[cfg(windows)]
    {
        let binary = format!("target\\{}\\{}.exe", profile, name);
        if windowed {
            // open in a new console window so the user can see output
            Command::new("cmd")
                .args(["/c", "start", "", &binary])
                .env(LAYOUT_OVERRIDE_ENV, layout_path)
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
                .env(LAYOUT_OVERRIDE_ENV, layout_path)
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
                .env(LAYOUT_OVERRIDE_ENV, layout_path)
                .spawn()
                .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        } else {
            Command::new(&binary)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .env(LAYOUT_OVERRIDE_ENV, layout_path)
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
        assert_eq!(p.states.len(), CRATE_ORDER.len());
        assert!(p
            .states
            .values()
            .all(|state| *state == ProcessState::NotStarted));
    }

    #[test]
    fn test_process_state_from_observation() {
        assert_eq!(
            ProcessState::from_observation(ProcessState::NotStarted, false),
            ProcessState::NotStarted
        );
        assert_eq!(
            ProcessState::from_observation(ProcessState::NotStarted, true),
            ProcessState::Running
        );
        assert_eq!(
            ProcessState::from_observation(ProcessState::Running, false),
            ProcessState::Exited
        );
        assert_eq!(
            ProcessState::from_observation(ProcessState::Exited, false),
            ProcessState::Exited
        );
        assert_eq!(
            ProcessState::from_observation(ProcessState::Exited, true),
            ProcessState::Running
        );
    }

    #[test]
    fn test_clear_exited_states() {
        let mut p = Processes::new();
        p.set_state("coordinator", ProcessState::Exited);
        p.set_state("scheduler", ProcessState::Running);

        assert_eq!(p.clear_exited_states(), 1);
        assert_eq!(p.process_state("coordinator"), ProcessState::NotStarted);
        assert_eq!(p.process_state("scheduler"), ProcessState::Running);
    }
}
