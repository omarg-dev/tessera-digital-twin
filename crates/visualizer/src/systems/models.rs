//! .glb model definitions and visual spawning utilities
//!
//! Centralizes model asset paths, weighted variant selection for wall types,
//! box placement offsets for shelves, and tile-specific spawn functions.
//! All visual model logic lives here to keep populate_scene focused on layout parsing.

use bevy::prelude::*;
use rand::Rng;
use crate::components::*;
use protocol::config::visual::{TILE_SIZE, colors, SHELF_MAX_CAPACITY, BOX_SCALE, PLACEHOLDER_Y_OFFSET};

// ── Asset paths ──

pub mod assets {
    /// floor tile
    pub const FLOOR: &str = "models/floor.glb";
    /// standard wall segment
    pub const WALL: &str = "models/wall.glb";
    /// wall segment with window cutout
    pub const WALL_WINDOW: &str = "models/wall_window.glb";
    /// inner corner wall piece
    pub const CORNER_INNER: &str = "models/structure-corner-inner.glb";
    /// outer corner wall piece
    pub const CORNER_OUTER: &str = "models/structure-corner-outer.glb";
    /// shelf unit (3 usable levels)
    pub const SHELF: &str = "models/shelf.glb";
    /// small cargo box
    pub const BOX_SMALL: &str = "models/box-small.glb";
    // /// robot chassis model
    // pub const ROBOT: &str = "models/robot.glb";
    // /// charging station model
    // pub const STATION: &str = "models/station.glb";
}

// ── Weighted variant selection ──

/// A model variant with a relative probability weight.
/// Higher weight = more frequently selected. NOT a percentage.
pub struct WeightedVariant {
    pub path: &'static str,
    pub weight: u32,
}

/// Wall model variants with selection probabilities
pub const WALL_VARIANTS: &[WeightedVariant] = &[
    WeightedVariant { path: assets::WALL, weight: 10 },
    WeightedVariant { path: assets::WALL_WINDOW, weight: 1 },
];

/// Pick a random variant from a weighted list of options.
pub fn pick_weighted(variants: &[WeightedVariant]) -> &'static str {
    let total: u32 = variants.iter().map(|v| v.weight).sum();
    let mut roll = rand::thread_rng().gen_range(0..total);
    for variant in variants {
        if roll < variant.weight {
            return variant.path;
        }
        roll -= variant.weight;
    }
    variants.last().expect("variants must not be empty").path
}

// ── Wall classification ──

/// What kind of wall piece to place at a grid cell.
pub enum WallKind {
    /// Straight wall segment (with a Y rotation in radians)
    Straight(f32),
    /// Inner corner piece (concave, with a Y rotation in radians)
    CornerInner(f32),
    /// Outer corner piece (convex, with a Y rotation in radians)
    CornerOuter(f32),
}

/// Classify a wall tile based on its 4-connected and diagonal neighbors.
/// Returns the wall kind and its Y-axis rotation.
///
/// Corner detection uses the diagonal cell between two orthogonal wall neighbors:
///   - diagonal is wall   → outer corner (factory sees the convex outside edge)
///   - diagonal is empty  → inner corner (factory is inside the concave L-turn)
///
/// 3+ neighbors (T-junctions, crossings) are not in the tileset, fall back to straight.
pub fn classify_wall(grid: &[Vec<&str>], row: usize, col: usize) -> WallKind {
    let rows = grid.len();
    let cols = if rows > 0 { grid[0].len() } else { 0 };

    let w = |r: usize, c: usize| -> bool {
        r < rows && c < cols && grid[r][c] == "#"
    };

    let up    = row > 0 && w(row - 1, col);
    let down  = row + 1 < rows && w(row + 1, col);
    let left  = col > 0 && w(row, col - 1);
    let right = col + 1 < cols && w(row, col + 1);

    let count = [up, down, left, right].iter().filter(|&&b| b).count();
    use std::f32::consts::{FRAC_PI_2, PI};

    // 2 orthogonal neighbors → corner piece
    // check the diagonal between the two wall neighbors to pick inner vs outer
    if count == 2 {
        let (is_corner, diag_is_wall, rotation) = if up && right {
            let d = row > 0 && col + 1 < cols && w(row - 1, col + 1);
            (true, d, -FRAC_PI_2)
        } else if right && down {
            let d = row + 1 < rows && col + 1 < cols && w(row + 1, col + 1);
            (true, d, 0.0)
        } else if down && left {
            let d = row + 1 < rows && col > 0 && w(row + 1, col - 1);
            (true, d, FRAC_PI_2)
        } else if left && up {
            let d = row > 0 && col > 0 && w(row - 1, col - 1);
            (true, d, PI)
        } else {
            (false, false, 0.0) // in-line, not a corner
        };

        if is_corner {
            return if diag_is_wall {
                WallKind::CornerOuter(rotation)
            } else {
                WallKind::CornerInner(rotation)
            };
        }
    }

    // 3+ neighbors (T-junctions, crossings) → straight fallback (tileset incomplete)
    // 1 neighbor, 0 neighbors, or 2 in-line → straight segment
    if (up || down) && !left && !right {
        WallKind::Straight(FRAC_PI_2 + PI)
    } else {
        WallKind::Straight(PI)
    }
}

