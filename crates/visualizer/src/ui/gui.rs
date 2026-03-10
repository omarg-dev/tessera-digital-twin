//! GUI layout for the Digital Twin Command Center.
//!
//! Strictly structural: register egui panel frames, tab bars, and routing.
//! All content implementations live in the `views` module.

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

// ── Control Bar (top) ─────────────────────────────────────────────

/// Thin top panel frame — content rendered by the control_bar view.
pub fn control_bar(
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
                views::control_bar(ui, ui_state, robot_index, queue_state, time, actions);
            });
        });
}

// ── Object Manager (left) ─────────────────────────────────────────

/// Tabbed left panel: Objects and Tasks tabs.
#[allow(clippy::too_many_arguments)]
pub fn object_manager(
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

// ── Inspector (right) ─────────────────────────────────────────────

/// Tabbed right panel: Details and Network tabs.
#[allow(clippy::too_many_arguments)]
pub fn inspector(
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
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.inspector_tab, RightTab::Details, "Details");
                ui.selectable_value(&mut ui_state.inspector_tab, RightTab::Network, "Network");
            });

            ui.separator();

            match ui_state.inspector_tab {
                RightTab::Network => {
                    views::network_view(ui);
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

// ── Log Console (bottom) ──────────────────────────────────────────

/// Tabbed bottom panel: Logs and Analytics tabs.
pub fn log_console(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    log_buffer: &mut LogBuffer,
) {
    egui::TopBottomPanel::bottom("bottom_panel")
        .default_height(ui_cfg::BOTTOM_PANEL_DEFAULT_HEIGHT)
        .height_range(ui_cfg::BOTTOM_PANEL_MIN_HEIGHT..=ui_cfg::BOTTOM_PANEL_MAX_HEIGHT)
        .resizable(true)
        .show(ctx, |ui| {
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
