//! Orchestrator - System Controller for Tessera
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
use protocol::{topics, SystemCommand, logs, publish_json_logged};
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

    println!("✓ Zenoh session established");
    println!();
    cli::print_help();

    let mut processes = Processes::new();

    use tokio::io::{self, AsyncBufReadExt, BufReader};
    let mut lines = BufReader::new(io::stdin()).lines();

    loop {
        print!("> ");
        use std::io::Write;
        if let Err(e) = std::io::stdout().flush() {
            println!("Warning: failed to flush stdout: {}", e);
        }

        let Some(line) = lines.next_line().await.ok().flatten() else {
            break;
        };

        match Command::parse(&line) {
            // Process management
            Command::RunAll { layout: layout_selector, mode } => {
                logs::start_run_session();
                let layout_path = match layout_selector {
                    Some(selector) => match protocol::layout::layout_path_from_selector(&selector) {
                        Ok(Some(path)) => Some(path),
                        Ok(None) => {
                            println!(
                                "✗ Unknown layout selector '{}'. Type 'layout' to list available layouts.",
                                selector
                            );
                            continue;
                        }
                        Err(e) => {
                            println!(
                                "✗ Failed to read layouts from '{}': {}",
                                protocol::layout::LAYOUTS_DIR,
                                e
                            );
                            continue;
                        }
                    },
                    None => None,
                };

                if let Err(e) = processes.start_all(layout_path.as_deref(), mode) {
                    println!("✗ Failed to run: {}", e);
                }
            }
            Command::Run { name, layout, mode } => {
                logs::start_run_session();
                let layout_path = match layout {
                    Some(selector) => match protocol::layout::layout_path_from_selector(&selector) {
                        Ok(Some(path)) => Some(path),
                        Ok(None) => {
                            println!(
                                "✗ Unknown layout selector '{}'. Type 'layout' to list available layouts.",
                                selector
                            );
                            continue;
                        }
                        Err(e) => {
                            println!(
                                "✗ Failed to read layouts from '{}': {}",
                                protocol::layout::LAYOUTS_DIR,
                                e
                            );
                            continue;
                        }
                    },
                    None => None,
                };

                if let Err(e) = processes.start(&name, layout_path.as_deref(), mode) {
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
                let status_entries = processes.status_snapshot();
                cli::print_status(&status_entries);
            }
            Command::Layouts => match protocol::layout::discover_layout_entries() {
                Ok(layouts) => cli::print_layouts(&layouts),
                Err(e) => println!(
                    "✗ Failed to read layouts from '{}': {}",
                    protocol::layout::LAYOUTS_DIR,
                    e
                ),
            },

            // output visibility
            Command::ShowOutput(name, true) => processes.show_output(&name),
            Command::ShowOutput(name, false) => processes.hide_output(&name),

            // Runtime commands (broadcast via Zenoh)
            Command::Pause => {
                logs::save_log("Orchestrator", "System command issued: PAUSE");
                if !publish_json_logged("Orchestrator", "PAUSE broadcast", &SystemCommand::Pause, |payload| {
                    async { publisher.put(payload).await.map(|_| ()) }
                }).await {
                    println!("✗ Failed to broadcast PAUSE");
                } else {
                    println!("⏸ PAUSE broadcast");
                }
            }
            Command::Resume => {
                logs::save_log("Orchestrator", "System command issued: RESUME");
                if !publish_json_logged("Orchestrator", "RESUME broadcast", &SystemCommand::Resume, |payload| {
                    async { publisher.put(payload).await.map(|_| ()) }
                }).await {
                    println!("✗ Failed to broadcast RESUME");
                } else {
                    println!("▶ RESUME broadcast");
                }
            }
            Command::Verbose(on) => {
                let status = if on { "ON" } else { "OFF" };
                logs::save_log("Orchestrator", &format!("System command issued: VERBOSE {}", status));
                let msg = if on { "🔊 VERBOSE ON" } else { "🔇 VERBOSE OFF" };
                if !publish_json_logged("Orchestrator", "VERBOSE broadcast", &SystemCommand::Verbose(on), |payload| {
                    async { publisher.put(payload).await.map(|_| ()) }
                }).await {
                    println!("✗ Failed to broadcast VERBOSE {}", status);
                } else {
                    println!("{} broadcast", msg);
                }
            }
            Command::Chaos(on) => {
                let status = if on { "ON" } else { "OFF" };
                logs::save_log("Orchestrator", &format!("System command issued: CHAOS {}", status));
                let msg = if on { "💥 CHAOS ON" } else { "✨ CHAOS OFF" };
                if !publish_json_logged("Orchestrator", "CHAOS broadcast", &SystemCommand::Chaos(on), |payload| {
                    async { publisher.put(payload).await.map(|_| ()) }
                }).await {
                    println!("✗ Failed to broadcast CHAOS {}", status);
                } else {
                    println!("{} broadcast", msg);
                }
            }

            // Meta
            Command::Help => cli::print_help(),
            Command::Quit => {
                processes.kill_all();
                tokio::time::sleep(Duration::from_millis(500)).await;
                println!("Merging logs...");
                logs::merge_logs();

                // close Zenoh cleanly before tokio runtime shuts down
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
