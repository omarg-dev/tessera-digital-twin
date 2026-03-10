//! Inbound system command handler for the visualizer (pause/resume/chaos/verbose).
//! Matches the pattern of every other crate's commands.rs: inbound-only.
//! Outbound UI publishing lives in command_bridge.rs.

use bevy::prelude::*;
use protocol::{SystemCommand, topics};
use serde_json::from_slice;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{LogBuffer, UiState, ZenohSession};

/// Receives system commands from Zenoh (pause/resume/reset/kill)
#[derive(Resource)]
pub struct SystemCommandReceiver(pub mpsc::Receiver<SystemCommand>);

/// Resource flag to trigger environment reload after reset.
///
/// TODO: Future use. The visualizer will eventually switch warehouse layouts
/// from the UI, broadcast a reset to other crates, and reload the environment
/// using the selected layout from config.
#[derive(Resource)]
pub struct ReloadEnvironment;

// ── Inbound: System command listener ─────────────────────────────

/// Initialize system command listener
pub fn setup_system_listener(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<SystemCommand>(16);
    let sess = session.session.clone();

    session.runtime.spawn(async move {
        if let Err(e) = run_system_listener(sess, tx).await {
            eprintln!("System command listener exited: {}", e);
        }
    });

    commands.insert_resource(SystemCommandReceiver(rx));
}

async fn run_system_listener(session: Session, tx: mpsc::Sender<SystemCommand>) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::ADMIN_CONTROL)
        .await
        .map_err(|e| format!("declare subscriber: {}", e))?;

    while let Ok(sample) = subscriber.recv_async().await {
        if let Ok(cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
            tx.send(cmd).await.ok();
        }
    }

    Err("subscriber closed".into())
}

/// Process system commands, update UiState, and log to the bottom panel
pub fn handle_system_commands(
    mut receiver: ResMut<SystemCommandReceiver>,
    mut ui_state: ResMut<UiState>,
    mut log_buffer: ResMut<LogBuffer>,
) {
    while let Ok(cmd) = receiver.0.try_recv() {
        // Keep UiState in sync with external commands (e.g. orchestrator pausing)
        match &cmd {
            SystemCommand::Pause => {
                ui_state.is_paused = true;
                log_buffer.push("[System] Paused (external)".into());
            }
            SystemCommand::Resume => {
                ui_state.is_paused = false;
                log_buffer.push("[System] Resumed (external)".into());
            }
            SystemCommand::Verbose(on) => {
                log_buffer.push(format!("[System] Verbose {}", if *on { "ON" } else { "OFF" }));
            }
            SystemCommand::Chaos(on) => {
                log_buffer.push(format!("[System] Chaos {}", if *on { "ON" } else { "OFF" }));
            }
        }
    }
}
