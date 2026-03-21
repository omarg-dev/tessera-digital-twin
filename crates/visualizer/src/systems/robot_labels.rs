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
use std::collections::HashMap;

use crate::components::Robot;
use crate::resources::{RenderPerfCounters, UiState};
use crate::systems::camera::CameraController;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelTier {
    Full,
    Compact,
    Hidden,
}

#[derive(Clone, Copy)]
struct LabelVisual {
    color: (u8, u8, u8),
    status_full: &'static str,
    status_compact: &'static str,
    has_cargo: bool,
    pulse_hz: f32,
    pulse_amplitude: f32,
}

struct LabelCandidate {
    robot_id: u32,
    anchor: egui::Pos2,
    distance: f32,
    tier: LabelTier,
    force_full: bool,
    visual: LabelVisual,
}

fn pulse_rgb(color: (u8, u8, u8), now: f32, hz: f32, amplitude: f32) -> (u8, u8, u8) {
    if hz <= 0.0 || amplitude <= 0.0 {
        return color;
    }

    let pulse = (now * hz * std::f32::consts::TAU).sin().abs() * amplitude;
    let scale = (1.0 + pulse).clamp(0.0, 1.5);

    let channel = |v: u8| -> u8 { (v as f32 * scale).clamp(0.0, 255.0) as u8 };
    (channel(color.0), channel(color.1), channel(color.2))
}

fn resolve_label_tier(distance: f32, compact_enabled: bool) -> LabelTier {
    if distance <= lbl::FULL_TIER_MAX_DISTANCE {
        return LabelTier::Full;
    }
    if distance <= lbl::COMPACT_TIER_MAX_DISTANCE {
        if compact_enabled {
            return LabelTier::Compact;
        }
        return LabelTier::Full;
    }
    LabelTier::Hidden
}

fn draw_label(
    ctx: &egui::Context,
    robot_id: u32,
    anchor: egui::Pos2,
    tier: LabelTier,
    zoom_scale: f32,
    status_rgb: (u8, u8, u8),
    visual: LabelVisual,
    bg: egui::Color32,
    id_color: egui::Color32,
) {
    let status_color = egui::Color32::from_rgb(status_rgb.0, status_rgb.1, status_rgb.2);
    let stroke = egui::Stroke::new(lbl::STROKE_WIDTH * zoom_scale.max(0.65), status_color);

    egui::Area::new(egui::Id::new(("rl", robot_id)))
        .fixed_pos(anchor)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .order(egui::Order::Background)
        .interactable(false)
        .show(ctx, |ui| {
            let mut frame = egui::Frame::new()
                .fill(bg)
                .stroke(stroke)
                .inner_margin(egui::Margin::symmetric(lbl::PADDING_H as i8, lbl::PADDING_V as i8));

            frame = match tier {
                LabelTier::Full => frame.corner_radius(lbl::CORNER_RADIUS as u8),
                LabelTier::Compact => frame.corner_radius(lbl::COMPACT_CORNER_RADIUS as u8),
                LabelTier::Hidden => frame,
            };

            frame.show(ui, |ui| match tier {
                LabelTier::Full => {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(
                            egui::RichText::new(format!("#{}", robot_id))
                                .color(id_color)
                                .size(lbl::FONT_SIZE * zoom_scale),
                        );

                        let label_text = if visual.has_cargo {
                            format!("{} PKG", visual.status_full)
                        } else {
                            visual.status_full.to_owned()
                        };
                        ui.label(
                            egui::RichText::new(label_text)
                                .color(status_color)
                                .size(lbl::ICON_SIZE * zoom_scale)
                                .strong(),
                        );
                    });
                }
                LabelTier::Compact => {
                    let mut compact = format!("#{} {}", robot_id, visual.status_compact);
                    if visual.has_cargo {
                        compact.push_str(" +PKG");
                    }
                    ui.label(
                        egui::RichText::new(compact)
                            .color(status_color)
                            .size(lbl::COMPACT_FONT_SIZE * zoom_scale.max(0.8))
                            .strong(),
                    );
                }
                LabelTier::Hidden => {}
            });
        });
}

fn draw_cluster_badges(
    ctx: &egui::Context,
    anchors: &[egui::Pos2],
    viewport: egui::Rect,
) {
    if anchors.is_empty() {
        return;
    }

    let mut buckets: HashMap<(i32, i32), (usize, f32, f32)> = HashMap::new();
    for anchor in anchors {
        if !viewport.contains(*anchor) {
            continue;
        }
        let bx = (anchor.x / lbl::CLUSTER_BUCKET_PX).floor() as i32;
        let by = (anchor.y / lbl::CLUSTER_BUCKET_PX).floor() as i32;
        let entry = buckets.entry((bx, by)).or_insert((0, 0.0, 0.0));
        entry.0 += 1;
        entry.1 += anchor.x;
        entry.2 += anchor.y;
    }

    for ((bx, by), (count, sx, sy)) in buckets {
        if count < lbl::CLUSTER_MIN_COUNT {
            continue;
        }

        let pos = egui::pos2(sx / count as f32, sy / count as f32);
        egui::Area::new(egui::Id::new(("rl_cluster", bx, by)))
            .fixed_pos(pos)
            .pivot(egui::Align2::CENTER_CENTER)
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(25, 30, 35, 224))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(130, 160, 175)))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("{} robots", count))
                                .size(lbl::COMPACT_FONT_SIZE)
                                .color(egui::Color32::from_rgb(195, 215, 230))
                                .strong(),
                        );
                    });
            });
    }
}

