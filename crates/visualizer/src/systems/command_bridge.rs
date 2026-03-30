//! Outbound Zenoh publishers for UI commands.
//! Bridges UiAction events from the Bevy ECS to Zenoh topics.
//!
//! Outbound-only counterpart to commands.rs (which handles inbound system commands).

use bevy::prelude::*;
use protocol::{RobotControl, SystemCommand, TaskCommand, topics};
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{CommandSender, LogBuffer, OutboundCommand, UiAction, UiState, VisualTuning, ZenohSession};

/// Initialize background Zenoh publishers for UI commands
pub fn setup_publishers(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<OutboundCommand>(64);
    let sess = session.session.clone();

    session.runtime.spawn(async move {
        if let Err(e) = run_publisher_loop(sess, rx).await {
            eprintln!("UI publisher loop exited: {}", e);
        }
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
                    if let Err(e) = admin_pub.put(payload).await {
                        eprintln!("failed to publish admin command: {}", e);
                    }
                } else {
                    eprintln!("failed to serialize admin command");
                }
            }
            OutboundCommand::Robot(ctrl) => {
                if let Ok(payload) = serde_json::to_vec(&ctrl) {
                    if let Err(e) = robot_pub.put(payload).await {
                        eprintln!("failed to publish robot command: {}", e);
                    }
                } else {
                    eprintln!("failed to serialize robot command");
                }
            }
            OutboundCommand::Task(cmd) => {
                if let Ok(payload) = serde_json::to_vec(&cmd) {
                    if let Err(e) = task_pub.put(payload).await {
                        eprintln!("failed to publish task command: {}", e);
                    }
                } else {
                    eprintln!("failed to serialize task command");
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
    mut ui_state: ResMut<UiState>,
    mut visual_tuning: ResMut<VisualTuning>,
    mut log_buffer: ResMut<LogBuffer>,
) {
    let Some(sender) = sender else { return };

    for action in events.read() {
        let (cmd, msg) = match action {
            UiAction::SetPaused(true) => (
                Some(OutboundCommand::System(SystemCommand::Pause)),
                "[UI] Pause broadcast".to_string(),
            ),
            UiAction::SetPaused(false) => (
                Some(OutboundCommand::System(SystemCommand::Resume)),
                "[UI] Resume broadcast".to_string(),
            ),
            UiAction::SetRealtime(true) => {
                ui_state.paused_before_realtime = Some(ui_state.is_paused);
                (
                    Some(OutboundCommand::System(SystemCommand::Pause)),
                    "[UI] Real-time ON: pausing simulation".to_string(),
                )
            }
            UiAction::SetRealtime(false) => {
                match ui_state.paused_before_realtime.take() {
                    Some(true) => (
                        Some(OutboundCommand::System(SystemCommand::Pause)),
                        "[UI] Real-time OFF: keeping previous paused state".to_string(),
                    ),
                    Some(false) => (
                        Some(OutboundCommand::System(SystemCommand::Resume)),
                        "[UI] Real-time OFF: restoring running state".to_string(),
                    ),
                    None if ui_state.is_paused => (
                        Some(OutboundCommand::System(SystemCommand::Pause)),
                        "[UI] Real-time OFF: missing previous state, preserving paused".to_string(),
                    ),
                    None => (
                        Some(OutboundCommand::System(SystemCommand::Resume)),
                        "[UI] Real-time OFF: missing previous state, preserving running".to_string(),
                    ),
                }
            }
            UiAction::KillRobot(id) => (
                Some(OutboundCommand::Robot(RobotControl::Down(*id))),
                format!("[UI] Kill Robot #{id}"),
            ),
            UiAction::RestartRobot(id) => (
                Some(OutboundCommand::Robot(RobotControl::Restart(*id))),
                format!("[UI] Restart Robot #{id}"),
            ),
            UiAction::EnableRobot(id) => (
                Some(OutboundCommand::Robot(RobotControl::Up(*id))),
                format!("[UI] Enable Robot #{id}"),
            ),
            UiAction::DisableRobot(id) => (
                Some(OutboundCommand::Robot(RobotControl::Down(*id))),
                format!("[UI] Disable Robot #{id}"),
            ),
            UiAction::SetTimeScale(scale) => (
                Some(OutboundCommand::System(SystemCommand::SetTimeScale(*scale))),
                format!("[UI] Speed: {scale:.1}x"),
            ),
            UiAction::SubmitTransportTask(req) => (
                Some(OutboundCommand::Task(TaskCommand::New {
                    task_type: req.task_type.clone(),
                    priority: req.priority,
                })),
                format!("[UI] Transport task: {:?}", req.task_type),
            ),
            UiAction::MassAddTasks {
                count,
                dropoff_probability,
            } => (
                Some(OutboundCommand::Task(TaskCommand::MassAdd {
                    count: *count,
                    dropoff_probability: *dropoff_probability,
                })),
                format!(
                    "[UI] Mass-add: {} tasks (dropoff {})",
                    count,
                    dropoff_probability
                        .map(|p| format!("{:.1}%", p * 100.0))
                        .unwrap_or_else(|| "default".to_string())
                ),
            ),
            UiAction::CancelTask(id) => (
                Some(OutboundCommand::Task(TaskCommand::Cancel(*id))),
                format!("[UI] Cancel task #{id}"),
            ),
            UiAction::ChangePriority(id, priority) => (
                Some(OutboundCommand::Task(TaskCommand::SetPriority(*id, *priority))),
                format!("[UI] Set task #{id} priority: {:?}", priority),
            ),
            UiAction::SetBloom { enabled, intensity } => {
                visual_tuning.bloom_enabled = *enabled;
                visual_tuning.bloom_intensity = *intensity;
                ui_state.bloom_enabled = *enabled;
                ui_state.bloom_intensity = *intensity;
                (
                    None,
                    format!("[UI] Bloom: {} ({:.2})", if *enabled { "on" } else { "off" }, intensity),
                )
            }
        };

        log_buffer.push(msg);
        if let Some(cmd) = cmd {
            if let Err(e) = sender.0.try_send(cmd) {
                log_buffer.push(format!("[UI] Command dropped before publish: {}", e));
            }
        }
    }
}
