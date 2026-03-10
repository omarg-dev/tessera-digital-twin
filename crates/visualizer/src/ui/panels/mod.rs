//! Panel layout for the Digital Twin Command Center.
//!
//! Each public function renders one panel via egui immediate mode.
//! Content is routed to the appropriate view module; shared widgets
//! live in the widgets module.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::visual::ui as ui_cfg;
use protocol::grid_map::GridMap;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{
    ActivePaths, BottomTab, LeftTab, LogBuffer, QueueStateData, RightTab, RobotIndex,
    TaskListData, UiAction, UiState,
};
use super::views;

// ── Top Panel ────────────────────────────────────────────────────

/// HUD bar: simulation controls (left), KPIs (center), layer toggles (right).
pub fn top_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    queue_state: &QueueStateData,
    time: &Time,
    actions: &mut Vec<UiAction>,
) {
    egui::TopBottomPanel::top("top_panel")
        .exact_height(ui_cfg::TOP_PANEL_HEIGHT)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // ── Left: Simulation Controls ──
                sim_controls(ui, ui_state, actions);

                ui.separator();

                // Mode toggle: Simulation / Real-time
                let mode_label = if ui_state.is_realtime { "Real-time" } else { "Simulation" };
                let mode_btn = egui::Button::new(mode_label)
                    .selected(ui_state.is_realtime);
                if ui.add(mode_btn).clicked() {
                    ui_state.is_realtime = !ui_state.is_realtime;
                }

                ui.separator();

                // ── Center: KPIs ──
                let active = robot_index.by_id.len();
                ui.label(egui::RichText::new(format!("Robots: {active}")).strong());
                ui.separator();

                let fps = 1.0 / time.delta_secs().max(0.0001);
                ui.label(format!("FPS: {fps:.0}"));
                ui.separator();

                // Live throughput from scheduler QueueState
                if queue_state.total > 0 {
                    let completed = queue_state.total.saturating_sub(queue_state.pending);
                    ui.label(format!(
                        "Tasks: {completed}/{} done  ({} pending)",
                        queue_state.total, queue_state.pending
                    ));
                } else {
                    ui.label("Tasks: --");
                }

                // Push layer toggles to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut ui_state.show_ids, "Labels");
                    ui.checkbox(&mut ui_state.show_heatmap, "Heatmap");
                    ui.checkbox(&mut ui_state.show_paths, "Paths");
                });
            });
        });
}

/// Speed / play-pause cluster. Publishes Pause/Resume over Zenoh.
fn sim_controls(ui: &mut egui::Ui, ui_state: &mut UiState, actions: &mut Vec<UiAction>) {
    let pause_label = if ui_state.is_paused { "\u{25B6}" } else { "\u{23F8}" }; // ▶ / ⏸
    if ui.button(pause_label).clicked() {
        ui_state.is_paused = !ui_state.is_paused;
        actions.push(UiAction::SetPaused(ui_state.is_paused));
    }

    let speeds: &[(&str, f32)] = &[("1x", 1.0), ("10x", 10.0), ("Max", f32::MAX)];
    for &(label, _factor) in speeds {
        let btn = egui::Button::new(label).selected(label == "1x");
        let response = ui.add_enabled(false, btn);
        response.on_disabled_hover_text("Speed control not yet wired");
    }
}

// ── Left Panel (Object Manager) ──────────────────────────────────

