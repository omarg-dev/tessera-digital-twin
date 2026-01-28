//! Control Plane - Single Interface for System Commands
//!
//! This is the ONLY crate that can issue system commands (pause/resume/reset/kill).
//! All other crates LISTEN for these commands but never originate them.
//!
//! Architecture:
//! - control_plane: Publishes SystemCommand to ADMIN_CONTROL
//! - mission_control, fleet_server, swarm_driver, visualizer: Subscribe and respond
//!
//! ## TODO: UI Improvements
//! - [ ] Integrate control panel into Bevy visualizer (egui sidebar)
//! - [ ] Add real-time status dashboard (robot count, task queue, system state)
//! - [ ] Web-based control panel for production deployments
//! - [ ] Keyboard shortcuts in visualizer (P=pause, R=resume, etc.)

use protocol::{SystemCommand, topics};
use serde_json::to_vec;
use std::process::{Command, Stdio};
use tokio::io::{self, AsyncBufReadExt, BufReader};

/// Managed child processes for start-all functionality
struct ManagedProcesses {
    /// Track process names for taskkill on Windows
    running: Vec<String>,
}

impl ManagedProcesses {
    fn new() -> Self {
        Self {
            running: Vec::new(),
        }
    }

    fn start_all(&mut self) -> Result<(), String> {
        // First stop any existing processes
        if !self.running.is_empty() {
            self.stop_all();
        }
        
        println!("🔨 Building all crates...");
        
        // Build all crates in one go to avoid cargo lock contention
        let build_status = Command::new("cargo")
            .args(["build", "-p", "fleet_server", "-p", "swarm_driver", "-p", "mission_control", "-p", "visualizer"])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("Failed to build: {}", e))?;
        
        if !build_status.success() {
            return Err("Build failed".to_string());
        }
        
        println!("✓ Build complete");
        println!("🚀 Starting all crates in order...");
        
        // 1. Fleet Server (must start first - broadcasts map hash)
        println!("  1/4 Starting fleet_server...");
        spawn_binary("fleet_server")?;
        self.running.push("fleet_server".to_string());
        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        // 2. Swarm Driver (validates map hash, starts physics)
        println!("  2/4 Starting swarm_driver...");
        spawn_binary("swarm_driver")?;
        self.running.push("swarm_driver".to_string());
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        // 3. Mission Control (task queue)
        println!("  3/4 Starting mission_control...");
        spawn_binary("mission_control")?;
        self.running.push("mission_control".to_string());
        std::thread::sleep(std::time::Duration::from_millis(300));
        
        // 4. Visualizer (Bevy window)
        println!("  4/4 Starting visualizer...");
        spawn_binary("visualizer")?;
        self.running.push("visualizer".to_string());
        
        println!("✓ All crates started successfully");
        println!("  Use 'status' to check running processes");
        Ok(())
    }

    fn stop_all(&mut self) {
        if self.running.is_empty() {
            println!("No managed processes to stop.");
            return;
        }
        
        println!("🛑 Stopping all managed processes...");
        
        // Kill in reverse order using taskkill on Windows
        for name in self.running.iter().rev() {
            if kill_process_by_name(name) {
                println!("  ✓ {} stopped", name);
            } else {
                println!("  ⚠ {} may not have been running", name);
            }
        }
        
        self.running.clear();
        println!("✓ All processes stopped");
    }

    fn print_status(&self) {
        println!("╭─────────────────────────────────────────╮");
        println!("│  PROCESS STATUS                         │");
        println!("├─────────────────────────────────────────┤");
        
        let processes = ["fleet_server", "swarm_driver", "mission_control", "visualizer"];
        for name in &processes {
            let status = if is_process_running(name) {
                "running"
            } else if self.running.contains(&name.to_string()) {
                "exited"
            } else {
                "not started"
            };
            println!("│  {:17} {:18} │", format!("{}:", name), status);
        }
        println!("╰─────────────────────────────────────────╯");
    }
}

