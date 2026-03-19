use bevy::prelude::*;

use crate::components::{Robot, RobotCargoBox};
use crate::systems::models;
use protocol::config::visual::robot as robot_cfg;

/// Sync robot cargo child visuals from Robot.carrying_cargo transitions.
///
/// Spawns one `box-small` child when carrying cargo and despawns it on dropoff.
pub fn sync_robot_cargo(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    robots: Query<(Entity, &Robot, Option<&Children>), Changed<Robot>>,
    cargo_q: Query<(), With<RobotCargoBox>>,
) {
    for (robot_entity, robot, children) in &robots {
        let mut robot_cargo_children = Vec::new();
        if let Some(children) = children {
            for &child in children {
                if cargo_q.contains(child) {
                    robot_cargo_children.push(child);
                }
            }
        }

        if robot.carrying_cargo.is_some() {
            if robot_cargo_children.is_empty() {
                let offset = Vec3::new(
                    robot_cfg::CARGO_CHILD_OFFSET.0,
                    robot_cfg::CARGO_CHILD_OFFSET.1,
                    robot_cfg::CARGO_CHILD_OFFSET.2,
                );
                let child = commands
                    .spawn((
                        SceneRoot(asset_server.load(format!("{}#Scene0", models::assets::BOX_SMALL))),
                        Transform::from_translation(offset)
                            .with_scale(Vec3::splat(robot_cfg::CARGO_CHILD_SCALE)),
                        RobotCargoBox,
                    ))
                    .id();
                commands.entity(robot_entity).add_child(child);
            }
        } else {
            for child in robot_cargo_children {
                commands.entity(child).despawn();
            }
        }
    }
}
