use bevy::prelude::*;
use crate::resources::{LogBuffer, RobotUpdates, RobotIndex};
use crate::components::Robot;
use protocol::config::visual::{colors, ROBOT_SIZE};

/// Applies `RobotUpdate`s to matching robots (by `Robot.id`) in the world.
/// If a robot ID is not found, spawns a new robot entity.
/// State changes and spawns are logged to the bottom-panel LogBuffer.
pub fn sync_robots(
    mut commands: Commands,
    mut robot_updates: ResMut<RobotUpdates>,
    mut index: ResMut<RobotIndex>,
    mut robots: Query<(&mut Transform, &mut Robot)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut log_buffer: ResMut<LogBuffer>,
) {
    // Drain all updates collected this frame
    for update in robot_updates.updates.drain(..) {
        // Lookup entity by id
        if let Some(&entity) = index.by_id.get(&update.id) {
            if let Ok((mut transform, mut robot)) = robots.get_mut(entity) {
                let old_state = robot.state.clone();
                robot.id = update.id;
                robot.state = update.state.clone();
                robot.battery = update.battery;
                robot.carrying_cargo = update.carrying_cargo;
                let pos = Vec3::new(update.position[0], update.position[1], update.position[2]);
                robot.position = pos;
                transform.translation = pos;

                // Log state changes to console and UI
                if old_state != update.state {
                    let msg = format!("[Robot #{}] {:?} -> {:?}", update.id, old_state, update.state);
                    println!("{msg}");
                    log_buffer.push(msg);
                }
            }
        } else {
            // Robot not found - spawn a new entity
            let pos = Vec3::new(update.position[0], update.position[1], update.position[2]);

            let entity = commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(ROBOT_SIZE, ROBOT_SIZE, ROBOT_SIZE))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(colors::ROBOT.0, colors::ROBOT.1, colors::ROBOT.2),
                    ..default()
                })),
                Transform::from_translation(pos),
                Robot {
                    id: update.id,
                    state: update.state.clone(),
                    position: pos,
                    battery: update.battery,
                    current_task: None,
                    carrying_cargo: update.carrying_cargo,
                },
            )).id();

            index.by_id.insert(update.id, entity);
            let msg = format!("[Robot #{}] Spawned at [{:.1}, {:.1}, {:.1}]", update.id, pos.x, pos.y, pos.z);
            println!("{msg}");
            log_buffer.push(msg);
        }
    }
}