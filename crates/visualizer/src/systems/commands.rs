//! Handle system commands for the visualizer (reset, pause, etc.)
//! Also publishes outbound commands from the UI to Zenoh.

use bevy::prelude::*;
use protocol::{RobotControl, SystemCommand, topics};
use serde_json::from_slice;
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{
    CommandSender, LogBuffer, OutboundCommand, UiAction, UiState, ZenohSession,
};

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

    println!("  System command listener initialized");

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
        cmd.apply_with_log("Visualizer", None, None, None);
    }
}

// ── Outbound: UI → Zenoh publisher ──────────────────────────────

/// Initialize background Zenoh publishers for UI commands
pub fn setup_publishers(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<OutboundCommand>(64);
    let session = session.0.clone();

    thread::spawn(move || {
        let rt = Runtime::new().expect("Failed to create Tokio runtime for UI publishers");
        rt.block_on(async move {
            if let Err(e) = run_publisher_loop(session, rx).await {
                eprintln!("UI publisher loop exited: {}", e);
            }
        });
    });

    commands.insert_resource(CommandSender(tx));
    println!("  UI command publishers initialized");
}

async fn run_publisher_loop(
    session: Session,
    mut rx: mpsc::Receiver<OutboundCommand>,
) -> Result<(), String> {
    let admin_pub = session
        .declare_publisher(topics::ADMIN_CONTROL)
        .await
        .map_err(|e| format!("declare admin publisher: {}", e))?;

    let robot_pub = session
        .declare_publisher(topics::ROBOT_CONTROL)
        .await
        .map_err(|e| format!("declare robot publisher: {}", e))?;

    let task_pub = session
        .declare_publisher(topics::TASK_REQUESTS)
        .await
        .map_err(|e| format!("declare task publisher: {}", e))?;

    while let Some(cmd) = rx.recv().await {
        match cmd {
            OutboundCommand::System(sys) => {
                if let Ok(payload) = serde_json::to_vec(&sys) {
                    admin_pub.put(payload).await.ok();
                }
            }
            OutboundCommand::Robot(ctrl) => {
                if let Ok(payload) = serde_json::to_vec(&ctrl) {
                    robot_pub.put(payload).await.ok();
                }
            }
            OutboundCommand::Task(req) => {
                if let Ok(payload) = serde_json::to_vec(&req) {
                    task_pub.put(payload).await.ok();
                }
            }
        }
    }

    Err("channel closed".into())
}

/// Bridge: reads UiAction events and sends them through Zenoh publishers
pub fn bridge_ui_commands(
    mut events: MessageReader<UiAction>,
    sender: Option<Res<CommandSender>>,
    mut log_buffer: ResMut<LogBuffer>,
) {
    let Some(sender) = sender else { return };

    for action in events.read() {
        let (cmd, msg) = match action {
            UiAction::SetPaused(true) => (
                OutboundCommand::System(SystemCommand::Pause),
                "[UI] Pause broadcast".to_string(),
            ),
            UiAction::SetPaused(false) => (
                OutboundCommand::System(SystemCommand::Resume),
                "[UI] Resume broadcast".to_string(),
            ),
            UiAction::KillRobot(id) => (
                OutboundCommand::Robot(RobotControl::Down(*id)),
                format!("[UI] Kill Robot #{id}"),
            ),
            UiAction::RestartRobot(id) => (
                OutboundCommand::Robot(RobotControl::Restart(*id)),
                format!("[UI] Restart Robot #{id}"),
            ),
            UiAction::EnableRobot(id) => (
                OutboundCommand::Robot(RobotControl::Up(*id)),
                format!("[UI] Enable Robot #{id}"),
            ),
            UiAction::SubmitTransportTask(req) => (
                OutboundCommand::Task(req.clone()),
                format!("[UI] Transport task: {:?}", req.task_type),
            ),
        };

        log_buffer.push(msg);
        sender.0.try_send(cmd).ok();
    }
}
