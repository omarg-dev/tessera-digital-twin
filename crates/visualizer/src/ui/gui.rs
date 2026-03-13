//! GUI layout for the Digital Twin Command Center.
//!
//! Strictly structural: register egui panel frames, tab bars, and routing.
//! All content implementations live in the `tabs` module.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::visual::ui as ui_cfg;
use protocol::grid_map::GridMap;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{
    ActivePaths, BottomTab, LeftTab, LogBuffer, QueueStateData, RightTab, RobotIndex,
    TaskListData, UiAction, UiState, WhcaMetricsData,
};
use super::tabs;

// ── Control Bar (top) ─────────────────────────────────────────────

/// Thin top panel frame -- content rendered by the control_bar tab.
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
                tabs::control_bar::draw(ui, ui_state, robot_index, queue_state, time, actions);
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
    let panel_resp = egui::SidePanel::left("left_panel")
        .default_width(ui_cfg::SIDE_PANEL_DEFAULT_WIDTH)
        .width_range(ui_cfg::SIDE_PANEL_MIN_WIDTH..=ui_cfg::SIDE_PANEL_MAX_WIDTH)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.object_tab, LeftTab::Objects, tabs::objects::LABEL);
                ui.selectable_value(&mut ui_state.object_tab, LeftTab::Tasks, tabs::tasks::LABEL);
            });

            ui.separator();

            match ui_state.object_tab {
                LeftTab::Objects => tabs::objects::draw(ui, ui_state, robot_index, robots, shelves),
                LeftTab::Tasks => tabs::tasks::draw(
                    ui, ui_state, queue_state, task_list,
                    shelves, dropoffs, transforms, warehouse_map, actions,
                ),
            }
        });
    ui_state.left_panel_width = panel_resp.response.rect.width();
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
    let panel_resp = egui::SidePanel::right("right_panel")
        .default_width(ui_cfg::SIDE_PANEL_DEFAULT_WIDTH)
        .width_range(ui_cfg::SIDE_PANEL_MIN_WIDTH..=ui_cfg::SIDE_PANEL_MAX_WIDTH)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.inspector_tab, RightTab::Details, tabs::details::LABEL);
                ui.selectable_value(&mut ui_state.inspector_tab, RightTab::Network, tabs::network::LABEL);
            });

            ui.separator();

            match ui_state.inspector_tab {
                RightTab::Details => tabs::details::draw(
                    ui, ui_state, robots, shelves, dropoffs, transforms,
                    warehouse_map, task_list, active_paths, actions,
                ),
                RightTab::Network => tabs::network::draw(ui),
            }
        });
    ui_state.right_panel_width = panel_resp.response.rect.width();
}

// ── Log Console (bottom) ──────────────────────────────────────────

/// Tabbed bottom panel: Logs and Analytics tabs.
pub fn log_console(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    log_buffer: &mut LogBuffer,
    whca_metrics: &WhcaMetricsData,
) {
    let panel_resp = egui::TopBottomPanel::bottom("bottom_panel")
        .default_height(ui_cfg::BOTTOM_PANEL_DEFAULT_HEIGHT)
        .height_range(ui_cfg::BOTTOM_PANEL_MIN_HEIGHT..=ui_cfg::BOTTOM_PANEL_MAX_HEIGHT)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.bottom_tab, BottomTab::Logs, tabs::logs::LABEL);
                ui.selectable_value(&mut ui_state.bottom_tab, BottomTab::Analytics, tabs::analytics::LABEL);
            });

            ui.separator();

            match ui_state.bottom_tab {
                BottomTab::Logs => tabs::logs::draw(ui, log_buffer),
                BottomTab::Analytics => tabs::analytics::draw(ui, whca_metrics),
            }
        });
    ui_state.bottom_panel_height = panel_resp.response.rect.height();
}

// ── Real-time Mode Overlay ────────────────────────────────────────

/// Shows a centered overlay when real-time mode is active.
pub fn realtime_overlay(ctx: &egui::Context, ui_state: &UiState) {
    if !ui_state.is_realtime {
        return;
    }
    
    egui::Area::new(egui::Id::new("realtime_overlay"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 230))
                .corner_radius(8.0)
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("Real-time Mode").size(20.0).strong());
                        ui.add_space(8.0);
                        ui.label("Physical robot tracking is not yet implemented.");
                        ui.label("Switch back to Simulation mode to continue.");
                    });
                });
        });
}
