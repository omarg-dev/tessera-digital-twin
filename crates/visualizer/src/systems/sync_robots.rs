use bevy::prelude::*;
use crate::resources::{LogBuffer, RobotUpdates, RobotIndex, WarehouseMap};
use crate::components::{Robot, Shelf};
use protocol::config::visual::{colors, ROBOT_SIZE, CARGO_SHELF_DISTANCE_SQ, TILE_SIZE};
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut log_buffer: ResMut<LogBuffer>,
    mut shelves: Query<(Entity, &mut Shelf, &Transform), Without<Robot>>,
    warehouse_map: Option<Res<WarehouseMap>>,
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

                // Log state changes to console and UI
                if old_state != update.state {
                    let msg = format!("[Robot #{}] {:?} -> {:?}", update.id, old_state, update.state);
                    println!("{msg}");
                    log_buffer.push(msg);
                }
            }
        } else {
            // Robot not found - spawn a new entity with primitive mesh
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
            // TODO: replace primitive mesh with .glb model when available
            // use crate::systems::models;
            // let entity = models::spawn_robot(
            //     &mut commands, &asset_server, pos,
            //     update.id, update.state.clone(), update.battery,
            // );

            index.by_id.insert(update.id, entity);
            let msg = format!("[Robot #{}] Spawned at [{:.1}, {:.1}, {:.1}]", update.id, pos.x, pos.y, pos.z);
            println!("{msg}");
            log_buffer.push(msg);
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