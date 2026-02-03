//! Handle system commands for the visualizer (reset, pause, etc.)

use bevy::prelude::*;
use protocol::{SystemCommand, topics};
use serde_json::from_slice;
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::ZenohSession;

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

/// Initialize system command listener
pub fn setup_system_listener(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<SystemCommand>(16);
    let session = session.0.clone();

    // Spawn background thread to listen for system commands
    thread::spawn(move || {
        let rt = Runtime::new().expect("Failed to create Tokio runtime for system commands");
        rt.block_on(async move {
            if let Err(e) = run_system_listener(session, tx).await {
                eprintln!("System command listener exited: {}", e);
            }
        });
    });

    commands.insert_resource(SystemCommandReceiver(rx));
}

async fn run_system_listener(session: Session, tx: mpsc::Sender<SystemCommand>) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::ADMIN_CONTROL)
        .await
        .map_err(|e| format!("declare subscriber: {}", e))?;

    println!("✓ System command listener initialized");

    while let Ok(sample) = subscriber.recv_async().await {
        if let Ok(cmd) = from_slice::<SystemCommand>(&sample.payload().to_bytes()) {
            tx.send(cmd).await.ok();
        }
    }

    Err("subscriber closed".into())
}

/// Process system commands and handle reset
pub fn handle_system_commands(
    mut receiver: ResMut<SystemCommandReceiver>,
) {
    while let Ok(cmd) = receiver.0.try_recv() {
        cmd.apply_with_log("Visualizer", None, None, None);
    }
}
