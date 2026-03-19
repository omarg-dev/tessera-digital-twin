use bevy::prelude::*;

use crate::components::{Robot, RobotCargoBox};
use crate::systems::models;
use protocol::config::visual::robot as robot_cfg;

/// Sync robot cargo child visuals from Robot.carrying_cargo transitions.
///
/// Keeps exactly one `box-small` child per robot and toggles visibility by
/// `Robot.carrying_cargo` state to avoid spawn/despawn churn.
pub fn sync_robot_cargo(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    robots: Query<(Entity, &Robot, Option<&Children>), Changed<Robot>>,
    mut cargo_q: Query<&mut Visibility, With<RobotCargoBox>>,
) {
    for (robot_entity, robot, children) in &robots {
        let mut robot_cargo_children = Vec::new();
        if let Some(children) = children {
            for &child in children {
                if cargo_q.get_mut(child).is_ok() {
                    robot_cargo_children.push(child);
                }
            }
        }

        // ensure one persistent cargo child exists for visibility toggling.
        if robot_cargo_children.is_empty() {
            let offset = Vec3::new(
                robot_cfg::CARGO_CHILD_OFFSET.0,
                robot_cfg::CARGO_CHILD_OFFSET.1,
                robot_cfg::CARGO_CHILD_OFFSET.2,
            );
            let visibility = if robot.carrying_cargo.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            let child = commands
                .spawn((
                    SceneRoot(asset_server.load(format!("{}#Scene0", models::assets::BOX_SMALL))),
                    Transform::from_translation(offset)
                        .with_scale(Vec3::splat(robot_cfg::CARGO_CHILD_SCALE)),
                    visibility,
                    RobotCargoBox,
                ))
                .id();
            commands.entity(robot_entity).add_child(child);
            robot_cargo_children.push(child);
        }

        // harden against duplicates by keeping one and removing extras.
        if robot_cargo_children.len() > 1 {
            for &child in robot_cargo_children.iter().skip(1) {
                commands.entity(child).despawn();
            }
            robot_cargo_children.truncate(1);
        }

        // toggle visibility on the persistent cargo child.
        if let Some(&child) = robot_cargo_children.first() {
            if let Ok(mut visibility) = cargo_q.get_mut(child) {
                *visibility = if robot.carrying_cargo.is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}
