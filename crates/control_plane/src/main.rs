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
use std::process::{Child, Command, Stdio};
use tokio::io::{self, AsyncBufReadExt, BufReader};

/// Managed child processes for start-all functionality
struct ManagedProcesses {
    fleet_server: Option<Child>,
    swarm_driver: Option<Child>,
    mission_control: Option<Child>,
    visualizer: Option<Child>,
}

impl ManagedProcesses {
    fn new() -> Self {
        Self {
            fleet_server: None,
            swarm_driver: None,
            mission_control: None,
            visualizer: None,
        }
    }

    fn start_all(&mut self) -> Result<(), String> {
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
        self.fleet_server = Some(spawn_binary("fleet_server")?);
        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        // 2. Swarm Driver (validates map hash, starts physics)
        println!("  2/4 Starting swarm_driver...");
        self.swarm_driver = Some(spawn_binary("swarm_driver")?);
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        // 3. Mission Control (task queue)
        println!("  3/4 Starting mission_control...");
        self.mission_control = Some(spawn_binary("mission_control")?);
        std::thread::sleep(std::time::Duration::from_millis(300));
        
        // 4. Visualizer (Bevy window)
        println!("  4/4 Starting visualizer...");
        self.visualizer = Some(spawn_binary("visualizer")?);
        
        println!("✓ All crates started successfully");
        println!("  Use 'status' to check running processes");
        Ok(())
    }

    fn stop_all(&mut self) {
        println!("🛑 Stopping all managed processes...");
        
        // Kill in reverse order
        if let Some(mut p) = self.visualizer.take() {
            let _ = p.kill();
            println!("  ✓ visualizer stopped");
        }
        if let Some(mut p) = self.mission_control.take() {
            let _ = p.kill();
            println!("  ✓ mission_control stopped");
        }
        if let Some(mut p) = self.swarm_driver.take() {
            let _ = p.kill();
            println!("  ✓ swarm_driver stopped");
        }
        if let Some(mut p) = self.fleet_server.take() {
            let _ = p.kill();
            println!("  ✓ fleet_server stopped");
        }
        
        println!("✓ All processes stopped");
    }

    fn print_status(&mut self) {
        println!("╭─────────────────────────────────────────╮");
        println!("│  PROCESS STATUS                         │");
        println!("├─────────────────────────────────────────┤");
        
        fn check(_name: &str, proc: &mut Option<Child>) -> &'static str {
            match proc {
                Some(p) => match p.try_wait() {
                    Ok(Some(_)) => "exited",
                    Ok(None) => "running",
                    Err(_) => "error",
                },
                None => "not started",
            }
        }
        
        println!("│  fleet_server:    {:18} │", check("fleet_server", &mut self.fleet_server));
        println!("│  swarm_driver:    {:18} │", check("swarm_driver", &mut self.swarm_driver));
        println!("│  mission_control: {:18} │", check("mission_control", &mut self.mission_control));
        println!("│  visualizer:      {:18} │", check("visualizer", &mut self.visualizer));
        println!("╰─────────────────────────────────────────╯");
    }
}

impl Drop for ManagedProcesses {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// Spawn a pre-built binary in its own terminal window
fn spawn_binary(name: &str) -> Result<Child, String> {
    // Determine the binary path based on the build profile
    #[cfg(debug_assertions)]
    let profile = "debug";
    #[cfg(not(debug_assertions))]
    let profile = "release";
    
    #[cfg(windows)]
    {
        let binary = format!("target\\{}\\{}.exe", profile, name);
        // Use cmd /c start to open in a new window with title
        Command::new("cmd")
            .args(["/c", "start", name, &binary])
            .spawn()
            .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))
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
            .map_err(|e| format!("Failed to start {} ({}): {}", name, binary, e))
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
    println!("│  pause, p    - Pause simulation         │");
    println!("│  resume, r   - Resume simulation        │");
    println!("├─────────────────────────────────────────┤");
    println!("│  quit, q     - Exit control plane       │");
    println!("│  help, h     - Show this help           │");
    println!("╰─────────────────────────────────────────╯");
}
