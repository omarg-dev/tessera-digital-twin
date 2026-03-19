use bevy::prelude::*;
use std::collections::HashSet;

use crate::components::{Robot, RobotCargoBindingReady, RobotCargoVisual};
use crate::resources::LogBuffer;
use protocol::config::visual::robot as robot_cfg;

#[derive(Resource, Default)]
pub struct RobotCargoBindingState {
    warned_missing: HashSet<Entity>,
}

fn collect_descendants(root: Entity, children_q: &Query<&Children>, out: &mut Vec<Entity>) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        out.push(entity);
        if let Ok(children) = children_q.get(entity) {
            for &child in children {
                stack.push(child);
            }
        }
    }
}

/// Bind embedded cargo child nodes on robot scenes once descendants are loaded.
///
/// Binding is name-driven and case-insensitive using `CARGO_NODE_NAME_TOKEN`.
/// Keep Blender child object names stable and containing the configured token.
pub fn bind_robot_cargo_visuals(
    mut commands: Commands,
    mut state: ResMut<RobotCargoBindingState>,
    mut log_buffer: ResMut<LogBuffer>,
    robots: Query<(Entity, Option<&Children>, &Robot), (With<Robot>, Without<RobotCargoBindingReady>)>,
    children_q: Query<&Children>,
    names_q: Query<&Name>,
) {
    let token = robot_cfg::CARGO_NODE_NAME_TOKEN.to_ascii_lowercase();

    for (robot_entity, children_opt, robot) in &robots {
        if children_opt.is_none() {
            continue;
        }

        let mut descendants = Vec::new();
        collect_descendants(robot_entity, &children_q, &mut descendants);

        let mut matched = 0usize;
        for entity in descendants {
            if entity == robot_entity {
                continue;
            }
            if let Ok(name) = names_q.get(entity) {
                if name.as_str().to_ascii_lowercase().contains(&token) {
                    commands.entity(entity).insert(RobotCargoVisual);
                    matched += 1;
                }
            }
        }

        if matched > 0 {
            commands.entity(robot_entity).insert(RobotCargoBindingReady);
            log_buffer.push(format!(
                "[Robot #{}] Cargo visual binding ready ({} node(s) matched token '{}')",
                robot.id,
                matched,
                robot_cfg::CARGO_NODE_NAME_TOKEN,
            ));
        } else if state.warned_missing.insert(robot_entity) {
            log_buffer.push(format!(
                "[Robot #{}] Cargo visual not found. Ensure a child node name contains '{}' in robot.glb.",
                robot.id,
                robot_cfg::CARGO_NODE_NAME_TOKEN,
            ));
        }
    }
}

/// Sync embedded cargo node visibility from Robot.carrying_cargo state.
///
/// Runs every frame so late-bound cargo nodes immediately adopt the correct state.
pub fn sync_robot_cargo_visibility(
    robots: Query<&Robot>,
    mut cargo_q: Query<(&ChildOf, &mut Visibility), With<RobotCargoVisual>>,
) {
    for (child_of, mut visibility) in &mut cargo_q {
        let parent = child_of.parent();
        if let Ok(robot) = robots.get(parent) {
            *visibility = if robot.carrying_cargo.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}