impl Drop for ManagedProcesses {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Kill a process by name using platform-specific commands
fn kill_process_by_name(name: &str) -> bool {
    #[cfg(windows)]
    {
        // Use taskkill to forcefully terminate by image name
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
        // Use pkill on Unix
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
fn is_process_running(name: &str) -> bool {
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
    // Determine the binary path based on the build profile
    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";
    
    #[cfg(windows)]
    {
        let binary = format!("target\\{}\\{}.exe", profile, name);
        // Use cmd /c start to open in a new window with title
        // The spawned cmd.exe exits immediately, but the binary keeps running
        Command::new("cmd")
            .args(["/c", "start", name, &binary])
            .spawn()
            .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        Ok(())
    }
    
    #[cfg(not(windows))]
    {
        let binary = format!("target/{}/{}", profile, name);
        // On Unix, try to use a terminal emulator
        // Fallback: run in background (user can use tmux/screen)
        Command::new(&binary)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))?;
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════╗");
    println!("║     CONTROL PLANE - System Commands    ║");
    println!("╚════════════════════════════════════════╝");

    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open Zenoh session");

    let publisher = session
        .declare_publisher(topics::ADMIN_CONTROL)
        .await
        .expect("Failed to declare ADMIN_CONTROL publisher");

    println!("✓ Zenoh session established");
    println!();
    print_help();

    let mut processes = ManagedProcesses::new();
    let mut lines = BufReader::new(io::stdin()).lines();

    loop {
        print!("> ");
        // Flush stdout (print! doesn't flush automatically)
        use std::io::Write;
        std::io::stdout().flush().ok();

        let Some(line) = lines.next_line().await.ok().flatten() else {
            break;
        };

        match line.trim().to_ascii_lowercase().as_str() {
            // Process management
            "start" | "start-all" => {
                if let Err(e) = processes.start_all() {
                    println!("✗ Failed to start: {}", e);
                }
            }
            "stop" | "stop-all" => {
                processes.stop_all();
            }
            "status" | "ps" => {
                processes.print_status();
            }
            
            // System commands (broadcast to all crates)
            "pause" | "p" => {
                broadcast(&publisher, SystemCommand::Pause, "⏸ PAUSE broadcast").await;
            }
            "resume" | "r" => {
                broadcast(&publisher, SystemCommand::Resume, "▶ RESUME broadcast").await;
            }
            "verbose on" | "v on" => {
                broadcast(&publisher, SystemCommand::Verbose(true), "🔊 VERBOSE ON broadcast").await;
            }
            "verbose off" | "v off" => {
                broadcast(&publisher, SystemCommand::Verbose(false), "🔇 VERBOSE OFF broadcast").await;
            }
            "reset" => {
                println!("🔄 Resetting all crates...");
                processes.stop_all();
                std::thread::sleep(std::time::Duration::from_millis(500));
                if let Err(e) = processes.start_all() {
                    println!("✗ Failed to restart: {}", e);
                }
            }
            "quit" | "exit" | "q" => {
                processes.stop_all();
                println!("Goodbye!");
                break;
            }
            
            // Help
            "help" | "h" | "?" => {
                print_help();
            }
            "" => {}
            other => {
                println!("Unknown command: '{}'. Type 'help' for available commands.", other);
            }
        }
    }
}

async fn broadcast(publisher: &zenoh::pubsub::Publisher<'_>, cmd: SystemCommand, msg: &str) {
    let payload = to_vec(&cmd).expect("Failed to serialize command");
    publisher.put(payload).await.expect("Failed to publish command");
    println!("{}", msg);
}

fn print_help() {
    println!("╭─────────────────────────────────────────╮");
    println!("│  PROCESS MANAGEMENT                     │");
    println!("├─────────────────────────────────────────┤");
    println!("│  start       - Start all crates         │");
    println!("│  stop        - Stop all crates          │");
    println!("│  reset       - Restart all crates       │");
    println!("│  status, ps  - Show process status      │");
    println!("├─────────────────────────────────────────┤");
    println!("│  RUNTIME COMMANDS (broadcast)           │");
    println!("├─────────────────────────────────────────┤");
    println!("│  pause, p      - Pause simulation       │");
    println!("│  resume, r     - Resume simulation      │");
    println!("│  verbose on/off - Toggle verbose mode   │");
    println!("├─────────────────────────────────────────┤");
    println!("│  quit, q     - Exit control plane       │");
    println!("│  help, h     - Show this help           │");
    println!("╰─────────────────────────────────────────╯");
}
