//! Subscribe to the scheduler's bounded TaskListSnapshot broadcasts via Zenoh.
//! Also cross-references task assignments with robot components so
//! Robot.current_task stays up-to-date without relying on RobotUpdate.

use bevy::prelude::*;
use protocol::{TaskListSnapshot, TaskStatus, topics};
use serde_json::from_slice;
use tokio::sync::mpsc;
use zenoh::Session;

use crate::components::Robot;
use crate::resources::{RobotIndex, TaskListData, TaskListReceiver, ZenohSession};

/// Initialize background Zenoh subscriber for task list windows.
pub fn setup_task_listener(mut commands: Commands, session: Res<ZenohSession>) {
    let (tx, rx) = mpsc::channel::<TaskListSnapshot>(16);
    let sess = session.session.clone();

    session.runtime.spawn(async move {
        if let Err(e) = run_task_listener(sess, tx).await {
            eprintln!("Task list listener exited: {}", e);
        }
    });

    commands.insert_resource(TaskListReceiver(rx));
}

async fn run_task_listener(
    session: Session,
    tx: mpsc::Sender<TaskListSnapshot>,
) -> Result<(), String> {
    let subscriber = session
        .declare_subscriber(topics::TASK_LIST)
        .await
        .map_err(|e| format!("declare task list subscriber: {}", e))?;

    while let Ok(sample) = subscriber.recv_async().await {
        if let Ok(snapshot) = from_slice::<TaskListSnapshot>(&sample.payload().to_bytes()) {
            if tx.send(snapshot).await.is_err() {
                return Err("task list receiver dropped".into());
            }
        }
    }

    Err("subscriber closed".into())
}

/// Poll the task list channel and update the shared resource
pub fn collect_task_list(
    receiver: Option<ResMut<TaskListReceiver>>,
    mut data: ResMut<TaskListData>,
    time: Res<Time>,
) {
    let Some(mut receiver) = receiver else { return };

    // drain channel, keep only the latest snapshot
    while let Ok(snapshot) = receiver.0.try_recv() {
        let mut tasks = snapshot.active_tasks;
        tasks.extend(snapshot.recent_terminal_tasks);
        data.tasks = tasks;
        data.active_total = snapshot.active_total;
        data.completed_total = snapshot.completed_total;
        data.failed_total = snapshot.failed_total;
        data.cancelled_total = snapshot.cancelled_total;
        data.last_updated_secs = time.elapsed_secs_f64();
    }
}

/// Sync Robot.current_task by cross-referencing the task list with robot entities.
/// Clears all robot task IDs then re-applies from Assigned/InProgress entries.
pub fn sync_robot_tasks(
    task_list: Res<TaskListData>,
    robot_index: Res<RobotIndex>,
    mut robots: Query<&mut Robot>,
) {
    // clear all
    for mut robot in &mut robots {
        robot.current_task = None;
    }

    // set from active assignments
    for task in &task_list.tasks {
        let robot_id = match &task.status {
            TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } => *robot_id,
            _ => continue,
        };
        if let Some(entity) = robot_index.get_entity(robot_id) {
            if let Ok(mut robot) = robots.get_mut(entity) {
                robot.current_task = Some(task.id);
            }
        }
    }
}