/// Render overhead floating labels for every visible robot.
pub fn draw_robot_labels(
    mut contexts: EguiContexts,
    ui_state: Res<UiState>,
    robots: Query<(Entity, &Robot, &Transform)>,
    camera: Query<(&Camera, &GlobalTransform)>,
    camera_ctrl: Query<&CameraController>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    mut counters: ResMut<RenderPerfCounters>,
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
    let selected_entity = ui_state.selected_entity;
    let hovered_entity = ui_state.hovered_entity;
    let has_hidden = !ui_state.hidden_labels.is_empty();
    let mut candidates = Vec::new();

    for (entity, robot, transform) in &robots {
        let force_full = selected_entity == Some(entity) || hovered_entity == Some(entity);

        // hide label if explicitly hidden by right-click (cleared on deselect)
        if !force_full && has_hidden && ui_state.hidden_labels.contains(&entity) {
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

        let distance = cam_gt.translation().distance(transform.translation);
        let tier = if force_full {
            LabelTier::Full
        } else {
            resolve_label_tier(distance, ui_state.compact_labels)
        };

        candidates.push(LabelCandidate {
            robot_id: robot.id,
            anchor: sp,
            distance,
            tier,
            force_full,
            visual: label_content(robot, now),
        });
    }

    // deterministic render priority: forced full labels first, then nearest robots, then id.
    candidates.sort_by(|a, b| {
        b.force_full
            .cmp(&a.force_full)
            .then_with(|| a.distance.total_cmp(&b.distance))
            .then_with(|| a.robot_id.cmp(&b.robot_id))
    });

    let mut budget_used = 0usize;
    let mut hidden_for_clusters = Vec::new();

    for candidate in candidates {
        if candidate.tier == LabelTier::Hidden {
            hidden_for_clusters.push(candidate.anchor);
            counters.labels_hidden_tier += 1;
            continue;
        }

        if !candidate.force_full && budget_used >= lbl::MAX_LABELS_PER_FRAME {
            hidden_for_clusters.push(candidate.anchor);
            counters.labels_hidden_budget += 1;
            continue;
        }

        let pulse_rgb = pulse_rgb(
            candidate.visual.color,
            now,
            candidate.visual.pulse_hz,
            candidate.visual.pulse_amplitude,
        );

        draw_label(
            ctx,
            candidate.robot_id,
            candidate.anchor,
            candidate.tier,
            zoom_scale,
            pulse_rgb,
            candidate.visual,
            bg,
            id_color,
        );

        if !candidate.force_full {
            budget_used += 1;
        }
        counters.labels_drawn += 1;
    }

    if ui_state.cluster_badges {
        draw_cluster_badges(ctx, &hidden_for_clusters, vp);
    }

    Ok(())
}

/// Returns `(rgb_color, status_label, has_cargo)` for a robot.
///
/// Status labels use plain ASCII so they render reliably in egui's default font.
/// Priority: offline > faulted > low_battery > blocked > charging > picking > normal.
fn label_content(robot: &Robot, now: f32) -> LabelVisual {
    let cargo = robot.carrying_cargo.is_some();

    if now - robot.last_update_secs > lbl::OFFLINE_TIMEOUT_SECS {
        return LabelVisual {
            color: lbl::COLOR_OFFLINE,
            status_full: "OFFLINE",
            status_compact: "OFF",
            has_cargo: cargo,
            pulse_hz: lbl::PULSE_NORMAL_HZ,
            pulse_amplitude: lbl::PULSE_NORMAL_AMPLITUDE,
        };
    }

    let (color, status_full, status_compact, pulse_hz, pulse_amplitude) = match robot.state {
        RobotState::Faulted => (
            lbl::COLOR_FAULTED,
            "FAULT",
            "FLT",
            lbl::PULSE_FAULT_HZ,
            lbl::PULSE_FAULT_AMPLITUDE,
        ),
        RobotState::LowBattery => (
            lbl::COLOR_LOW_BATT,
            "LOW BATT",
            "LOW",
            lbl::PULSE_LOW_BATT_HZ,
            lbl::PULSE_LOW_BATT_AMPLITUDE,
        ),
        RobotState::Blocked => (
            lbl::COLOR_BLOCKED,
            "BLOCKED",
            "BLK",
            lbl::PULSE_BLOCKED_HZ,
            lbl::PULSE_BLOCKED_AMPLITUDE,
        ),
        RobotState::Charging => (
            lbl::COLOR_CHARGING,
            "CHARGING",
            "CHG",
            lbl::PULSE_CHARGING_HZ,
            lbl::PULSE_CHARGING_AMPLITUDE,
        ),
        RobotState::Picking | RobotState::MovingToPickup | RobotState::MovingToDrop | RobotState::MovingToStation => (
            lbl::COLOR_EXECUTING,
            "EXECUTING",
            "RUN",
            lbl::PULSE_NORMAL_HZ,
            lbl::PULSE_NORMAL_AMPLITUDE,
        ),
        RobotState::Idle => (
            lbl::COLOR_NORMAL,
            "IDLE",
            "IDL",
            lbl::PULSE_NORMAL_HZ,
            lbl::PULSE_NORMAL_AMPLITUDE,
        ),
    };

    LabelVisual {
        color,
        status_full,
        status_compact,
        has_cargo: cargo,
        pulse_hz,
        pulse_amplitude,
    }
}
