//! Gizmo-based path visualization for active robot paths.
//!
//! Draws glowing linestrips from each robot's current position through its
//! remaining waypoints, with a flat circle at the destination.

use bevy::prelude::*;
use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore};
use std::f32::consts::FRAC_PI_2;

use crate::components::Robot;
use crate::resources::{ActivePaths, RobotIndex, UiState};
use protocol::config::visual::path::{DEST_CIRCLE_RADIUS, GLOBAL_PATH_GLOW, LINE_WIDTH, PATH_Y_OFFSET, SELECTED_PATH_GLOW};

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
) {
    let global_show = ui_state.show_paths;

    for (&robot_id, waypoints) in active_paths.0.iter() {
        if waypoints.is_empty() {
            continue;
        }

        // look up the Bevy entity for this robot
        let Some(entity) = robot_index.get_entity(robot_id) else {
            continue;
        };

        // visibility: global toggle OR this robot is selected
        let selected = ui_state.selected_entity == Some(entity);
        if !global_show && !selected {
            continue;
        }

        // selected robot gets the bright prominent color; others get the subtle global color
        let (r, g, b) = if selected { SELECTED_PATH_GLOW } else { GLOBAL_PATH_GLOW };
        let color = Color::linear_rgb(r, g, b);

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

        // draw a flat floor circle at the destination
        if let Some(&dest) = waypoints.last() {
            let iso = Isometry3d::new(dest, Quat::from_rotation_x(-FRAC_PI_2));
            gizmos.circle(iso, DEST_CIRCLE_RADIUS, color);
        }
    }
}
