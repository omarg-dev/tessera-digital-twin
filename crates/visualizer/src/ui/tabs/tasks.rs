//! Tasks tab: queue stats, task list with categories, and Add Task wizard.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::{
    scheduler as sched_cfg,
    visualizer::{self as vis_cfg, TILE_SIZE},
};
use protocol::grid_map::GridMap;
use protocol::{Priority, TaskRequest, TaskStatus, TaskType};
use std::collections::{HashMap, HashSet};

use crate::components::{Dropoff, Shelf};
use crate::resources::{QueueStateData, RightTab, TaskListData, UiAction, UiState};
use crate::ui::widgets::{wizard_minimap_legend, wizard_minimap_widget};

pub const LABEL: &str = "Tasks";

struct TaskBuckets<'a> {
    active: Vec<&'a protocol::Task>,
    failed: Vec<&'a protocol::Task>,
    completed: Vec<&'a protocol::Task>,
}

fn categorize_tasks(tasks: &[protocol::Task]) -> TaskBuckets<'_> {
    let mut active = Vec::new();
    let mut failed = Vec::new();
    let mut completed = Vec::new();

    for task in tasks {
        match task.status {
            TaskStatus::Pending | TaskStatus::Assigned { .. } | TaskStatus::InProgress { .. } => {
                active.push(task)
            }
            TaskStatus::Failed { .. } | TaskStatus::Cancelled => failed.push(task),
            TaskStatus::Completed => completed.push(task),
        }
    }

    TaskBuckets {
        active,
        failed,
        completed,
    }
}

fn task_pages(len: usize, page_size: usize) -> usize {
    if len == 0 {
        1
    } else {
        (len + page_size - 1) / page_size
    }
}

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

    let buckets = categorize_tasks(&task_list.tasks);
    let active_count = task_list.active_total;
    let failed_count = task_list.failed_like_total();
    let completed_count = task_list.completed_total;

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
        render_mass_add_controls(ui, ui_state, actions);
        ui.add_space(6.0);

        // render button BEFORE the scroll area so the scroll area doesn't consume all
        // remaining vertical space and hide the button below the visible region
        let add_btn = egui::Button::new(egui::RichText::new("+ Add New Task").strong())
            .min_size(egui::Vec2::new(ui.available_width(), 28.0));
        if ui.add(add_btn).clicked() {
            ui_state.task_wizard_active = true;
            ui_state.mass_add_form_open = false;
            ui_state.wizard_pickup = None;
            ui_state.wizard_dropoff = None;
            ui_state.wizard_priority = Priority::default();
        }
        ui.add_space(4.0);

        task_list_view(ui, ui_state, &buckets, task_list, actions);
    }
}

