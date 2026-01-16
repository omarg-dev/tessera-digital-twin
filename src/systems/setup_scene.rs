use bevy::prelude::*;
use crate::components::*;

// Read from a .txt map file and populate the warehouse layout
pub fn setup_environment(
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

            match token {
                "." => {
                    // Ground tile
                        commands.spawn((
                            Mesh3d(meshes.add(Plane3d::default().mesh().size(tile_size, tile_size))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.9, 0.9, 0.9), // white-ish
                            ..default()
                        })),
                        Transform::from_translation(pos),
                        Ground {},
                    ));
                }
                "#" => {
                    // Wall tile (cube for visibility)
                    commands.spawn((
                        Mesh3d(meshes.add(Cuboid::new(tile_size, tile_size, tile_size))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.1, 0.1, 0.1), // black
                            ..default()
                        })),
                        Transform::from_translation(pos + Vec3::Y * 0.5),
                        Wall {},
                    ));
                }
                "_" => {
                    // Station tile
                    commands.spawn((
                        Mesh3d(meshes.add(Plane3d::default().mesh().size(tile_size, tile_size))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(1.0, 0.41, 0.71), // reddish-pink
                            ..default()
                        })),
                        Transform::from_translation(pos),
                        Station {},
                    ));
                }
                "v" => {
                    // Dropoff tile
                    commands.spawn((
                        Mesh3d(meshes.add(Plane3d::default().mesh().size(tile_size, tile_size))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.0, 1.0, 1.0), // cyan
                            ..default()
                        })),
                        Transform::from_translation(pos),
                        Dropoff {},
                    ));
                }
                _ if token.starts_with("x") && token.len() > 1 => {
                    // Shelf tile with capacity(c): xc
                    let capacity: u32 = token[1..]
                        .parse()
                        .unwrap_or(5); // default capacity if parse fails

                    commands.spawn((
                        Mesh3d(meshes.add(Cuboid::new(tile_size * 0.9, tile_size * 0.6, tile_size * 0.9))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.55, 0.27, 0.07), // brown
                            ..default()
                        })),
                        Transform::from_translation(pos + Vec3::Y * 0.3),
                        Shelf { _capacity: capacity },
                    ));
                    println!("Spawned shelf at ({}, {}) with capacity {}", x, y, capacity);
                }
                _ => {
                    // Unknown token - skip
                    // This includes ~ for N/A tile
                }
            }
        }
    }

    info!("Warehouse layout loaded from {}", path);
}

/// Spawns the scene lighting
pub fn setup_lighting(mut commands: Commands) {
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
