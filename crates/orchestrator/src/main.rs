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
use protocol::{topics, SystemCommand, RobotControl, logs};
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════════╗");
    println!("║     ORCHESTRATOR - System Controller       ║");
    println!("╚════════════════════════════════════════════╝");

    // Initialize per-orchestrator session directory
    let orch_dir = logs::start_orchestrator_session();
    println!("✓ Log session: {}", orch_dir.display());

    let session = zenoh::open(zenoh::Config::default())
        .await
        .expect("Failed to open Zenoh session");

    let publisher = session
        .declare_publisher(topics::ADMIN_CONTROL)
        .await
        .expect("Failed to declare ADMIN_CONTROL publisher");

    let robot_publisher = session
        .declare_publisher(topics::ROBOT_CONTROL)
        .await
        .expect("Failed to declare ROBOT_CONTROL publisher");

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
            Command::RunAll => {
                logs::start_run_session();
                if let Err(e) = processes.start_all() {
                    println!("✗ Failed to run: {}", e);
                }
            }
            Command::Run(name) => {
                logs::start_run_session();
                if let Err(e) = processes.start(&name) {
                    println!("✗ Failed to run {}: {}", name, e);
                }
            }
            Command::KillAll => {
                processes.kill_all();
                tokio::time::sleep(Duration::from_millis(500)).await;
                println!("Merging logs...");
                logs::merge_logs();
            }
            Command::Kill(name) => {
                if let Err(e) = processes.kill(&name) {
                    println!("✗ {}", e);
                }
            }
            Command::Restart => {
                tokio::time::sleep(Duration::from_millis(500)).await;
                println!("Merging logs...");
                logs::merge_logs();
                logs::start_run_session();
                if let Err(e) = processes.restart_all() {
                    println!("✗ Failed to restart: {}", e);
                }
            }
            Command::Status => {
                cli::print_status(processes.running());
            }

            // Runtime commands (broadcast via Zenoh)
            Command::Pause => {
                logs::save_log("Orchestrator", "System command issued: PAUSE");
                broadcast(&publisher, SystemCommand::Pause, "⏸ PAUSE broadcast").await;
            }
            Command::Resume => {
                logs::save_log("Orchestrator", "System command issued: RESUME");
                broadcast(&publisher, SystemCommand::Resume, "▶ RESUME broadcast").await;
            }
            Command::Verbose(on) => {
                let status = if on { "ON" } else { "OFF" };
                logs::save_log("Orchestrator", &format!("System command issued: VERBOSE {}", status));
                let msg = if on { "🔊 VERBOSE ON" } else { "🔇 VERBOSE OFF" };
                broadcast(&publisher, SystemCommand::Verbose(on), &format!("{} broadcast", msg)).await;
            }
            Command::Chaos(on) => {
                let status = if on { "ON" } else { "OFF" };
                logs::save_log("Orchestrator", &format!("System command issued: CHAOS {}", status));
                let msg = if on { "💥 CHAOS ON" } else { "✨ CHAOS OFF" };
                broadcast(&publisher, SystemCommand::Chaos(on), &format!("{} broadcast", msg)).await;
            }

            // Robot control
            Command::RobotUp(id) => {
                logs::save_log("Orchestrator", &format!("Robot control: UP robot {}", id));
                broadcast_robot(&robot_publisher, RobotControl::Up(id), &format!("🤖 Robot {} UP", id)).await;
            }
            Command::RobotDown(id) => {
                logs::save_log("Orchestrator", &format!("Robot control: DOWN robot {}", id));
                broadcast_robot(&robot_publisher, RobotControl::Down(id), &format!("🔻 Robot {} DOWN", id)).await;
            }
            Command::RobotRestart(id) => {
                logs::save_log("Orchestrator", &format!("Robot control: RESTART robot {}", id));
                broadcast_robot(&robot_publisher, RobotControl::Restart(id), &format!("🔄 Robot {} RESTART", id)).await;
            }

            // Meta
            Command::Help => cli::print_help(),
            Command::Quit => {
                processes.kill_all();
                tokio::time::sleep(Duration::from_millis(500)).await;
                println!("Merging logs...");
                logs::merge_logs();

                // close Zenoh cleanly before tokio runtime shuts down
                drop(robot_publisher);
                drop(publisher);
                session.close().await.ok();

                println!("Goodbye!");
                return;
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

async fn broadcast_robot(publisher: &zenoh::pubsub::Publisher<'_>, cmd: RobotControl, msg: &str) {
    let payload = serde_json::to_vec(&cmd).expect("Failed to serialize robot control");
    publisher.put(payload).await.expect("Failed to publish robot control");
    println!("{}", msg);
}
