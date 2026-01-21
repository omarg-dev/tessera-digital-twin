use bevy::prelude::*;
use crate::components::*;
use protocol::config::{LAYOUT_FILE_PATH, visual::colors, visual::TILE_SIZE, visual::SHELF_SIZE, visual::lighting};

/// Check if environment reload is requested and trigger repopulation
pub fn check_reload_environment(
    mut commands: Commands,
    reload_trigger: Option<ResMut<crate::systems::commands::ReloadEnvironment>>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
) {
    if reload_trigger.is_some() {
        println!("↻ Reloading warehouse environment");
        populate_environment(commands.reborrow(), meshes, materials);
        // Remove the trigger resource
        commands.remove_resource::<crate::systems::commands::ReloadEnvironment>();
    }
}

// Read from a .txt map file and populate the warehouse layout
pub fn populate_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Read layout file directly (blocking I/O - fine for startup)
    let contents = match std::fs::read_to_string(LAYOUT_FILE_PATH) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to load layout file: {}", e);
            return;
        }
    };

    let mut max_x: usize = 0;
    let mut max_y: usize = 0;

    // Track actual row index (skipping comments/empty lines)
    let mut row_index = 0;
    
    for line in contents.lines() {
        // Skip empty lines or comments (starting with '/')
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('/') {
            continue;
        }

        // Split by whitespace to get tokens
        for (x, token) in trimmed.split_whitespace().enumerate() {
            let pos = Vec3::new(x as f32 * TILE_SIZE, 0.0, row_index as f32 * TILE_SIZE);

            max_x = max_x.max(x);
            max_y = max_y.max(row_index);

            match token {
                "." => {
                    // Ground tile
                    instantiate(&mut commands, &mut meshes, &mut materials, pos,
                        TileShape::Plane(TILE_SIZE), colors::GROUND, Ground {});
                }
                "#" => {
                    // Wall Cube tile
                    instantiate(&mut commands, &mut meshes, &mut materials, pos,
                        TileShape::Cube(Vec3::splat(TILE_SIZE)), colors::WALL, Wall {});
                }
                "_" => {
                    // Station tile
                    instantiate(&mut commands, &mut meshes, &mut materials, pos,
                        TileShape::Plane(TILE_SIZE), colors::STATION, Station {});
                }
                "v" => {
                    // Dropoff tile
                    instantiate(&mut commands, &mut meshes, &mut materials, pos,
                        TileShape::Plane(TILE_SIZE), colors::DROPOFF, Dropoff {});
                }
                _ if token.starts_with("x") && token.len() > 1 => {
                    // Shelf Cube tile with capacity(c): xc
                    let capacity: u32 = token[1..]
                        .parse()
                        .unwrap_or(5); // default capacity if parse fails

                    let shelf_size = Vec3::new(SHELF_SIZE.0, SHELF_SIZE.1, SHELF_SIZE.2);
                    instantiate(&mut commands, &mut meshes, &mut materials, pos,
                        TileShape::Cube(shelf_size), colors::SHELF, Shelf { capacity });
                    
                    // Also spawn ground tile beneath shelf
                    instantiate(&mut commands, &mut meshes, &mut materials, pos,
                        TileShape::Plane(TILE_SIZE), colors::GROUND, Ground {});
                }
                _ => {
                    // Unknown token - skip (includes ~ for N/A tile)
                }
            }
        }
        
        // Increment row index for next non-empty line
        row_index += 1;
    }

    info!("Warehouse layout loaded: {}x{}", max_x + 1, max_y + 1);
}

/// Shape type for tile instantiation
enum TileShape {
    Cube(Vec3),   // size (x, y, z)
    Plane(f32),   // tile_size
}

/// Spawns a tile entity with the given shape, color, and component
fn instantiate(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec3,
    shape: TileShape,
    rgb: (f32, f32, f32),
    component: impl Component,
) {
    let (mesh, transform) = match shape {
        TileShape::Cube(size) => (
            meshes.add(Cuboid::new(size.x, size.y, size.z)),
            Transform::from_translation(pos + Vec3::Y * (size.y / 2.0)),
        ),
        TileShape::Plane(tile_size) => (
            meshes.add(Plane3d::default().mesh().size(tile_size, tile_size)),
            Transform::from_translation(pos),
        ),
    };

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(rgb.0, rgb.1, rgb.2),
            ..default()
        })),
        transform,
        component,
    ));
}

/// Spawns the scene lighting
pub fn populate_lighting(mut commands: Commands) {
    // Directional light (sun-like) for even illumination
    commands.spawn((
        DirectionalLight {
            illuminance: lighting::DIRECTIONAL_ILLUMINANCE,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Ambient light so shadows aren't pitch black
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: lighting::AMBIENT_BRIGHTNESS,
        affects_lightmapped_meshes: true
    });
}
