//! Tasks tab: queue stats, task list with categories, and Add Task wizard.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::grid_map::GridMap;
use protocol::{Priority, TaskRequest, TaskStatus, TaskType};
use std::collections::HashSet;

use crate::components::{Dropoff, Shelf};
use crate::resources::{QueueStateData, RightTab, TaskListData, UiAction, UiState};
use crate::ui::widgets::wizard_minimap_widget;

pub const LABEL: &str = "Tasks";

/// Task queue tab -- stats summary + task list or Add Task wizard.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    queue_state: &QueueStateData,
    task_list: &TaskListData,
    all_shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    // ── Stats header ──
    ui.label(egui::RichText::new("Task Queue").strong());
    ui.add_space(4.0);

    let active_count = task_list.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Assigned { .. } | TaskStatus::InProgress { .. }))
        .count();
    let failed_count = task_list.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Failed { .. } | TaskStatus::Cancelled))
        .count();
    let completed_count = task_list.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Completed))
        .count();

    egui::Grid::new("queue_stats")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Active:");
            ui.label(egui::RichText::new(active_count.to_string()).strong());
            ui.end_row();

            ui.label("Completed:");
            ui.label(completed_count.to_string());
            ui.end_row();

            ui.label("Failed:");
            let failed_color = if failed_count > 0 {
                egui::Color32::from_rgb(220, 80, 80)
            } else {
                ui.visuals().text_color()
            };
            ui.label(egui::RichText::new(failed_count.to_string()).color(failed_color));
            ui.end_row();

            ui.label("Robots:");
            ui.label(queue_state.robots_online.to_string());
            ui.end_row();
        });

    ui.separator();

    if ui_state.task_wizard_active {
        wizard_view(ui, ui_state, all_shelves, dropoffs, transforms, warehouse_map, actions);
    } else {
        // render button BEFORE the scroll area so the scroll area doesn't consume all
        // remaining vertical space and hide the button below the visible region
        let add_btn = egui::Button::new(egui::RichText::new("+ Add New Task").strong())
            .min_size(egui::Vec2::new(ui.available_width(), 28.0));
        if ui.add(add_btn).clicked() {
            ui_state.task_wizard_active = true;
            ui_state.wizard_pickup = None;
            ui_state.wizard_dropoff = None;
            ui_state.wizard_priority = Priority::default();
        }
        ui.add_space(4.0);

        task_list_view(ui, ui_state, &task_list.tasks, actions);
    }
}

/// Render categorised task rows inside a scroll area.
fn task_list_view(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    tasks: &[protocol::Task],
    _actions: &mut Vec<UiAction>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Active
            let active: Vec<_> = tasks.iter()
                .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Assigned { .. } | TaskStatus::InProgress { .. }))
                .collect();

            let active_id = egui::Id::new("task_list_active");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), active_id, true)
                .show_header(ui, |ui| {
                    ui.label(egui::RichText::new(format!("Active ({})", active.len())).strong());
                })
                .body(|ui| {
                    for task in &active {
                        task_row(ui, task, ui_state);
                    }
                    if active.is_empty() { ui.weak("no active tasks"); }
                });

            ui.add_space(4.0);

            // Failed
            let failed: Vec<_> = tasks.iter()
                .filter(|t| matches!(t.status, TaskStatus::Failed { .. } | TaskStatus::Cancelled))
                .collect();

            let failed_id = egui::Id::new("task_list_failed");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), failed_id, true)
                .show_header(ui, |ui| {
                    ui.label(egui::RichText::new(format!("Failed ({})", failed.len()))
                        .strong()
                        .color(egui::Color32::from_rgb(220, 80, 80)));
                })
                .body(|ui| {
                    for task in &failed {
                        let is_selected = ui_state.selected_task_id == Some(task.id);
                        let label = task_row_label(task);
                        if ui.selectable_label(
                            is_selected,
                            egui::RichText::new(label).color(egui::Color32::from_rgb(220, 80, 80)),
                        ).clicked() {
                            ui_state.selected_task_id = Some(task.id);
                            ui_state.selected_entity = None;
                            ui_state.inspector_tab = RightTab::Details;
                        }
                    }
                    if failed.is_empty() { ui.weak("no failed tasks"); }
                });

            ui.add_space(4.0);

            // Completed
            let completed: Vec<_> = tasks.iter()
                .filter(|t| matches!(t.status, TaskStatus::Completed))
                .collect();

            let completed_id = egui::Id::new("task_list_completed");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), completed_id, false)
                .show_header(ui, |ui| {
                    ui.label(egui::RichText::new(format!("Completed ({})", completed.len())).strong());
                })
                .body(|ui| {
                    for task in &completed {
                        task_row(ui, task, ui_state);
                    }
                    if completed.is_empty() { ui.weak("no completed tasks"); }
                });
        });
}

/// Single selectable task row. Click -> select task and switch to Details tab.
fn task_row(ui: &mut egui::Ui, task: &protocol::Task, ui_state: &mut UiState) {
    let is_selected = ui_state.selected_task_id == Some(task.id);
    if ui.selectable_label(is_selected, task_row_label(task)).clicked() {
        ui_state.selected_task_id = Some(task.id);
        ui_state.selected_entity = None;
        ui_state.inspector_tab = RightTab::Details;
    }
}

