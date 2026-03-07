//! Gizmo-based path visualization for active robot paths.
//!
//! Draws glowing linestrips from each robot's current position through its
//! remaining waypoints, with a sphere "lollipop" at the destination.

use bevy::prelude::*;

use crate::components::Robot;
use crate::resources::{ActivePaths, RobotIndex, UiState};
use protocol::config::visual::path::{GLOBAL_PATH_GLOW, SELECTED_PATH_GLOW, DEST_SPHERE_RADIUS};



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

    let glow = if global_show {
        GLOBAL_PATH_GLOW
    } else {
        SELECTED_PATH_GLOW
    };

    for (&robot_id, waypoints) in active_paths.0.iter() {
        if waypoints.is_empty() {
            continue;
        }

        // look up the Bevy entity for this robot
        let Some(&entity) = robot_index.by_id.get(&robot_id) else {
            continue;
        };

        // visibility: global toggle OR this robot is selected
        let selected = ui_state.selected_entity == Some(entity);
        if !global_show && !selected {
            continue;
        }

        // get current robot transform to start the line from the live position
        let Ok((_e, _robot, transform)) = robot_query.get(entity) else {
            continue;
        };

        // build the point chain: robot's live position → remaining waypoints
        let start = Vec3::new(transform.translation.x, 0.05, transform.translation.z);
        let points: Vec<Vec3> = std::iter::once(start)
            .chain(waypoints.iter().copied())
            .collect();

        // draw linestrip
        gizmos.linestrip(points, Color::linear_rgb(glow.0, glow.1, glow.2));

        // draw destination sphere at the final waypoint
        if let Some(&dest) = waypoints.last() {
            let iso = Isometry3d::from_translation(dest);
            gizmos.sphere(iso, DEST_SPHERE_RADIUS, Color::linear_rgb(glow.0, glow.1, glow.2));
        }
    }
}