/// Render categorised task rows inside a scroll area.
fn task_list_view(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    buckets: &TaskBuckets,
    task_list: &TaskListData,
    _actions: &mut Vec<UiAction>,
) {
    let mut active_page = ui_state.task_page_active;
    let mut failed_page = ui_state.task_page_failed;
    let mut completed_page = ui_state.task_page_completed;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let active_id = egui::Id::new("task_list_active");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), active_id, true)
                .show_header(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("Active ({})", task_list.active_total)).strong(),
                    );
                })
                .body(|ui| {
                    let page_size = vis_cfg::ui::TASK_LIST_PAGE_SIZE.max(1);
                    let page_count = task_pages(buckets.active.len(), page_size);
                    active_page = active_page.min(page_count.saturating_sub(1));

                    if task_list.active_total > buckets.active.len() {
                        ui.weak(format!(
                            "showing {} of {} active tasks",
                            buckets.active.len(),
                            task_list.active_total
                        ));
                    }

                    if buckets.active.is_empty() {
                        ui.weak("no active tasks");
                    } else {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(active_page > 0, egui::Button::new("< Prev"))
                                .clicked()
                            {
                                active_page = active_page.saturating_sub(1);
                            }
                            ui.label(format!("Page {}/{}", active_page + 1, page_count));
                            if ui
                                .add_enabled(
                                    active_page + 1 < page_count,
                                    egui::Button::new("Next >"),
                                )
                                .clicked()
                            {
                                active_page += 1;
                            }
                        });

                        let start = active_page * page_size;
                        let end = (start + page_size).min(buckets.active.len());
                        for task in &buckets.active[start..end] {
                            task_row(ui, task, ui_state);
                        }
                    }
                });

            ui.add_space(4.0);

            let failed_id = egui::Id::new("task_list_failed");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), failed_id, true)
                .show_header(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("Failed ({})", task_list.failed_like_total()))
                            .strong()
                            .color(egui::Color32::from_rgb(220, 80, 80)),
                    );
                })
                .body(|ui| {
                    let page_size = vis_cfg::ui::TASK_LIST_PAGE_SIZE.max(1);
                    let page_count = task_pages(buckets.failed.len(), page_size);
                    failed_page = failed_page.min(page_count.saturating_sub(1));

                    if task_list.failed_like_total() > buckets.failed.len() {
                        ui.weak(format!(
                            "showing {} of {} failed/cancelled tasks",
                            buckets.failed.len(),
                            task_list.failed_like_total()
                        ));
                    }

                    if buckets.failed.is_empty() {
                        ui.weak("no failed tasks");
                    } else {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(failed_page > 0, egui::Button::new("< Prev"))
                                .clicked()
                            {
                                failed_page = failed_page.saturating_sub(1);
                            }
                            ui.label(format!("Page {}/{}", failed_page + 1, page_count));
                            if ui
                                .add_enabled(
                                    failed_page + 1 < page_count,
                                    egui::Button::new("Next >"),
                                )
                                .clicked()
                            {
                                failed_page += 1;
                            }
                        });

                        let start = failed_page * page_size;
                        let end = (start + page_size).min(buckets.failed.len());
                        for task in &buckets.failed[start..end] {
                            let is_selected = ui_state.selected_task_id == Some(task.id);
                            let label = task_row_label(task);
                            if ui
                                .selectable_label(
                                    is_selected,
                                    egui::RichText::new(label)
                                        .color(egui::Color32::from_rgb(220, 80, 80)),
                                )
                                .clicked()
                            {
                                ui_state.selected_task_id = Some(task.id);
                                ui_state.selected_entity = None;
                                ui_state.inspector_tab = RightTab::Details;
                            }
                        }
                    }
                });

            ui.add_space(4.0);

            let completed_id = egui::Id::new("task_list_completed");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), completed_id, false)
                .show_header(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!("Completed ({})", task_list.completed_total))
                            .strong(),
                    );
                })
                .body(|ui| {
                    let page_size = vis_cfg::ui::TASK_LIST_PAGE_SIZE.max(1);
                    let page_count = task_pages(buckets.completed.len(), page_size);
                    completed_page = completed_page.min(page_count.saturating_sub(1));

                    if task_list.completed_total > buckets.completed.len() {
                        ui.weak(format!(
                            "showing {} of {} completed tasks",
                            buckets.completed.len(),
                            task_list.completed_total
                        ));
                    }

                    if buckets.completed.is_empty() {
                        ui.weak("no completed tasks");
                    } else {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(completed_page > 0, egui::Button::new("< Prev"))
                                .clicked()
                            {
                                completed_page = completed_page.saturating_sub(1);
                            }
                            ui.label(format!("Page {}/{}", completed_page + 1, page_count));
                            if ui
                                .add_enabled(
                                    completed_page + 1 < page_count,
                                    egui::Button::new("Next >"),
                                )
                                .clicked()
                            {
                                completed_page += 1;
                            }
                        });

                        let start = completed_page * page_size;
                        let end = (start + page_size).min(buckets.completed.len());
                        for task in &buckets.completed[start..end] {
                            task_row(ui, task, ui_state);
                        }
                    }
                });
        });

    ui_state.task_page_active = active_page;
    ui_state.task_page_failed = failed_page;
    ui_state.task_page_completed = completed_page;
}

/// Single selectable task row. Click -> select task and switch to Inspector tab.
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

fn transform_to_grid(transform: &Transform) -> Option<(usize, usize)> {
    protocol::world_to_grid([
        transform.translation.x / TILE_SIZE,
        0.0,
        transform.translation.z / TILE_SIZE,
    ])
}

fn parse_optional_dropoff_percentage(input: &str) -> Option<Option<f32>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(None);
    }

    let pct = trimmed.parse::<f32>().ok()?;
    if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
        return None;
    }

    Some(Some((pct / 100.0).clamp(0.0, 1.0)))
}

fn parse_mass_add_inputs(count_input: &str, dropoff_pct_input: &str) -> Option<(u32, Option<f32>)> {
    let count = count_input
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|count| *count > 0 && *count <= sched_cfg::MASS_ADD_MAX_COUNT)?;
    let dropoff_probability = parse_optional_dropoff_percentage(dropoff_pct_input)?;
    Some((count, dropoff_probability))
}

