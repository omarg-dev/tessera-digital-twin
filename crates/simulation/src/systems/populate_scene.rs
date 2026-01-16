use bevy::prelude::*;
use crate::components::*;

// Read from a .txt map file and populate the warehouse layout
pub fn populate_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Read layout file directly (blocking I/O - fine for startup)
    let path = "assets/data/layout.txt";
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to load layout file: {}", e);
            return;
        }
    };
    // array to hold all tiles
    let mut tiles: Vec<(Entity, usize, usize)> = Vec::new();
    let mut max_x: usize = 0;
    let mut max_y: usize = 0;

    // Legend:
    // #       wall    black
    // .       ground  white
    // xc    shelf   brown (c = capacity)
    // _       station reddish-pink
    // v       dropoff cyan

    let tile_size = 1.0;

    for (y, line) in contents.lines().enumerate() {
        // Skip empty lines or comments (starting with '/')
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('/') {
            continue;
        }

        // Split by whitespace to get tokens
        for (x, token) in trimmed.split_whitespace().enumerate() {
            let pos = Vec3::new(x as f32 * tile_size, 0.0, y as f32 * tile_size);

            max_x = max_x.max(x);
            max_y = max_y.max(y);

            match token {
                "." => {
                    // Ground tile (white-ish)
                    tiles.push(instantiate_plane(&mut commands, &mut meshes, &mut materials, tile_size, pos, Vec3::new(0.9, 0.9, 0.9), Ground {}, x, y));
                }
                "#" => {
                    // Wall Cube tile (dark gray)
                    tiles.push(instantiate_cube(&mut commands, &mut meshes, &mut materials, pos, Vec3::new(0.1, 0.1, 0.1), Vec3::splat(tile_size), Wall {}, x, y));
                }
                "_" => {
                    // Station tile (reddish-pink)
                    tiles.push(instantiate_plane(&mut commands, &mut meshes, &mut materials, tile_size, pos, Vec3::new(1.0, 0.4, 0.6), Station {}, x, y));
                }
                "v" => {
                    // Dropoff tile (light green)
                    tiles.push(instantiate_plane(&mut commands, &mut meshes, &mut materials, tile_size, pos, Vec3::new(0.0, 1.0, 0.4), Dropoff {}, x, y));
                }
                _ if token.starts_with("x") && token.len() > 1 => {
                    // Shelf Cube tile with capacity(c): xc
                    let capacity: u32 = token[1..]
                        .parse()
                        .unwrap_or(5); // default capacity if parse fails

                    tiles.push(instantiate_cube(&mut commands, &mut meshes, &mut materials, pos, Vec3::new(0.6, 0.4, 0.2), Vec3::new(0.8, 0.6, 0.8), Shelf { _capacity: capacity }, x, y));
                    println!("Spawned shelf at ({}, {}) with capacity {}", x, y, capacity);
                    
                    // Also spawn ground tile beneath shelf (don't add to tiles list - shelf takes priority)
                    instantiate_plane(&mut commands, &mut meshes, &mut materials, tile_size, pos, Vec3::new(0.9, 0.9, 0.9), Ground {}, x, y);
                }
                _ => {
                    // Unknown token - skip
                    // This includes ~ for N/A tile
                }
            }
        }
    }

    info!("Warehouse layout loaded from {}", path);
    
    // Store the tile map as a resource
    commands.insert_resource(crate::resources::TileMap {
        tiles: tiles,
        width: max_x + 1,
        height: max_y + 1,
    });
}

fn instantiate_cube(
    commands: &mut Commands<'_, '_>,
    meshes: &mut ResMut<'_, Assets<Mesh>>,
    materials: &mut ResMut<'_, Assets<StandardMaterial>>,
    pos: Vec3,
    rgb: Vec3,
    size: Vec3,
    component: impl Component,
    x: usize,
    y: usize,
) -> (Entity, usize, usize) {
    let entity = commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(rgb.x, rgb.y, rgb.z),
            ..default()
        })),
        Transform::from_translation(pos + Vec3::Y * (size.y / 2.0)),
        component,
    )).id();
    (entity, x, y)
}

fn instantiate_plane(
    commands: &mut Commands<'_, '_>,
    meshes: &mut ResMut<'_, Assets<Mesh>>,
    materials: &mut ResMut<'_, Assets<StandardMaterial>>,
    tile_size: f32,
    pos: Vec3,
    rgb: Vec3,
    component: impl Component,
    x: usize,
    y: usize,
) -> (Entity, usize, usize) {
    let entity = commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(tile_size, tile_size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(rgb.x, rgb.y, rgb.z),
            ..default()
        })),
        Transform::from_translation(pos),
        component,
    )).id();
    (entity, x, y)
}

/// Spawns the scene lighting
pub fn populate_lighting(mut commands: Commands) {
    // Directional light (sun-like) for even illumination
    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Ambient light so shadows aren't pitch black
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
        affects_lightmapped_meshes: true
    });
}
