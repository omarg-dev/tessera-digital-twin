//! Overhead robot labels rendered as egui floating areas in the 3D viewport.
//!
//! Each label shows: `#ID  <goal-icon>  <battery%> [▣]`.
//! Color encodes operational state. Globally toggled with `UiState.show_ids`.
//! Right-click a robot in the viewport to hide/show its individual label.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use protocol::RobotState;
use protocol::config::visual::{ROBOT_SIZE, labels as lbl};

use crate::components::Robot;
use crate::resources::UiState;

/// Render overhead floating labels for every non-hidden robot.
pub fn draw_robot_labels(
    mut contexts: EguiContexts,
    ui_state: Res<UiState>,
    robots: Query<(&Robot, &Transform)>,
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

    for (robot, transform) in &robots {
        if robot.label_hidden {
            continue;
        }

        // project a point just above the robot mesh into viewport physical pixels,
        // then convert to egui logical pixels
        let label_world = transform.translation + Vec3::Y * (ROBOT_SIZE * 0.5 + lbl::Y_OFFSET);
        let Ok(phys_pos) = cam.world_to_viewport(cam_gt, label_world) else {
            continue;
        };
        let sp = egui::pos2(phys_pos.x / scale, phys_pos.y / scale);

        let (color, icon, extra) = label_style(robot, now);
        let eg_color = egui::Color32::from_rgb(color.0, color.1, color.2);
        let dim_color = egui::Color32::from_rgba_unmultiplied(180, 180, 180, 200);

        egui::Area::new(egui::Id::new(("rl", robot.id)))
            .fixed_pos(sp)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(lbl::BG_ALPHA))
                    .corner_radius(lbl::CORNER_RADIUS as u8)
                    .inner_margin(egui::Margin::symmetric(lbl::PADDING_H as i8, lbl::PADDING_V as i8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            // robot ID (colored by state priority)
                            ui.label(
                                egui::RichText::new(format!("#{}", robot.id))
                                    .color(eg_color)
                                    .size(lbl::FONT_SIZE)
                                    .strong(),
                            );
                            // goal / status icon (colored by state)
                            ui.label(
                                egui::RichText::new(icon)
                                    .color(eg_color)
                                    .size(lbl::FONT_SIZE),
                            );
                            // secondary info: battery %, cargo flag (muted gray)
                            if !extra.is_empty() {
                                ui.label(
                                    egui::RichText::new(extra)
                                        .color(dim_color)
                                        .size(lbl::FONT_SIZE - 1.0),
                                );
                            }
                        });
                    });
            });
    }

    Ok(())
}

/// Map robot state + recency to `(rgb_color, goal_icon, extra_text)`.
///
/// Priority: offline > faulted > low_battery > blocked > charging > normal states.
fn label_style(robot: &Robot, now: f32) -> ((u8, u8, u8), &'static str, String) {
    let cargo = if robot.carrying_cargo.is_some() { " ▣" } else { "" };

    // offline: no update received within the timeout window
    if now - robot.last_update_secs > lbl::OFFLINE_TIMEOUT_SECS {
        return (lbl::COLOR_OFFLINE, "✕", format!("offline{}", cargo));
    }

    let bat = format!("{:.0}%{}", robot.battery, cargo);

    match robot.state {
        // critical: hard fault / collision
        RobotState::Faulted => (lbl::COLOR_FAULTED, "✖", bat),
        // warning: insufficient charge
        RobotState::LowBattery => (lbl::COLOR_LOW_BATT, "⚡!", bat),
        // rerouting: WHCA* recalculating path
        RobotState::Blocked => (lbl::COLOR_BLOCKED, "↺", bat),
        // charging at home station
        RobotState::Charging => (lbl::COLOR_CHARGING, "⚡", bat),
        // cargo transfer in progress
        RobotState::Picking => (lbl::COLOR_PICKING, "↓PKG", bat),
        // transit states: show destination type
        RobotState::MovingToPickup => (lbl::COLOR_NORMAL, "→PKG", bat),
        RobotState::MovingToDrop => (lbl::COLOR_NORMAL, "→DST", bat),
        RobotState::MovingToStation => (lbl::COLOR_NORMAL, "→⚡", bat),
        // standing by
        RobotState::Idle => (lbl::COLOR_NORMAL, "●", bat),
    }
}
