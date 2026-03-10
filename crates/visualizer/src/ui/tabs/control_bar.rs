//! Control bar content: simulation controls, KPIs, and layer toggles.
//! This is the top bar, not a selectable tab — no LABEL constant.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::resources::{QueueStateData, RobotIndex, UiAction, UiState};

/// Renders the full control bar content inside the top panel frame.
pub fn draw(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    queue_state: &QueueStateData,
    time: &Time,
    actions: &mut Vec<UiAction>,
) {
    // ── Play/pause + speed controls ──
    sim_controls(ui, ui_state, actions);

    ui.separator();

    // Mode toggle: Simulation / Real-time
    let mode_label = if ui_state.is_realtime { "Real-time" } else { "Simulation" };
    let mode_btn = egui::Button::new(mode_label).selected(ui_state.is_realtime);
    if ui.add(mode_btn).clicked() {
        ui_state.is_realtime = !ui_state.is_realtime;
    }

    ui.separator();

    // ── KPIs ──
    let active = robot_index.by_id.len();
    ui.label(egui::RichText::new(format!("Robots: {active}")).strong());
    ui.separator();

    let fps = 1.0 / time.delta_secs().max(0.0001);
    ui.label(format!("FPS: {fps:.0}"));
    ui.separator();

    if queue_state.total > 0 {
        let completed = queue_state.total.saturating_sub(queue_state.pending);
        ui.label(format!(
            "Tasks: {completed}/{} done  ({} pending)",
            queue_state.total, queue_state.pending
        ));
    } else {
        ui.label("Tasks: --");
    }

    // ── Layer toggles (right-aligned) ──
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.checkbox(&mut ui_state.show_ids, "Labels");
        ui.checkbox(&mut ui_state.show_heatmap, "Heatmap");
        ui.checkbox(&mut ui_state.show_paths, "Paths");
    });
}

/// Play/pause button and speed multiplier selector.
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
