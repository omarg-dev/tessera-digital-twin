//! Orchestrator - System Controller for Hyper-Twin
//!
//! This is the ONLY crate that can issue system commands (pause/resume/reset/kill).
//! All other crates LISTEN for these commands but never originate them.
//!
//! Architecture:
//! - orchestrator: Publishes SystemCommand to ADMIN_CONTROL
//! - All other layers (scheduler, coordinator, physical, renderer): Subscribe and respond
//!
//! ## TODO: UI Improvements
//! - [ ] Integrate control panel into Bevy visualizer (egui sidebar)
//! - [ ] Add real-time status dashboard (robot count, task queue, system state)
//! - [ ] Web-based control panel for production deployments
//! - [ ] Keyboard shortcuts in visualizer (P=pause, R=resume, etc.)

mod cli;
mod processes;

use cli::Command;
use processes::Processes;
use protocol::{topics, SystemCommand};

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════════╗");
    println!("║     ORCHESTRATOR - System Controller       ║");
    println!("╚════════════════════════════════════════════╝");

    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open Zenoh session");

    let publisher = session
        .declare_publisher(topics::ADMIN_CONTROL)
        .await
        .expect("Failed to declare ADMIN_CONTROL publisher");

    println!("✓ Zenoh session established");
    println!();
    cli::print_help();

    let mut processes = Processes::new();

    use tokio::io::{self, AsyncBufReadExt, BufReader};
    let mut lines = BufReader::new(io::stdin()).lines();

    loop {
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush().ok();

        let Some(line) = lines.next_line().await.ok().flatten() else {
            break;
        };

        match Command::parse(&line) {
            // Process management
            Command::StartAll => {
                if let Err(e) = processes.start_all() {
                    println!("✗ Failed to start: {}", e);
                }
            }
            Command::Start(name) => {
                if let Err(e) = processes.start(&name) {
                    println!("✗ Failed to start {}: {}", name, e);
                }
            }
            Command::KillAll => {
                processes.kill_all();
            }
            Command::Kill(name) => {
                if let Err(e) = processes.kill(&name) {
                    println!("✗ {}", e);
                }
            }
            Command::Restart => {
                if let Err(e) = processes.restart_all() {
                    println!("✗ Failed to restart: {}", e);
                }
            }
            Command::Status => {
                cli::print_status(processes.running());
            }

            // Runtime commands (broadcast via Zenoh)
            Command::Pause => {
                broadcast(&publisher, SystemCommand::Pause, "⏸ PAUSE broadcast").await;
            }
            Command::Resume => {
                broadcast(&publisher, SystemCommand::Resume, "▶ RESUME broadcast").await;
            }
            Command::Verbose(on) => {
                let msg = if on { "🔊 VERBOSE ON" } else { "🔇 VERBOSE OFF" };
                broadcast(&publisher, SystemCommand::Verbose(on), &format!("{} broadcast", msg)).await;
            }

            // Meta
            Command::Help => cli::print_help(),
            Command::Quit => {
                processes.kill_all();
                println!("Goodbye!");
                break;
            }
            Command::Empty => {}
            Command::Unknown(cmd) => {
                println!("Unknown command: '{}'. Type 'help' for available commands.", cmd);
            }
        }
    }
}

async fn broadcast(publisher: &zenoh::pubsub::Publisher<'_>, cmd: SystemCommand, msg: &str) {
    let payload = serde_json::to_vec(&cmd).expect("Failed to serialize command");
    publisher.put(payload).await.expect("Failed to publish command");
    println!("{}", msg);
}
