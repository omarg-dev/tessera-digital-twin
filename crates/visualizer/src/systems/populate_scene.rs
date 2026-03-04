use bevy::prelude::*;
use crate::components::*;
use crate::systems::models;
use protocol::config::{LAYOUT_FILE_PATH, visual::TILE_SIZE, visual::SHELF_MAX_CAPACITY, visual::BOX_SCALE, visual::lighting};
use protocol::grid_map::{GridMap, TileType};

/// Check if environment reload is requested and trigger repopulation
pub fn check_reload_environment(
    mut commands: Commands,
    reload_trigger: Option<ResMut<crate::systems::commands::ReloadEnvironment>>,
    asset_server: Res<AssetServer>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    env_entities: Query<Entity, Or<(With<Ground>, With<Wall>, With<Shelf>, With<Station>, With<Dropoff>)>>,
) {
    if reload_trigger.is_some() {
        println!("↻ Reloading warehouse environment");
        // despawn existing environment before repopulating
        for entity in &env_entities {
            commands.entity(entity).despawn();
        }
        populate_environment(commands.reborrow(), asset_server, meshes, materials);
        commands.remove_resource::<crate::systems::commands::ReloadEnvironment>();
    }
}

/// Read from a .txt map file and populate the warehouse layout with .glb models.
/// Uses protocol::GridMap as the sole source of truth for tile parsing.
/// A bool wall grid is derived from the GridMap for wall neighbor analysis.
pub fn populate_environment(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // load map via protocol::GridMap (same parser as coordinator + scheduler)
    let map = match GridMap::load_from_file(LAYOUT_FILE_PATH) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to load layout file via GridMap: {}", e);
            return;
        }
    };

    // build wall grid from GridMap tiles (true = wall)
    let mut wall_grid = vec![vec![false; map.width]; map.height];
    for tile in &map.tiles {
        if tile.tile_type == TileType::Wall {
            wall_grid[tile.y][tile.x] = true;
        }
    }

    // spawn environment entities from the parsed GridMap tiles
    for tile in &map.tiles {
        let pos = Vec3::new(tile.x as f32 * TILE_SIZE, 0.0, tile.y as f32 * TILE_SIZE);

        match tile.tile_type {
            TileType::Ground => {
                models::spawn_floor(&mut commands, &asset_server, pos);
            }
            TileType::Wall => {
                models::spawn_wall(&mut commands, &asset_server, pos, &wall_grid, tile.y, tile.x);
            }
            TileType::Station => {
                models::spawn_floor(&mut commands, &asset_server, pos);
                models::spawn_station(&mut commands, &mut meshes, &mut materials, pos);
            }
            TileType::Dropoff => {
                models::spawn_floor(&mut commands, &asset_server, pos);
                models::spawn_dropoff(&mut commands, &mut meshes, &mut materials, pos);
            }
            TileType::Shelf(capacity) => {
                models::spawn_floor(&mut commands, &asset_server, pos);
                models::spawn_shelf(&mut commands, &asset_server, pos, capacity as u32);
            }
            TileType::Empty => {
                // N/A tile (~), skip
            }
        }
    }

    info!("Warehouse layout loaded: {}x{} (GridMap)", map.width, map.height);

    // store map as a resource for tile lookups in other systems
    commands.insert_resource(crate::resources::WarehouseMap(map));
}

/// Sync visual box entities with shelf cargo count.
/// Removes boxes when cargo decreases, adds boxes when cargo increases.
/// Triggers on any Shelf component change (including initial add from populate_environment).
pub fn sync_shelf_boxes(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    shelves: Query<(Entity, &Shelf, Option<&Children>), Changed<Shelf>>,
    box_query: Query<&BoxCargo>,
) {
    for (shelf_entity, shelf, children) in &shelves {
        let current_boxes: Vec<Entity> = children
            .map(|c| {
                let mut boxes = Vec::new();
                for child in c.iter() {
                    if box_query.contains(child) {
                        boxes.push(child);
                    }
                }
                boxes
            })
            .unwrap_or_default();

        let target = shelf.cargo.min(SHELF_MAX_CAPACITY) as usize;
        let current = current_boxes.len();

        if current > target {
            // remove excess boxes (from the end)
            for &box_entity in current_boxes.iter().rev().take(current - target) {
                commands.entity(box_entity).despawn();
            }
        } else if current < target {
            // spawn missing boxes at available offsets
            let offsets = models::box_offsets();
            for i in current..target {
                if let Some(&offset) = offsets.get(i) {
                    let child = commands.spawn((
                        SceneRoot(asset_server.load(
                            format!("{}#Scene0", models::assets::BOX_SMALL)
                        )),
                        Transform::from_translation(offset)
                            .with_scale(Vec3::splat(BOX_SCALE)),
                        BoxCargo,
                    )).id();
                    commands.entity(shelf_entity).add_child(child);
                }
            }
        }
    }
}

/// Spawns the scene lighting
pub fn populate_lighting(mut commands: Commands) {
    // directional light (sun-like) for even illumination
    commands.spawn((
        DirectionalLight {
            illuminance: lighting::DIRECTIONAL_ILLUMINANCE,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // ambient light so shadows aren't pitch black
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: lighting::AMBIENT_BRIGHTNESS,
        affects_lightmapped_meshes: true
    });
}