// ── Box placement ──

/// Number of boxes per shelf (3 levels x 4 boxes per level)
pub const BOXES_PER_SHELF: usize = SHELF_MAX_CAPACITY as usize;

/// Y-heights of the 3 usable shelf levels (relative to shelf origin)
const SHELF_LEVEL_HEIGHTS: [f32; 4] = [0.3, 0.6, 0.9, 1.2];

/// X offsets for the 2-column box grid per shelf level
const BOX_X_OFFSETS: [f32; 2] = [-0.15, 0.15];

/// Z offsets for the 2-row box grid per shelf level
const BOX_Z_OFFSETS: [f32; 2] = [-0.1, 0.1];

/// Generate local-space positions for up to 12 boxes on a shelf.
/// Layout: 3 levels x 4 boxes (2x2 grid) = 12 total positions.
pub fn box_offsets() -> [Vec3; BOXES_PER_SHELF] {
    let mut offsets = [Vec3::ZERO; BOXES_PER_SHELF];
    let mut i = 0;
    for &y in &SHELF_LEVEL_HEIGHTS {
        for &x in &BOX_X_OFFSETS {
            for &z in &BOX_Z_OFFSETS {
                offsets[i] = Vec3::new(x, y, z);
                i += 1;
            }
        }
    }
    offsets
}

// ── Scene loading helper ──

/// Load a .glb model's first scene
fn load_scene(asset_server: &AssetServer, path: &str) -> Handle<Scene> {
    asset_server.load(format!("{path}#Scene0"))
}

// ── Spawn functions ──

/// Spawn a floor tile
pub fn spawn_floor(commands: &mut Commands, asset_server: &AssetServer, pos: Vec3) {
    commands.spawn((
        SceneRoot(load_scene(asset_server, assets::FLOOR)),
        Transform::from_translation(pos),
        Ground,
    ));
}

/// Spawn a wall piece (straight, inner corner, or outer corner) with correct rotation
pub fn spawn_wall(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    grid: &[Vec<&str>],
    row: usize,
    col: usize,
) {
    let kind = classify_wall(grid, row, col);

    let (model_path, rotation) = match kind {
        WallKind::Straight(rot) => (pick_weighted(WALL_VARIANTS), rot),
        WallKind::CornerInner(rot) => (assets::CORNER_INNER, rot),
        WallKind::CornerOuter(rot) => (assets::CORNER_OUTER, rot),
    };

    commands.spawn((
        SceneRoot(load_scene(asset_server, model_path)),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_y(rotation)),
        Wall,
    ));
}

/// Spawn a shelf unit with cargo boxes as child entities
pub fn spawn_shelf(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    cargo: u32,
) {
    let offsets = box_offsets();
    let box_count = (cargo as usize).min(offsets.len());

    commands.spawn((
        SceneRoot(load_scene(asset_server, assets::SHELF)),
        Transform::from_translation(pos),
        Shelf { cargo },
    )).with_children(|parent| {
        for offset in offsets.iter().take(box_count) {
            parent.spawn((
                SceneRoot(load_scene(asset_server, assets::BOX_SMALL)),
                Transform::from_translation(*offset)
                    .with_scale(Vec3::splat(BOX_SCALE)),
                BoxCargo,
            ));
        }
    });
}

/// Spawn a station marker (primitive mesh until .glb model is available)
pub fn spawn_station(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(colors::STATION.0, colors::STATION.1, colors::STATION.2),
            ..default()
        })),
        Transform::from_translation(pos + Vec3::Y * PLACEHOLDER_Y_OFFSET),
        Station,
    ));
    // TODO: replace with .glb model when available
    // fn spawn_station_model(
    //     commands: &mut Commands,
    //     asset_server: &AssetServer,
    //     pos: Vec3,
    // ) {
    //     commands.spawn((
    //         SceneRoot(load_scene(asset_server, assets::STATION)),
    //         Transform::from_translation(pos),
    //         Station,
    //     ));
    // }
}

/// Spawn a dropoff zone marker (primitive mesh)
pub fn spawn_dropoff(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(colors::DROPOFF.0, colors::DROPOFF.1, colors::DROPOFF.2),
            ..default()
        })),
        Transform::from_translation(pos + Vec3::Y * PLACEHOLDER_Y_OFFSET),
        Dropoff,
    ));
}

// ── Robot (prepared, commented out until model is available) ──

// /// Spawn a robot entity with .glb model.
// /// Replace the primitive mesh spawn in sync_robots.rs with this
// /// once the robot .glb model is added to assets/.
// pub fn spawn_robot(
//     commands: &mut Commands,
//     asset_server: &AssetServer,
//     pos: Vec3,
//     id: u32,
//     state: protocol::RobotState,
//     battery: f32,
// ) -> Entity {
//     commands.spawn((
//         SceneRoot(load_scene(asset_server, assets::ROBOT)),
//         Transform::from_translation(pos),
//         crate::components::Robot {
//             id,
//             state,
//             position: pos,
//             battery,
//             current_task: None,
//             carrying_cargo: None,
//         },
//     )).id()
// }
