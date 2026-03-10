//! Task inspector panel: details, minimap, priority editor, and cancel action.

use bevy_egui::egui;
use protocol::grid_map::GridMap;
use protocol::{Priority, TaskStatus, TaskType};

use crate::resources::{ActivePaths, UiAction, UiState};
use crate::ui::widgets::task_detail_minimap;

/// Details pane displayed when a task is selected in the task list.
pub fn draw(
    ui: &mut egui::Ui,
    task: &protocol::Task,
    ui_state: &mut UiState,
    active_paths: &ActivePaths,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    ui.label(egui::RichText::new(format!("Task #{}", task.id)).heading().strong());
    ui.add_space(8.0);

    // type
    ui.horizontal(|ui| {
        ui.label("Type:");
        let kind = match &task.task_type {
            TaskType::PickAndDeliver { .. } => "Pick & Deliver",
            TaskType::Relocate { .. } => "Relocate",
            TaskType::ReturnToStation { .. } => "Return to Station",
        };
        ui.label(kind);
    });

    // locations + minimap
    let pickup = task.pickup_location();
    let dropoff = task.target_location();
    if let Some((px, py)) = pickup {
        ui.horizontal(|ui| {
            ui.label("Pickup:");
            ui.label(format!("({px},{py})"));
        });
    }
    if let Some((dx, dy)) = dropoff {
        ui.horizontal(|ui| {
            ui.label("Drop-off:");
            ui.label(format!("({dx},{dy})"));
        });
    }
    if pickup.is_some() || dropoff.is_some() {
        if let Some(grid) = warehouse_map {
            ui.add_space(4.0);
            task_detail_minimap(ui, grid, pickup, dropoff);
        }
    }

    ui.add_space(4.0);

    // assignment
    ui.horizontal(|ui| {
        ui.label("Robot:");
        match &task.status {
            TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } => {
                ui.label(format!("#{robot_id}"));
            }
            _ => { ui.weak("Pending"); }
        }
    });

    // status
    ui.horizontal(|ui| {
        ui.label("Status:");
        let s = match &task.status {
            TaskStatus::Pending => "Pending".to_string(),
            TaskStatus::Assigned { .. } => "Assigned".to_string(),
            TaskStatus::InProgress { .. } => "In Progress".to_string(),
            TaskStatus::Completed => "Completed".to_string(),
            TaskStatus::Failed { reason } => format!("Failed: {reason}"),
            TaskStatus::Cancelled => "Cancelled".to_string(),
        };
        ui.label(s);
    });

    // created timestamp
    ui.horizontal(|ui| {
        ui.label("Created:");
        let secs = task.created_at / 1000;
        ui.label(format!("{:02}:{:02}:{:02} UTC", (secs / 3600) % 24, (secs / 60) % 60, secs % 60));
    });

    // ETA (only when a robot is actively working on it)
    if matches!(task.status, TaskStatus::InProgress { .. } | TaskStatus::Assigned { .. }) {
        let eta_str = if let TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } = &task.status {
            if let Some(path) = active_paths.0.get(robot_id) {
                if path.is_empty() {
                    "Arriving".to_string()
                } else {
                    let eta_secs = path.len() as f32 / protocol::config::physics::ROBOT_SPEED;
                    format!("~{:.0}s", eta_secs)
                }
            } else {
                "N/A".to_string()
            }
        } else {
            "N/A".to_string()
        };
        ui.horizontal(|ui| {
            ui.label("ETA:");
            ui.label(eta_str);
        });
    }

    ui.add_space(4.0);

    // priority (editable)
    ui.horizontal(|ui| {
        ui.label("Priority:");
        let mut current = task.priority;
        let old = current;
        egui::ComboBox::from_id_salt(format!("task_prio_{}", task.id))
            .selected_text(format!("{:?}", current))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut current, Priority::Low, "Low");
                ui.selectable_value(&mut current, Priority::Normal, "Normal");
                ui.selectable_value(&mut current, Priority::High, "High");
                ui.selectable_value(&mut current, Priority::Critical, "Critical");
            });
        if current != old {
            actions.push(UiAction::ChangePriority(task.id, current));
        }
    });

    ui.add_space(12.0);
    ui.separator();
    ui.label(egui::RichText::new("Actions").strong());
    ui.add_space(4.0);

    if ui.button(egui::RichText::new("Remove Task").color(egui::Color32::from_rgb(220, 60, 60))).clicked() {
        actions.push(UiAction::CancelTask(task.id));
        ui_state.selected_task_id = None;
    }
}