/// Short one-line summary for a task row.
fn task_row_label(task: &protocol::Task) -> String {
    let locs = match &task.task_type {
        TaskType::PickAndDeliver { pickup, dropoff, .. } =>
            format!("({},{})→({},{})", pickup.0, pickup.1, dropoff.0, dropoff.1),
        TaskType::Relocate { from, to } =>
            format!("Move ({},{})→({},{})", from.0, from.1, to.0, to.1),
        TaskType::ReturnToStation { robot_id } =>
            format!("Return R#{robot_id}"),
    };
    let robot = match &task.status {
        TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } =>
            format!(" R#{robot_id}"),
        _ => String::new(),
    };
    format!("#{} {}{} [{:?}]", task.id, locs, robot, task.priority)
}

/// Inline wizard that replaces the task list when `wizard_active`.
#[allow(clippy::too_many_arguments)]
fn wizard_view(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    all_shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        if ui.button("\u{2190} Back").clicked() {
            ui_state.task_wizard_active = false;
        }
        ui.label(egui::RichText::new("Add New Task").strong());
    });

    ui.add_space(4.0);

    // build set of empty shelf grid positions to gray them out in the pickup minimap
    let empty_shelves: HashSet<(usize, usize)> = all_shelves.iter()
        .filter(|(_, sh)| sh.cargo == 0)
        .filter_map(|(e, _)| transforms.get(e).ok())
        .map(|t| (t.translation.x.round() as usize, t.translation.z.round() as usize))
        .collect();

    // ── Step 1: Pickup ──
    let pickup_done = ui_state.wizard_pickup.is_some();
    let step1_text = if let Some((x, y)) = ui_state.wizard_pickup {
        format!("Step 1: Pickup \u{2713} ({x},{y})")
    } else {
        "Step 1: Select Pickup Point".to_string()
    };
    ui.label(egui::RichText::new(step1_text).strong());

    if !pickup_done {
        if let Some(grid) = warehouse_map {
            if let Some(clicked) = wizard_minimap_widget(
                ui, grid,
                ui_state.wizard_pickup,
                ui_state.wizard_dropoff,
                true, false, // shelves clickable, dropoffs not
                Some(&empty_shelves),
                "wzrd_pickup",
            ) {
                ui_state.wizard_pickup = Some(clicked);
            }
        } else {
            ui.weak("Map not loaded yet.");
        }
    }

    // ── Step 2: Drop-off (only after pickup chosen) ──
    if pickup_done {
        ui.add_space(4.0);
        let dropoff_done = ui_state.wizard_dropoff.is_some();
        let step2_text = if let Some((x, y)) = ui_state.wizard_dropoff {
            format!("Step 2: Drop-off \u{2713} ({x},{y})")
        } else {
            "Step 2: Select Drop-off Point".to_string()
        };
        ui.label(egui::RichText::new(step2_text).strong());

        if !dropoff_done {
            let _ = dropoffs; // not yet used in dropoff step
            if let Some(grid) = warehouse_map {
                if let Some(clicked) = wizard_minimap_widget(
                    ui, grid,
                    ui_state.wizard_pickup,
                    ui_state.wizard_dropoff,
                    true, true, // shelves + dropoffs clickable
                    None, // no empty-shelf filter for dropoff destination
                    "wzrd_dropoff",
                ) {
                    // don't let them pick the same cell as pickup
                    if Some(clicked) != ui_state.wizard_pickup {
                        ui_state.wizard_dropoff = Some(clicked);
                    }
                }
            } else {
                ui.weak("Map not loaded yet.");
            }
        }
    }

    ui.add_space(4.0);
    ui.separator();

    // ── Priority selector ──
    ui.horizontal(|ui| {
        ui.label("Priority:");
        egui::ComboBox::from_id_salt("wizard_priority")
            .selected_text(format!("{:?}", ui_state.wizard_priority))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::Low, "Low");
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::Normal, "Normal");
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::High, "High");
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::Critical, "Critical");
            });
    });

    ui.add_space(8.0);

    // ── Submit ──
    let can_submit = ui_state.wizard_pickup.is_some() && ui_state.wizard_dropoff.is_some();
    let add_btn = egui::Button::new(egui::RichText::new("Add Task").strong())
        .min_size(egui::Vec2::new(ui.available_width(), 28.0));
    if ui.add_enabled(can_submit, add_btn).clicked() {
        let pickup = ui_state.wizard_pickup.unwrap();
        let dropoff = ui_state.wizard_dropoff.unwrap();
        actions.push(UiAction::SubmitTransportTask(TaskRequest {
            task_type: TaskType::PickAndDeliver { pickup, dropoff, cargo_id: None },
            priority: ui_state.wizard_priority,
        }));
        ui_state.task_wizard_active = false;
        ui_state.wizard_pickup = None;
        ui_state.wizard_dropoff = None;
    }
}