fn reset_mass_add_form(ui_state: &mut UiState) {
    ui_state.mass_add_form_open = false;
    ui_state.mass_add_count_input.clear();
    ui_state.mass_add_dropoff_pct_input.clear();
}

fn render_mass_add_controls(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    actions: &mut Vec<UiAction>,
) {
    let toggle_btn = egui::Button::new(egui::RichText::new("Mass-Add Tasks").strong())
        .fill(egui::Color32::from_rgb(38, 76, 122))
        .min_size(egui::Vec2::new(ui.available_width(), 32.0));

    if ui.add(toggle_btn).clicked() {
        ui_state.mass_add_form_open = !ui_state.mass_add_form_open;
        if !ui_state.mass_add_form_open {
            ui_state.mass_add_count_input.clear();
            ui_state.mass_add_dropoff_pct_input.clear();
        }
    }

    if !ui_state.mass_add_form_open {
        return;
    }

    let default_pct = sched_cfg::MASS_ADD_DROPOFF_PROBABILITY * 100.0;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new("Mass-Add Traffic").strong());
        ui.add_space(4.0);

        ui.label("Amount of Tasks");
        ui.add(
            egui::TextEdit::singleline(&mut ui_state.mass_add_count_input)
                .hint_text(format!("1-{}", sched_cfg::MASS_ADD_MAX_COUNT)),
        );

        let count_is_valid = ui_state
            .mass_add_count_input
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|count| *count > 0 && *count <= sched_cfg::MASS_ADD_MAX_COUNT)
            .is_some();

        ui.add_space(4.0);
        ui.label("Drop-off %");

        let mut slider_pct = parse_optional_dropoff_percentage(&ui_state.mass_add_dropoff_pct_input)
            .and_then(|value| value.map(|probability| probability * 100.0))
            .unwrap_or(default_pct)
            .clamp(0.0, 100.0);

        let slider = egui::Slider::new(&mut slider_pct, 0.0..=100.0)
            .step_by(1.0)
            .suffix(" %");
        if ui.add(slider).changed() {
            ui_state.mass_add_dropoff_pct_input = format!("{slider_pct:.0}");
        }
        if ui_state.mass_add_dropoff_pct_input.trim().is_empty() {
            ui.weak(format!("Using default: {default_pct:.0} %"));
        }

        ui.add_space(8.0);
        let mut show_validation_hint = false;
        ui.horizontal(|ui| {
            let execute_btn = egui::Button::new(egui::RichText::new("Execute").strong());
            if ui.add(execute_btn).clicked() {
                if let Some((count, dropoff_probability)) = parse_mass_add_inputs(
                    &ui_state.mass_add_count_input,
                    &ui_state.mass_add_dropoff_pct_input,
                ) {
                    actions.push(UiAction::MassAddTasks {
                        count,
                        dropoff_probability,
                    });
                    reset_mass_add_form(ui_state);
                } else {
                    show_validation_hint = true;
                }
            }

            if ui.button("Cancel").clicked() {
                reset_mass_add_form(ui_state);
            }
        });

        if show_validation_hint || (!ui_state.mass_add_count_input.trim().is_empty() && !count_is_valid)
        {
            ui.colored_label(
                egui::Color32::from_rgb(220, 80, 80),
                format!(
                    "Enter a valid whole number between 1 and {} for Amount of Tasks.",
                    sched_cfg::MASS_ADD_MAX_COUNT
                ),
            );
        }
    });
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
        if ui.button("<- Back").clicked() {
            ui_state.task_wizard_active = false;
        }
        ui.label(egui::RichText::new("Add New Task").strong());
    });

    ui.add_space(4.0);

    // build set of empty shelf grid positions to gray them out in the pickup minimap
    let empty_shelves: HashSet<(usize, usize)> = all_shelves.iter()
        .filter(|(_, sh)| sh.cargo == 0)
        .filter_map(|(e, _)| transforms.get(e).ok())
        .filter_map(transform_to_grid)
        .collect();

    let full_shelves: HashSet<(usize, usize)> = all_shelves
        .iter()
        .filter(|(_, sh)| sh.max_capacity > 0 && sh.cargo >= sh.max_capacity)
        .filter_map(|(e, _)| transforms.get(e).ok())
        .filter_map(transform_to_grid)
        .collect();

    // track shelf fill levels so wizard minimaps can use the same capacity colors
    let shelf_capacity: HashMap<(usize, usize), (u32, u32)> = all_shelves
        .iter()
        .filter_map(|(e, sh)| {
            transforms
                .get(e)
                .ok()
                .and_then(transform_to_grid)
                .map(|g| (g, (sh.cargo, sh.max_capacity)))
        })
        .collect();

    // ── Step 1: Pickup ──
    let pickup_done = ui_state.wizard_pickup.is_some();
    let step1_text = if let Some((x, y)) = ui_state.wizard_pickup {
        format!("Step 1: Pickup ✓ ({x},{y})")
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
                Some(&shelf_capacity),
                "wzrd_pickup",
            ) {
                ui_state.wizard_pickup = Some(clicked);
            }
            ui.add_space(3.0);
            wizard_minimap_legend(ui);
        } else {
            ui.weak("Map not loaded yet.");
        }
    }

    // ── Step 2: Drop-off (only after pickup chosen) ──
    if pickup_done {
        ui.add_space(4.0);
        let dropoff_done = ui_state.wizard_dropoff.is_some();
        let step2_text = if let Some((x, y)) = ui_state.wizard_dropoff {
            format!("Step 2: Drop-off ✓ ({x},{y})")
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
                    Some(&full_shelves),
                    Some(&shelf_capacity),
                    "wzrd_dropoff",
                ) {
                    // don't let them pick the same cell as pickup
                    if Some(clicked) != ui_state.wizard_pickup {
                        ui_state.wizard_dropoff = Some(clicked);
                    }
                }
                ui.add_space(3.0);
                wizard_minimap_legend(ui);
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
    let dropoff_has_capacity = ui_state
        .wizard_dropoff
        .and_then(|dropoff| shelf_capacity.get(&dropoff))
        .map_or(true, |&(cargo, max)| max == 0 || cargo < max);
    let can_submit =
        ui_state.wizard_pickup.is_some() && ui_state.wizard_dropoff.is_some() && dropoff_has_capacity;
    let add_btn = egui::Button::new(egui::RichText::new("Add Task").strong())
        .min_size(egui::Vec2::new(ui.available_width(), 28.0));
    if ui.add_enabled(can_submit, add_btn).clicked() {
        if let (Some(pickup), Some(dropoff)) = (
            ui_state.wizard_pickup.take(),
            ui_state.wizard_dropoff.take(),
        ) {
            actions.push(UiAction::SubmitTransportTask(TaskRequest {
                task_type: TaskType::PickAndDeliver { pickup, dropoff, cargo_id: None },
                priority: ui_state.wizard_priority,
            }));
            ui_state.task_wizard_active = false;
        }
    }

    if !dropoff_has_capacity {
        ui.colored_label(
            egui::Color32::from_rgb(220, 80, 80),
            "Selected destination shelf is full.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_optional_dropoff_percentage() {
        assert_eq!(parse_optional_dropoff_percentage(""), Some(None));
        assert_eq!(parse_optional_dropoff_percentage("60"), Some(Some(0.6)));
        assert_eq!(parse_optional_dropoff_percentage("0"), Some(Some(0.0)));
        assert_eq!(parse_optional_dropoff_percentage("100"), Some(Some(1.0)));
    }

    #[test]
    fn test_parse_optional_dropoff_percentage_invalid() {
        assert_eq!(parse_optional_dropoff_percentage("abc"), None);
        assert_eq!(parse_optional_dropoff_percentage("-1"), None);
        assert_eq!(parse_optional_dropoff_percentage("101"), None);
    }

    #[test]
    fn test_parse_mass_add_inputs() {
        assert_eq!(parse_mass_add_inputs("250", "60"), Some((250, Some(0.6))));
        assert_eq!(parse_mass_add_inputs("10", ""), Some((10, None)));
        assert_eq!(parse_mass_add_inputs("0", "60"), None);
        assert_eq!(parse_mass_add_inputs("10001", "60"), None);
        assert_eq!(parse_mass_add_inputs("abc", "60"), None);
    }

    #[test]
    fn test_task_pages() {
        assert_eq!(task_pages(0, 50), 1);
        assert_eq!(task_pages(1, 50), 1);
        assert_eq!(task_pages(50, 50), 1);
        assert_eq!(task_pages(51, 50), 2);
    }
}
