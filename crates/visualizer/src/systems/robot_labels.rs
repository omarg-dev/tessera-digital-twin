//! Overhead robot labels rendered as egui floating areas in the 3D viewport.
//!
//! Each label shows `#ID  STATUS  [PKG]`.
//! egui's default font (Hack) only covers ASCII + basic Latin, so we use
//! short ALL-CAPS status words instead of Unicode symbols.
//!
//! - Globally toggled with `UiState.show_ids` (top-bar "Labels" checkbox).
//! - Label hides while the robot is selected (inspector shows full detail) and
//!   restores automatically on deselect.
//! - Labels are clipped to the 3D viewport rectangle so they never bleed into
//!   the side panels, top bar, or bottom log area.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use protocol::RobotState;
use protocol::config::visual::{ROBOT_SIZE, labels as lbl, ui as ui_cfg, camera as cam_cfg};

use crate::components::Robot;
use crate::resources::UiState;
use crate::systems::camera::CameraController;

/// Render overhead floating labels for every visible robot.
pub fn draw_robot_labels(
    mut contexts: EguiContexts,
    ui_state: Res<UiState>,
    robots: Query<(Entity, &Robot, &Transform)>,
    camera: Query<(&Camera, &GlobalTransform)>,
    camera_ctrl: Query<&CameraController>,
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

    // scale labels with zoom: at default radius labels are 1×; closer = larger, farther = smaller.
    // sqrt smooths the curve. clamp range controls how extreme the size change is:
    //   lower bound: minimum scale at max zoom-out (e.g. 0.3 = 30% of base size)
    //   upper bound: maximum scale at max zoom-in  (e.g. 1.5 = 150% of base size)
    // FONT_SIZE and ICON_SIZE in protocol::config::visual::labels set the base sizes.
    let zoom_scale = if let Ok(ctrl) = camera_ctrl.single() {
        (cam_cfg::DEFAULT_RADIUS / ctrl.radius).sqrt().clamp(0.3, 1.5)
    } else {
        1.0
    };

    // viewport rect in egui logical pixels — labels are clipped to this area
    // so they never bleed into the side panels, top bar, or bottom panel.
    // uses actual runtime panel widths from UiState (updated each frame by gui.rs)
    // so the rect stays correct even when the user resizes panels.
    let screen = ctx.content_rect();
    let vp = egui::Rect::from_min_max(
        egui::pos2(screen.left() + ui_state.left_panel_width, screen.top() + ui_cfg::TOP_PANEL_HEIGHT),
        egui::pos2(screen.right() - ui_state.right_panel_width, screen.bottom() - ui_state.bottom_panel_height),
    );

    let (bg_r, bg_g, bg_b, bg_a) = lbl::BG_COLOR;
    let bg = egui::Color32::from_rgba_unmultiplied(bg_r, bg_g, bg_b, bg_a);
    let id_color = egui::Color32::from_rgba_unmultiplied(160, 160, 160, 200);

    for (entity, robot, transform) in &robots {
        // hide label if explicitly hidden by right-click (cleared on deselect)
        if ui_state.hidden_labels.contains(&entity) {
            continue;
        }

        // project a point just above the robot mesh into viewport physical pixels,
        // then convert to egui logical pixels
        let label_world = transform.translation + Vec3::Y * (ROBOT_SIZE * 0.5 + lbl::Y_OFFSET);
        let Ok(phys_pos) = cam.world_to_viewport(cam_gt, label_world) else {
            continue;
        };
        let sp = egui::pos2(phys_pos.x / scale, phys_pos.y / scale);

        // skip labels whose anchor projects outside the 3D viewport
        if !vp.contains(sp) {
            continue;
        }

        let (color, status, has_cargo) = label_content(robot, now);
        let status_color = egui::Color32::from_rgb(color.0, color.1, color.2);
        let stroke = egui::Stroke::new(lbl::STROKE_WIDTH * zoom_scale.max(0.7), status_color);

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
                            ui.spacing_mut().item_spacing.x = 4.0;
                            // small dim robot ID
                            ui.label(
                                egui::RichText::new(format!("#{}", robot.id))
                                    .color(id_color)
                                    .size(lbl::FONT_SIZE * zoom_scale),
                            );
                            // large bold status word in state color
                            let label_text = if has_cargo {
                                format!("{} PKG", status)
                            } else {
                                status.to_owned()
                            };
                            ui.label(
                                egui::RichText::new(label_text)
                                    .color(status_color)
                                    .size(lbl::ICON_SIZE * zoom_scale)
                                    .strong(),
                            );
                        });
                    });
            });
    }

    Ok(())
}

/// Returns `(rgb_color, status_label, has_cargo)` for a robot.
///
/// Status labels use plain ASCII so they render reliably in egui's default font.
/// Priority: offline > faulted > low_battery > blocked > charging > picking > normal.
fn label_content(robot: &Robot, now: f32) -> ((u8, u8, u8), &'static str, bool) {
    let cargo = robot.carrying_cargo.is_some();

    if now - robot.last_update_secs > lbl::OFFLINE_TIMEOUT_SECS {
        return (lbl::COLOR_OFFLINE, "OFFLINE", cargo);
    }

    let (color, status) = match robot.state {
        RobotState::Faulted         => (lbl::COLOR_FAULTED,  "FAULT"),
        RobotState::LowBattery      => (lbl::COLOR_LOW_BATT, "LOW BATT"),
        RobotState::Blocked         => (lbl::COLOR_BLOCKED,  "REROUTING"),
        RobotState::Charging        => (lbl::COLOR_CHARGING, "CHARGING"),
        RobotState::Picking         => (lbl::COLOR_PICKING,  "PICKING"),
        RobotState::MovingToPickup  => (lbl::COLOR_NORMAL,   "-> PICKUP"),
        RobotState::MovingToDrop    => (lbl::COLOR_NORMAL,   "-> DROP"),
        RobotState::MovingToStation => (lbl::COLOR_NORMAL,   "-> CHARGER"),
        RobotState::Idle            => (lbl::COLOR_NORMAL,   "IDLE"),
    };

    (color, status, cargo)
}
