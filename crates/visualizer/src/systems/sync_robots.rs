use bevy::prelude::*;
use crate::resources::{LogBuffer, RobotUpdates, RobotIndex, WarehouseMap, PlaceholderMeshes};
use crate::components::{Robot, Shelf};
use protocol::config::visual::{CARGO_SHELF_DISTANCE_SQ, TILE_SIZE};
use protocol::grid_map::TileType;

/// Applies `RobotUpdate`s to matching robots (by `Robot.id`) in the world.
/// If a robot ID is not found, spawns a new robot entity.
/// When a robot picks up or drops cargo, updates the nearest shelf's cargo count
/// so that `sync_shelf_boxes` can add/remove visual box entities.
/// State changes and spawns are logged to the bottom-panel LogBuffer.
pub fn sync_robots(
    mut commands: Commands,
    mut robot_updates: ResMut<RobotUpdates>,
    mut index: ResMut<RobotIndex>,
    mut robots: Query<(&mut Transform, &mut Robot), Without<Shelf>>,
    mut log_buffer: ResMut<LogBuffer>,
    mut shelves: Query<(Entity, &mut Shelf, &Transform), Without<Robot>>,
    warehouse_map: Option<Res<WarehouseMap>>,
    placeholder_meshes: Option<Res<PlaceholderMeshes>>,
) {
    // Drain all updates collected this frame
    for update in robot_updates.updates.drain(..) {
        // Lookup entity by id
        if let Some(&entity) = index.by_id.get(&update.id) {
            if let Ok((mut transform, mut robot)) = robots.get_mut(entity) {
                let old_state = robot.state.clone();
                let old_carrying = robot.carrying_cargo;
                robot.id = update.id;
                robot.state = update.state.clone();
                robot.battery = update.battery;
                robot.carrying_cargo = update.carrying_cargo;
                let pos = Vec3::new(update.position[0], update.position[1], update.position[2]);
                robot.position = pos;
                transform.translation = pos;

                // Cargo pickup/drop: update nearest shelf only when on correct tile
                let grid_col = (pos.x / TILE_SIZE).round() as usize;
                let grid_row = (pos.z / TILE_SIZE).round() as usize;
                let tile_type = warehouse_map
                    .as_ref()
                    .and_then(|m| m.0.get_tile(grid_col, grid_row))
                    .map(|t| t.tile_type);

                match (old_carrying, robot.carrying_cargo) {
                    (None, Some(_)) => {
                        // robot picked up cargo - only decrement if at a Shelf tile
                        if matches!(tile_type, Some(TileType::Shelf(_))) {
                            if let Some(shelf_entity) = find_nearest_shelf(&shelves, pos) {
                                if let Ok((_, mut shelf, _)) = shelves.get_mut(shelf_entity) {
                                    shelf.cargo = shelf.cargo.saturating_sub(1);
                                }
                            }
                        }
                    }
                    (Some(_), None) => {
                        // robot dropped cargo - only increment if at a Shelf tile
                        if matches!(tile_type, Some(TileType::Shelf(_))) {
                            if let Some(shelf_entity) = find_nearest_shelf(&shelves, pos) {
                                if let Ok((_, mut shelf, _)) = shelves.get_mut(shelf_entity) {
                                    shelf.cargo += 1;
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if old_state != update.state {
                    log_buffer.push(
                        format!("[Robot #{}] {:?} -> {:?}", update.id, old_state, update.state),
                    );
                }
            }
        } else {
            // robot not found - spawn a new entity
            let pos = Vec3::new(update.position[0], update.position[1], update.position[2]);

            // TODO: replace placeholder cuboid with .glb robot model when available
            let entity = if let Some(ref handles) = placeholder_meshes {
                commands.spawn((
                    Mesh3d(handles.robot_mesh.clone()),
                    MeshMaterial3d(handles.robot_material.clone()),
                    Transform::from_translation(pos),
                    Robot {
                        id: update.id,
                        state: update.state.clone(),
                        position: pos,
                        battery: update.battery,
                        current_task: None,
                        carrying_cargo: update.carrying_cargo,
                    },
                )).id()
            } else {
                // fallback: PlaceholderMeshes not yet inserted (race on first frame)
                commands.spawn((
                    Transform::from_translation(pos),
                    Robot {
                        id: update.id,
                        state: update.state.clone(),
                        position: pos,
                        battery: update.battery,
                        current_task: None,
                        carrying_cargo: update.carrying_cargo,
                    },
                )).id()
            };

            index.by_id.insert(update.id, entity);
            log_buffer.push(
                format!("[Robot #{}] Spawned at [{:.1}, {:.1}, {:.1}]", update.id, pos.x, pos.y, pos.z),
            );
        }
    }
}

/// Find the nearest shelf entity within CARGO_SHELF_DISTANCE_SQ of a position
fn find_nearest_shelf(
    shelves: &Query<(Entity, &mut Shelf, &Transform), Without<Robot>>,
    pos: Vec3,
) -> Option<Entity> {
    let mut nearest_dist = CARGO_SHELF_DISTANCE_SQ;
    let mut nearest: Option<Entity> = None;
    for (entity, _, shelf_transform) in shelves.iter() {
        let dist = pos.distance_squared(shelf_transform.translation);
        if dist < nearest_dist {
            nearest_dist = dist;
            nearest = Some(entity);
        }
    }
    nearest
}