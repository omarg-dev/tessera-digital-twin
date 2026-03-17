//! Control bar content: simulation controls, KPIs, and layer toggles.
//! This is the top bar, not a selectable tab — no LABEL constant.

use bevy_egui::egui;
use protocol::config::visual::bloom as bloom_cfg;

use crate::resources::{CameraPreset, QueueStateData, RobotIndex, UiAction, UiState};

/// Renders the full control bar content inside the top panel frame.
pub fn draw(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    queue_state: &QueueStateData,
    delta_secs: f32,
    actions: &mut Vec<UiAction>,
) {
    // ── Play/pause + speed controls ──
    sim_controls(ui, ui_state, actions);

    ui.separator();

    // Mode toggle: Simulation / Real-time
    let mode_label = if ui_state.is_realtime { "Real-time" } else { "Simulation" };
    let mode_btn = egui::Button::new(mode_label).selected(ui_state.is_realtime);
    if ui.add(mode_btn).clicked() {
        let realtime = !ui_state.is_realtime;
        ui_state.is_realtime = realtime;
        ui_state.custom_speed_editing = false;
        actions.push(UiAction::SetRealtime(realtime));
    }

    ui.separator();

    // ── KPIs ──
    let active = robot_index.by_id.len();
    ui.label(egui::RichText::new(format!("Robots: {active}")).strong());
    ui.separator();

    let fps = 1.0 / delta_secs.max(0.0001);
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
        let mut bloom_changed = false;
        bloom_changed |= ui.checkbox(&mut ui_state.bloom_enabled, "Bloom").changed();

        let slider = egui::Slider::new(
            &mut ui_state.bloom_intensity,
            bloom_cfg::MIN_INTENSITY..=bloom_cfg::MAX_INTENSITY,
        )
        .text("Glow")
        .show_value(false)
        .step_by(0.01);
        bloom_changed |= ui.add_enabled(ui_state.bloom_enabled, slider).changed();

        if bloom_changed {
            actions.push(UiAction::SetBloom {
                enabled: ui_state.bloom_enabled,
                intensity: ui_state.bloom_intensity,
            });
        }

        ui.add_enabled_ui(ui_state.show_ids, |ui| {
            ui.checkbox(&mut ui_state.compact_labels, "Compact");
            ui.checkbox(&mut ui_state.cluster_badges, "Clusters");
        });

        ui.separator();

        let old_preset = ui_state.camera_preset;
        egui::ComboBox::from_id_salt("camera_preset_combo")
            .selected_text(format!("View: {}", ui_state.camera_preset.label()))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut ui_state.camera_preset, CameraPreset::Idle, "Idle");
                ui.selectable_value(&mut ui_state.camera_preset, CameraPreset::Congestion, "Congestion");
                ui.selectable_value(&mut ui_state.camera_preset, CameraPreset::Routing, "Routing");
                ui.selectable_value(&mut ui_state.camera_preset, CameraPreset::Shelf, "Shelf");
            });
        if ui_state.camera_preset != old_preset {
            ui_state.camera_preset_dirty = true;
        }

        if ui.button("Baseline").clicked() {
            ui_state.snapshot_mark_baseline = true;
        }
        if ui.button("After").clicked() {
            ui_state.snapshot_mark_after = true;
        }

        ui.checkbox(&mut ui_state.show_ids, "Labels");
        ui.add_enabled(false, egui::Checkbox::new(&mut ui_state.show_heatmap, "Heatmap"))
            .on_hover_text("heatmap visuals are temporarily disabled pending revamp");
        ui.checkbox(&mut ui_state.show_paths, "Paths");
    });
}

/// Play/pause button and speed multiplier selector.
fn sim_controls(ui: &mut egui::Ui, ui_state: &mut UiState, actions: &mut Vec<UiAction>) {
    let controls_enabled = !ui_state.is_realtime;

    let pause_label = if ui_state.is_paused { "\u{25B6}" } else { "\u{23F8}" }; // ▶ / ⏸
    if ui.add_enabled(controls_enabled, egui::Button::new(pause_label)).clicked() {
        ui_state.is_paused = !ui_state.is_paused;
        actions.push(UiAction::SetPaused(ui_state.is_paused));
    }

    // preset speeds
    let speeds: &[(f32, &str)] = &[
        (1.0, "1x"),
        (2.0, "2x"),
        (5.0, "5x"),
        (10.0, "10x"),
    ];

    for &(factor, label) in speeds {
        let is_selected = !ui_state.custom_speed_editing
            && (ui_state.sim_speed - factor).abs() < 0.001;
        let btn = egui::Button::new(label).selected(is_selected);
        if ui.add_enabled(controls_enabled, btn).clicked() {
            ui_state.sim_speed = factor;
            ui_state.custom_speed_editing = false;
            ui_state.custom_speed_text.clear();
            actions.push(UiAction::SetTimeScale(factor));
        }
    }

    // custom speed button/input
    if ui_state.custom_speed_editing {
        // show text input
        let response = ui.add_enabled(
            controls_enabled,
            egui::TextEdit::singleline(&mut ui_state.custom_speed_text)
                .desired_width(40.0)
                .hint_text("x"),
        );
        // auto-focus on first frame
        if response.gained_focus() || ui_state.custom_speed_text.is_empty() {
            response.request_focus();
        }
        // submit on enter or lose focus
        if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Ok(val) = ui_state.custom_speed_text.trim().parse::<f32>() {
                let clamped = val.clamp(0.1, 1000.0);
                ui_state.sim_speed = clamped;
                ui_state.custom_speed_text = format!("{:.0}", clamped);
                actions.push(UiAction::SetTimeScale(clamped));
            }
            ui_state.custom_speed_editing = false;
        }
        // cancel on escape
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui_state.custom_speed_editing = false;
            ui_state.custom_speed_text.clear();
        }
    } else {
        // determine button label: "Custom" or the custom value
        let is_custom_value = !speeds.iter().any(|(f, _)| (ui_state.sim_speed - f).abs() < 0.001);
        let label = if is_custom_value {
            format!("{:.0}x", ui_state.sim_speed)
        } else {
            "Custom".to_string()
        };
        let btn = egui::Button::new(&label).selected(is_custom_value);
        if ui.add_enabled(controls_enabled, btn).clicked() {
            ui_state.custom_speed_editing = true;
            ui_state.custom_speed_text = if is_custom_value {
                format!("{:.0}", ui_state.sim_speed)
            } else {
                String::new()
            };
        }
    }
}
