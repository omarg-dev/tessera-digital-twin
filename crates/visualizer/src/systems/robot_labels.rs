//! Overhead robot labels rendered as egui floating areas in the 3D viewport.
//!
//! Each label shows `#ID` (small, muted) + a large goal/status icon + `▣` when carrying.
//! Color encodes operational state via the icon and a matching border stroke.
//!
//! - Globally toggled with `UiState.show_ids` (top-bar "Labels" checkbox).
//! - Clicking a robot selects it (shows the inspector); its label is suppressed while selected
//!   and reappears automatically when the robot is deselected.
//! - Labels use `Order::Background` so egui panels always render on top.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use protocol::RobotState;
use protocol::config::visual::{ROBOT_SIZE, labels as lbl};

use crate::components::Robot;
use crate::resources::UiState;

/// Render overhead floating labels for every visible robot.
pub fn draw_robot_labels(
    mut contexts: EguiContexts,
    ui_state: Res<UiState>,
    robots: Query<(Entity, &Robot, &Transform)>,
    camera: Query<(&Camera, &GlobalTransform)>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
) -> Result {
    if !ui_state.show_ids {
        return Ok(());
    }

    let Ok((cam, cam_gt)) = camera.single() else {
        return Ok(());
    };
    let Ok(window) = primary_window.single() else {
        return Ok(());
    };

    let ctx = contexts.ctx_mut()?;
    let now = time.elapsed_secs();
    let scale = window.scale_factor();

    let (bg_r, bg_g, bg_b, bg_a) = lbl::BG_COLOR;
    let bg = egui::Color32::from_rgba_unmultiplied(bg_r, bg_g, bg_b, bg_a);
    let id_color = egui::Color32::from_rgba_unmultiplied(160, 160, 160, 200);

    for (entity, robot, transform) in &robots {
        // hide label while the robot is selected — inspector shows all detail
        if ui_state.selected_entity == Some(entity) {
            continue;
        }

        // project a point just above the robot mesh into viewport physical pixels,
        // then convert to egui logical pixels
        let label_world = transform.translation + Vec3::Y * (ROBOT_SIZE * 0.5 + lbl::Y_OFFSET);
        let Ok(phys_pos) = cam.world_to_viewport(cam_gt, label_world) else {
            continue;
        };
        let sp = egui::pos2(phys_pos.x / scale, phys_pos.y / scale);

        let (color, icon, has_cargo) = label_content(robot, now);
        let icon_color = egui::Color32::from_rgb(color.0, color.1, color.2);
        let stroke = egui::Stroke::new(lbl::STROKE_WIDTH, icon_color);

        // Order::Background keeps labels behind egui panels
        egui::Area::new(egui::Id::new(("rl", robot.id)))
            .fixed_pos(sp)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(bg)
                    .stroke(stroke)
                    .corner_radius(lbl::CORNER_RADIUS as u8)
                    .inner_margin(egui::Margin::symmetric(lbl::PADDING_H as i8, lbl::PADDING_V as i8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            // small dimmed robot ID
                            ui.label(
                                egui::RichText::new(format!("#{}", robot.id))
                                    .color(id_color)
                                    .size(lbl::FONT_SIZE),
                            );
                            // large state icon in state color, with cargo flag if carrying
                            let icon_text = if has_cargo {
                                format!("{} ▣", icon)
                            } else {
                                icon.to_owned()
                            };
                            ui.label(
                                egui::RichText::new(icon_text)
                                    .color(icon_color)
                                    .size(lbl::ICON_SIZE)
                                    .strong(),
                            );
                        });
                    });
            });
    }

    Ok(())
}

/// Returns `(rgb_color, goal_icon, has_cargo)` for a robot.
///
/// Priority: offline > faulted > low_battery > blocked > charging > picking > normal.
fn label_content(robot: &Robot, now: f32) -> ((u8, u8, u8), &'static str, bool) {
    let cargo = robot.carrying_cargo.is_some();

    if now - robot.last_update_secs > lbl::OFFLINE_TIMEOUT_SECS {
        return (lbl::COLOR_OFFLINE, "✕", cargo);
    }

    let (color, icon) = match robot.state {
        RobotState::Faulted         => (lbl::COLOR_FAULTED,  "✖"),
        RobotState::LowBattery      => (lbl::COLOR_LOW_BATT, "⚡!"),
        RobotState::Blocked         => (lbl::COLOR_BLOCKED,  "↺"),
        RobotState::Charging        => (lbl::COLOR_CHARGING, "⚡"),
        RobotState::Picking         => (lbl::COLOR_PICKING,  "↓"),
        RobotState::MovingToPickup  => (lbl::COLOR_NORMAL,   "→P"),
        RobotState::MovingToDrop    => (lbl::COLOR_NORMAL,   "→D"),
        RobotState::MovingToStation => (lbl::COLOR_NORMAL,   "→⚡"),
        RobotState::Idle            => (lbl::COLOR_NORMAL,   "●"),
    };

    (color, icon, cargo)
}
