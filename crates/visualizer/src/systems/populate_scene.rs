use bevy::prelude::*;
use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use crate::components::*;
use crate::resources::PlaceholderMeshes;
use crate::systems::models;
use protocol::config::{LAYOUT_FILE_PATH,
    visual::{TILE_SIZE, shelf, lighting, colors, ROBOT_SIZE},
    warehouse};
use protocol::config::optimization as opt;
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

    // pre-allocate shared placeholder meshes (station, dropoff, robot)
    let placeholders = PlaceholderMeshes {
        station_mesh: meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)),
        station_material: materials.add(StandardMaterial {
            base_color: Color::srgb(colors::STATION.0, colors::STATION.1, colors::STATION.2),
            ..default()
        }),
        dropoff_mesh: meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)),
        dropoff_material: materials.add(StandardMaterial {
            base_color: Color::srgb(colors::DROPOFF.0, colors::DROPOFF.1, colors::DROPOFF.2),
            ..default()
        }),
        robot_mesh: meshes.add(Cuboid::new(ROBOT_SIZE, ROBOT_SIZE, ROBOT_SIZE)),
        robot_material: materials.add(StandardMaterial {
            base_color: Color::srgb(colors::ROBOT.0, colors::ROBOT.1, colors::ROBOT.2),
            ..default()
        }),
    };

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
                models::spawn_station(&mut commands, &placeholders, pos);
            }
            TileType::Dropoff => {
                models::spawn_floor(&mut commands, &asset_server, pos);
                models::spawn_dropoff(&mut commands, &placeholders, pos);
            }
            TileType::Shelf(initial_stock) => {
                models::spawn_floor(&mut commands, &asset_server, pos);
                // initial_stock from layout token (xN); max from global warehouse config
                models::spawn_shelf(&mut commands, &asset_server, pos,
                    initial_stock as u32, warehouse::SHELF_MAX_CAPACITY);
            }
            TileType::Empty => {
                // N/A tile (~), skip
            }
        }
    }

    info!("Warehouse layout loaded: {}x{} (GridMap)", map.width, map.height);

    // store resources for other systems
    commands.insert_resource(placeholders);
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

        let target = shelf.cargo.min(shelf.max_capacity) as usize;
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
                            .with_scale(Vec3::splat(shelf::BOX_SCALE)),
                        BoxCargo,
                    )).id();
                    commands.entity(shelf_entity).add_child(child);
                }
            }
        }
    }
}

/// After .glb scenes finish loading, propagate shadow and picking optimizations to
/// the actual `Mesh3d` entities inside tile scene hierarchies.
///
/// Each `Ground` or `Wall` root entity's child meshes are tagged with
/// `NotShadowCaster` (and `NotShadowReceiver` for floors) the first time they
/// are seen. The `Without` filter makes this a no-op once all tiles are tagged.
pub fn propagate_tile_optimizations(
    mut commands: Commands,
    ground_tiles: Query<Entity, With<Ground>>,
    wall_tiles: Query<Entity, With<Wall>>,
    uncast_meshes: Query<Entity, (With<Mesh3d>, Without<NotShadowCaster>)>,
    unrecv_meshes: Query<Entity, (With<Mesh3d>, Without<NotShadowReceiver>)>,
    children_q: Query<&Children>,
) {
    if !opt::DISABLE_TILE_SHADOW_CAST && !opt::DISABLE_FLOOR_SHADOW_RECEIVE {
        return;
    }

    // floors: shadow cast + shadow receive suppression
    if opt::DISABLE_TILE_SHADOW_CAST || opt::DISABLE_FLOOR_SHADOW_RECEIVE {
        for root in ground_tiles.iter() {
            let mut stack = vec![root];
            while let Some(e) = stack.pop() {
                if e != root {
                    if opt::DISABLE_TILE_SHADOW_CAST && uncast_meshes.get(e).is_ok() {
                        commands.entity(e).insert(NotShadowCaster);
                    }
                    if opt::DISABLE_FLOOR_SHADOW_RECEIVE && unrecv_meshes.get(e).is_ok() {
                        commands.entity(e).insert(NotShadowReceiver);
                    }
                }
                if let Ok(children) = children_q.get(e) {
                    for &child in children {
                        stack.push(child);
                    }
                }
            }
        }
    }

    // walls: shadow cast suppression only
    if opt::DISABLE_TILE_SHADOW_CAST {
        for root in wall_tiles.iter() {
            let mut stack = vec![root];
            while let Some(e) = stack.pop() {
                if e != root && uncast_meshes.get(e).is_ok() {
                    commands.entity(e).insert(NotShadowCaster);
                }
                if let Ok(children) = children_q.get(e) {
                    for &child in children {
                        stack.push(child);
                    }
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
            shadows_enabled: !opt::DISABLE_DIRECTIONAL_SHADOWS,
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