/// Tabbed list of robots, shelves, and tasks with a search bar.
#[allow(clippy::too_many_arguments)]
pub fn left_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
    queue_state: &QueueStateData,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    task_list: &TaskListData,
    actions: &mut Vec<UiAction>,
) {
    egui::SidePanel::left("left_panel")
        .default_width(ui_cfg::SIDE_PANEL_DEFAULT_WIDTH)
        .width_range(ui_cfg::SIDE_PANEL_MIN_WIDTH..=ui_cfg::SIDE_PANEL_MAX_WIDTH)
        .show(ctx, |ui| {
            // ── Tab bar ──
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.object_tab, LeftTab::Objects, "Objects");
                ui.selectable_value(&mut ui_state.object_tab, LeftTab::Tasks, "Tasks");
            });

            ui.separator();

            match ui_state.object_tab {
                LeftTab::Objects => views::objects_tab(ui, ui_state, robot_index, robots, shelves),
                LeftTab::Tasks => views::tasks_tab(
                    ui, ui_state, queue_state, task_list,
                    shelves, dropoffs, transforms, warehouse_map, actions,
                ),
            }
        });
}

// ── Right Panel (Inspector) ──────────────────────────────────────

/// Tabbed inspector for the selected entity or task.
#[allow(clippy::too_many_arguments)]
pub fn right_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    task_list: &TaskListData,
    active_paths: &ActivePaths,
    actions: &mut Vec<UiAction>,
) {
    egui::SidePanel::right("right_panel")
        .default_width(ui_cfg::SIDE_PANEL_DEFAULT_WIDTH)
        .width_range(ui_cfg::SIDE_PANEL_MIN_WIDTH..=ui_cfg::SIDE_PANEL_MAX_WIDTH)
        .show(ctx, |ui| {
            // ── Tab bar ──
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut ui_state.inspector_tab,
                    RightTab::Details,
                    "Details",
                );
                ui.selectable_value(
                    &mut ui_state.inspector_tab,
                    RightTab::Network,
                    "Network",
                );
            });

            ui.separator();

            match ui_state.inspector_tab {
                RightTab::Network => {
                    ui.label("Network view not yet implemented.");
                    ui.label("Future: show robot connectivity, packet loss graph, signal strength, etc.");
                    return;
                }
                _ => {}
            }

            // Entity inspector takes priority over task inspector
            if let Some(entity) = ui_state.selected_entity {
                if let Ok((_, robot)) = robots.get(entity) {
                    views::robot_inspector(ui, robot, actions);
                    return;
                }
                if let Ok((_, shelf)) = shelves.get(entity) {
                    views::shelf_inspector(
                        ui, entity, shelf, ui_state, shelves, dropoffs, transforms,
                        warehouse_map, actions,
                    );
                    return;
                }
                ui.label(format!("Entity {:?}", entity));
                ui.label("No detailed view for this entity type.");
                return;
            }

            // Task inspector
            if let Some(task_id) = ui_state.selected_task_id {
                if let Some(task) = task_list.tasks.iter().find(|t| t.id == task_id) {
                    views::task_inspector(ui, task, ui_state, active_paths, warehouse_map, actions);
                } else {
                    ui.label("Task data unavailable (pending sync).");
                    ui.weak("The task list is broadcast every ~2 seconds.");
                }
                return;
            }

            ui.label("Select an entity or task to view details.");
        });
}

// ── Bottom Panel (Logs / Analytics) ──────────────────────────────

/// Tabbed bottom panel: system logs and analytics placeholder.
pub fn bottom_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    log_buffer: &mut LogBuffer,
) {
    egui::TopBottomPanel::bottom("bottom_panel")
        .default_height(ui_cfg::BOTTOM_PANEL_DEFAULT_HEIGHT)
        .height_range(ui_cfg::BOTTOM_PANEL_MIN_HEIGHT..=ui_cfg::BOTTOM_PANEL_MAX_HEIGHT)
        .resizable(true)
        .show(ctx, |ui| {
            // Tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.bottom_tab, BottomTab::Logs, "Logs");
                ui.selectable_value(&mut ui_state.bottom_tab, BottomTab::Analytics, "Analytics");
            });

            ui.separator();

            match ui_state.bottom_tab {
                BottomTab::Logs => views::logs_tab(ui, log_buffer),
                BottomTab::Analytics => views::analytics_tab(ui),
            }
        });
}
