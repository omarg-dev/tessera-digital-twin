use bevy::prelude::*;
use crate::resources::{LogBuffer, RobotUpdates, RobotIndex, WarehouseMap};
use crate::components::{Robot, Shelf};
use crate::systems::models;
use protocol::config::visual::{CARGO_SHELF_DISTANCE_SQ, TILE_SIZE};
use protocol::grid_map::TileType;

fn world_to_grid_xy(pos: Vec3) -> Option<(usize, usize)> {
    protocol::world_to_grid([pos.x / TILE_SIZE, 0.0, pos.z / TILE_SIZE])
}

/// Applies `RobotUpdate`s to matching robots (by `Robot.id`) in the world.
/// If a robot ID is not found, spawns a new robot entity.
/// When a robot picks up or drops cargo, updates the nearest shelf's cargo count
/// so that `sync_shelf_boxes` can add/remove visual box entities.
/// State changes and spawns are logged to the bottom-panel LogBuffer.
pub fn sync_robots(
    mut commands: Commands,
    mut robot_updates: ResMut<RobotUpdates>,
    mut index: ResMut<RobotIndex>,
    mut robots: Query<(&Transform, &mut Robot), Without<Shelf>>,
    mut log_buffer: ResMut<LogBuffer>,
    mut shelves: Query<(Entity, &mut Shelf, &Transform), Without<Robot>>,
    warehouse_map: Option<Res<WarehouseMap>>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    // Drain all updates collected this frame
    for update in robot_updates.updates.drain(..) {
        // Lookup entity by id
        if let Some(entity) = index.get_entity(update.id) {
            if let Ok((_, mut robot)) = robots.get_mut(entity) {
                let old_state = robot.state.clone();
                let old_carrying = robot.carrying_cargo;
                let new_state = update.state.clone();
                robot.id = update.id;
                robot.state = new_state;
                robot.battery = update.battery;
                robot.carrying_cargo = update.carrying_cargo;
                robot.enabled = update.enabled;
                let pos = Vec3::new(update.position[0], update.position[1], update.position[2]);
                let vel = Vec3::new(update.velocity[0], update.velocity[1], update.velocity[2]);
                robot.position = pos;
                // update interpolation target and velocity — transform.translation is
                // owned by interpolate_robots (dead-reckons + lerps every render frame)
                robot.target_position = pos;
                robot.network_velocity = vel;
                robot.last_update_secs = time.elapsed_secs();
                // do NOT write transform.translation here; interpolate_robots handles it

                match (old_carrying, robot.carrying_cargo) {
                    (None, Some(_)) => {
                        // robot picked up cargo — mirror the drop logic: find nearest shelf and
                        // verify its own tile type (don't check the robot's tile, which may be
                        // slightly off when the state transition arrives)
                        if let Some(shelf_entity) = find_nearest_shelf(&shelves, pos) {
                            if let Ok((_, mut shelf, shelf_transform)) = shelves.get_mut(shelf_entity) {
                                if let Some((shelf_col, shelf_row)) = world_to_grid_xy(shelf_transform.translation) {
                                    let shelf_tile = warehouse_map
                                        .as_ref()
                                        .and_then(|m| m.0.get_tile(shelf_col, shelf_row))
                                        .map(|t| t.tile_type);
                                    if matches!(shelf_tile, Some(TileType::Shelf(_))) {
                                        shelf.cargo = shelf.cargo.saturating_sub(1);
                                    }
                                }
                            }
                        }
                    }
                    (Some(_), None) => {
                        // robot dropped cargo - use nearest-shelf distance match (avoids imprecise
                        // grid snapping) and verify the shelf's own tile type
                        if let Some(shelf_entity) = find_nearest_shelf(&shelves, pos) {
                            if let Ok((_, mut shelf, shelf_transform)) = shelves.get_mut(shelf_entity) {
                                if let Some((shelf_col, shelf_row)) = world_to_grid_xy(shelf_transform.translation) {
                                    let shelf_tile_type = warehouse_map
                                        .as_ref()
                                        .and_then(|m| m.0.get_tile(shelf_col, shelf_row))
                                        .map(|t| t.tile_type);
                                    if matches!(shelf_tile_type, Some(TileType::Shelf(_))) {
                                        shelf.cargo = (shelf.cargo + 1).min(shelf.max_capacity);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }

                if old_state != robot.state {
                    log_buffer.push(
                        format!("[Robot #{}] {:?} -> {:?}", update.id, old_state, robot.state),
                    );
                }
            }
        } else {
            // robot not found - spawn a new entity
            let pos = Vec3::new(update.position[0], update.position[1], update.position[2]);

            let entity = commands.spawn((
                SceneRoot(asset_server.load(format!("{}#Scene0", models::assets::ROBOT))),
                Transform::from_translation(pos),
                Robot {
                    id: update.id,
                    state: update.state.clone(),
                    position: pos,
                    battery: update.battery,
                    current_task: None,
                    carrying_cargo: update.carrying_cargo,
                    // on spawn, target = current pos so interpolation starts settled
                    target_position: pos,
                    network_velocity: Vec3::ZERO,
                    last_update_secs: time.elapsed_secs(),
                    enabled: update.enabled,
                },
            )).id();

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