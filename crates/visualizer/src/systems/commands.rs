//! Handle system commands for the visualizer (reset, pause, etc.)

use bevy::prelude::*;
use protocol::{SystemCommand, topics};
use serde_json::from_slice;
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use crate::components::Robot;
use crate::resources::RobotIndex;

/// Receives system commands from Zenoh (pause/resume/reset/kill)
#[derive(Resource)]
pub struct SystemCommandReceiver(pub mpsc::Receiver<SystemCommand>);

/// Resource flag to trigger environment reload after reset
#[derive(Resource)]
pub struct ReloadEnvironment;

/// Initialize system command listener
pub fn setup_system_listener(mut commands: Commands) {
    let (tx, rx) = mpsc::channel::<SystemCommand>(16);

    // Spawn background thread to listen for system commands
    thread::spawn(move || {
        let rt = Runtime::new().expect("Failed to create Tokio runtime for system commands");
        rt.block_on(async move {
            if let Err(e) = run_system_listener(tx).await {
                eprintln!("System command listener exited: {}", e);
            }
        });
    });

    commands.insert_resource(SystemCommandReceiver(rx));
}

async fn run_system_listener(tx: mpsc::Sender<SystemCommand>) -> Result<(), String> {
    let session = zenoh::open(zenoh::Config::default())
        .await
        .map_err(|e| format!("open session: {}", e))?;

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
    mut commands: Commands,
    mut robot_index: ResMut<RobotIndex>,
    robot_query: Query<Entity, With<Robot>>,
    env_query: Query<Entity, Or<(With<crate::components::Ground>, With<crate::components::Wall>, 
                                   With<crate::components::Shelf>, With<crate::components::Station>, 
                                   With<crate::components::Dropoff>)>>,
) {
    while let Ok(cmd) = receiver.0.try_recv() {
        match cmd {
            SystemCommand::Reset => {
                println!("↻ Reset received - clearing entities and reloading map");
                // Delete all robot entities
                for entity in robot_query.iter() {
                    commands.entity(entity).despawn();
                }
                // Clear robot index (will be repopulated as robots reconnect)
                robot_index.by_id.clear();
                
                // Delete all environment entities
                for entity in env_query.iter() {
                    commands.entity(entity).despawn();
                }
                
                // Trigger environment repopulation
                commands.insert_resource(ReloadEnvironment);
            }
            SystemCommand::Pause => {
                println!("⏸ Pause received (visualization only - physics paused by fleet_server)");
            }
            SystemCommand::Resume => {
                println!("▶ Resume received");
            }
            SystemCommand::Kill => {
                println!("☠ Kill received - shutting down");
                std::process::exit(0);
            }
        }
    }
}
