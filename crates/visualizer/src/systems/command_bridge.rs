//! Outbound Zenoh publishers for UI commands.
//! Bridges UiAction events from the Bevy ECS to Zenoh topics.
//!
//! Outbound-only counterpart to commands.rs (which handles inbound system commands).

use bevy::prelude::*;
use protocol::{RobotControl, SystemCommand, TaskCommand, topics};
use tokio::sync::mpsc;
use zenoh::Session;

use crate::resources::{CommandSender, LogBuffer, OutboundCommand, UiAction, ZenohSession};

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
                    admin_pub.put(payload).await.ok();
                }
            }
            OutboundCommand::Robot(ctrl) => {
                if let Ok(payload) = serde_json::to_vec(&ctrl) {
                    robot_pub.put(payload).await.ok();
                }
            }
            OutboundCommand::Task(cmd) => {
                if let Ok(payload) = serde_json::to_vec(&cmd) {
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
                OutboundCommand::Task(TaskCommand::New {
                    task_type: req.task_type.clone(),
                    priority: req.priority,
                }),
                format!("[UI] Transport task: {:?}", req.task_type),
            ),
            UiAction::CancelTask(id) => (
                OutboundCommand::Task(TaskCommand::Cancel(*id)),
                format!("[UI] Cancel task #{id}"),
            ),
            UiAction::ChangePriority(id, priority) => (
                OutboundCommand::Task(TaskCommand::SetPriority(*id, *priority)),
                format!("[UI] Set task #{id} priority: {:?}", priority),
            ),
        };

        log_buffer.push(msg);
        sender.0.try_send(cmd).ok();
    }
}
