//! Gizmo-based path visualization for active robot paths.
//!
//! Draws glowing linestrips from each robot's current position through its
//! remaining waypoints, with a flat circle at the destination.

use bevy::prelude::*;
use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore};
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use crate::components::Robot;
use crate::resources::{ActivePaths, RenderPerfCounters, RobotIndex, UiState};
use protocol::config::visual::path::{
    ACTIVE_OTHER_COLOR, COMPLETED_COLOR, COMPLETED_FADE_SECS, DEST_CIRCLE_RADIUS, LINE_WIDTH,
    MAX_FADE_SEGMENTS_PER_FRAME, MAX_SEGMENTS_PER_FRAME,
    OTHER_DEST_RADIUS_MULTIPLIER, PATH_Y_OFFSET, SELECTED_ACTIVE_COLOR, SELECTED_DEST_RADIUS_MULTIPLIER,
    SELECTED_PULSE_AMPLITUDE, SELECTED_PULSE_SPEED,
};

/// One-shot startup system that sets gizmo line width from config.
pub fn configure_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = LINE_WIDTH;
}


/// Draw gizmo paths for robots whose path is in ActivePaths.
///
/// Visibility rules:
/// - `ui_state.show_paths` is the global toggle (draws all paths).
/// - If a specific robot's entity matches `ui_state.selected_entity`, its path
///   is drawn regardless of the global toggle.
pub fn draw_robot_paths(
    mut gizmos: Gizmos,
    active_paths: Res<ActivePaths>,
    ui_state: Res<UiState>,
    robot_index: Res<RobotIndex>,
    robot_query: Query<(Entity, &Robot, &Transform)>,
    time: Res<Time>,
    mut last_paths: Local<HashMap<u32, Vec<Vec3>>>,
    mut completed_paths: Local<HashMap<u32, (Vec<Vec3>, f32)>>,
    mut counters: ResMut<RenderPerfCounters>,
) {
    let global_show = ui_state.show_paths;
    let now = time.elapsed_secs();
    let mut path_segment_budget = MAX_SEGMENTS_PER_FRAME;
    let mut fade_segment_budget = MAX_FADE_SEGMENTS_PER_FRAME;

    let mut removed_ids = Vec::new();
    for &robot_id in last_paths.keys() {
        if !active_paths.0.contains_key(&robot_id) {
            removed_ids.push(robot_id);
        }
    }
    for robot_id in removed_ids {
        if let Some(points) = last_paths.remove(&robot_id) {
            if points.len() > 1 {
                completed_paths.insert(robot_id, (points, now));
            }
        }
    }

    if global_show {
        completed_paths.retain(|_, (points, finished_at)| {
            let age = now - *finished_at;
            if age > COMPLETED_FADE_SECS {
                return false;
            }

            let needed = points.len().saturating_add(1);
            if needed > fade_segment_budget {
                return true;
            }
            fade_segment_budget = fade_segment_budget.saturating_sub(needed);

            let alpha = 1.0 - (age / COMPLETED_FADE_SECS);
            let color = Color::linear_rgba(
                COMPLETED_COLOR.0,
                COMPLETED_COLOR.1,
                COMPLETED_COLOR.2,
                alpha.clamp(0.0, 1.0),
            );

            gizmos.linestrip(points.iter().copied(), color);
            counters.paths_faded_drawn += points.len();
            if let Some(&dest) = points.last() {
                let iso = Isometry3d::new(dest, Quat::from_rotation_x(-FRAC_PI_2));
                gizmos.circle(iso, DEST_CIRCLE_RADIUS * 0.75, color);
                counters.paths_faded_drawn += 1;
            }

            true
        });
    }

    for (&robot_id, waypoints) in active_paths.0.iter() {
        if waypoints.is_empty() {
            continue;
        }

        last_paths.insert(robot_id, waypoints.clone());

        // look up the Bevy entity for this robot
        let Some(entity) = robot_index.get_entity(robot_id) else {
            continue;
        };

        // visibility: global toggle OR this robot is selected
        let selected = ui_state.selected_entity == Some(entity);
        if !global_show && !selected {
            continue;
        }

        let needed = waypoints.len().saturating_add(2);
        if needed > path_segment_budget {
            continue;
        }
        path_segment_budget = path_segment_budget.saturating_sub(needed);

        let mut color = if selected {
            let pulse = (now * SELECTED_PULSE_SPEED).sin().abs() * SELECTED_PULSE_AMPLITUDE;
            Color::linear_rgb(
                SELECTED_ACTIVE_COLOR.0 * (1.0 + pulse),
                SELECTED_ACTIVE_COLOR.1 * (1.0 + pulse),
                SELECTED_ACTIVE_COLOR.2 * (1.0 + pulse),
            )
        } else {
            Color::linear_rgba(
                ACTIVE_OTHER_COLOR.0,
                ACTIVE_OTHER_COLOR.1,
                ACTIVE_OTHER_COLOR.2,
                0.78,
            )
        };

        if !global_show && selected {
            color = Color::linear_rgb(
                SELECTED_ACTIVE_COLOR.0,
                SELECTED_ACTIVE_COLOR.1,
                SELECTED_ACTIVE_COLOR.2,
            );
        }

        // get current robot transform to start the line from the live position
        let Ok((_e, robot, _transform)) = robot_query.get(entity) else {
            continue;
        };

        // build point chain: robot's authoritative network position → remaining waypoints.
        // using robot.position (last reported by firmware) rather than transform.translation
        // (the dead-reckoned visual position) because ActivePaths waypoints are indexed
        // against the reported position; using the interpolated position would draw a
        // backwards stray line to waypoints the robot has already visually passed.
        let start = Vec3::new(robot.position.x, PATH_Y_OFFSET, robot.position.z);

        // draw linestrip without allocating a temporary Vec each frame
        gizmos.linestrip(std::iter::once(start).chain(waypoints.iter().copied()), color);
        counters.path_segments_drawn += waypoints.len().saturating_add(1);

        // draw a flat floor circle at the destination
        if let Some(&dest) = waypoints.last() {
            let iso = Isometry3d::new(dest, Quat::from_rotation_x(-FRAC_PI_2));
            let pulse = if selected {
                (now * SELECTED_PULSE_SPEED).sin().abs() * SELECTED_PULSE_AMPLITUDE
            } else {
                0.0
            };

            let radius = if selected {
                DEST_CIRCLE_RADIUS * SELECTED_DEST_RADIUS_MULTIPLIER * (1.0 + pulse)
            } else {
                DEST_CIRCLE_RADIUS * OTHER_DEST_RADIUS_MULTIPLIER
            };
            gizmos.circle(iso, radius, color);
            counters.path_segments_drawn += 1;
        }
    }
}
