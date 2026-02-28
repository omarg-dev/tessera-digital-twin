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

// ── Wall classification (3x3 neighborhood tile-rule) ──

/// What kind of wall piece to place at a grid cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WallKind {
    /// Straight wall segment (with a Y rotation in radians)
    Straight(f32),
    /// Inner corner piece (concave L-turn, with a Y rotation in radians)
    CornerInner(f32),
    /// Outer corner piece (convex, with a Y rotation in radians)
    CornerOuter(f32),
}

/// 3x3 neighborhood around a wall tile.
/// Each field indicates whether that adjacent cell is also a wall (#).
///
/// ```text
///  NW  N  NE
///   W  *  E
///  SW  S  SE
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Neighborhood {
    pub n: bool,
    pub ne: bool,
    pub e: bool,
    pub se: bool,
    pub s: bool,
    pub sw: bool,
    pub w: bool,
    pub nw: bool,
}

impl Neighborhood {
    /// Build a neighborhood by sampling the token grid around (row, col).
    /// Out-of-bounds cells are treated as non-wall.
    pub fn from_grid(grid: &[Vec<&str>], row: usize, col: usize) -> Self {
        let rows = grid.len();
        let cols = if rows > 0 { grid[0].len() } else { 0 };

        let is_wall = |r: isize, c: isize| -> bool {
            r >= 0
                && c >= 0
                && (r as usize) < rows
                && (c as usize) < cols
                && grid[r as usize][c as usize] == "#"
        };

        let r = row as isize;
        let c = col as isize;

        Neighborhood {
            n:  is_wall(r - 1, c),
            ne: is_wall(r - 1, c + 1),
            e:  is_wall(r,     c + 1),
            se: is_wall(r + 1, c + 1),
            s:  is_wall(r + 1, c),
            sw: is_wall(r + 1, c - 1),
            w:  is_wall(r,     c - 1),
            nw: is_wall(r - 1, c - 1),
        }
    }

    /// Count cardinal wall neighbors (N, E, S, W)
    pub fn cardinal_count(&self) -> usize {
        [self.n, self.e, self.s, self.w]
            .iter()
            .filter(|&&b| b)
            .count()
    }
}

// ── Rotation constants ──
//
// Calibrated to the Kenney prototype-kit .glb models.
// If corners appear visually wrong, adjust CORNER_ROTATIONS.
// Each set must have uniform 90-degree steps.

use std::f32::consts::{FRAC_PI_2, PI};

/// Straight wall: model default faces +Z; PI flips to -Z.
const STRAIGHT_EW: f32 = PI;
/// Vertical run rotates an additional 90 degrees.
const STRAIGHT_NS: f32 = FRAC_PI_2 + PI;

/// Corner rotation lookup indexed by L-configuration.
///   index 0 = N+E,  index 1 = E+S,  index 2 = S+W,  index 3 = W+N
///
/// Derived from original tile rotations + PI offset (same correction
/// applied to straight walls). Uniform -PI/2 steps (90 degrees CW from
/// above) going NE → ES → SW → WN.
///
/// Used for BOTH inner and outer corner models.
/// If one corner type is consistently off, split into two arrays.
const CORNER_ROTATIONS: [f32; 4] = [
    0.0,            // N+E
    -FRAC_PI_2,     // E+S
    PI,             // S+W
    FRAC_PI_2,      // W+N
];

/// Classify a wall tile from its 3x3 neighborhood.
///
/// Rules (from the tile-rule book):
/// - 2 opposite cardinal neighbors (N+S or E+W) → straight
/// - 2 adjacent cardinal neighbors → check diagonal between them:
///   - diagonal is wall → outer corner (factory sees convex edge)
///   - diagonal is empty → inner corner (factory sees concave L-turn)
/// - 0, 1, 3, or 4 cardinal neighbors → straight fallback (no T/cross model)
pub fn classify_wall(nb: &Neighborhood) -> WallKind {
    let count = nb.cardinal_count();

    if count == 2 {
        // Opposite pairs → straight
        if nb.n && nb.s { return WallKind::Straight(STRAIGHT_NS); }
        if nb.e && nb.w { return WallKind::Straight(STRAIGHT_EW); }

        // Adjacent pair → corner
        let (diag_is_wall, idx) = if nb.n && nb.e {
            (nb.ne, 0)
        } else if nb.e && nb.s {
            (nb.se, 1)
        } else if nb.s && nb.w {
            (nb.sw, 2)
        } else {
            // w && n
            (nb.nw, 3)
        };

        let rotation = CORNER_ROTATIONS[idx];
        return if diag_is_wall {
            WallKind::CornerOuter(rotation)
        } else {
            WallKind::CornerInner(rotation)
        };
    }

    // 0, 1, 3, 4 cardinal neighbors → straight fallback
    if nb.n || nb.s {
        WallKind::Straight(STRAIGHT_NS)
    } else {
        WallKind::Straight(STRAIGHT_EW)
    }
}

/// Convenience wrapper: build neighborhood then classify.
pub fn classify_wall_from_grid(grid: &[Vec<&str>], row: usize, col: usize) -> WallKind {
    let nb = Neighborhood::from_grid(grid, row, col);
    classify_wall(&nb)
}

// ── Box placement ──

/// Number of boxes per shelf (3 levels x 4 boxes per level)
pub const BOXES_PER_SHELF: usize = SHELF_MAX_CAPACITY as usize;

/// Y-heights of the 3 usable shelf levels (relative to shelf origin)
const SHELF_LEVEL_HEIGHTS: [f32; 4] = [0.3, 0.6, 0.9, 1.2];

/// X offsets for the 2-column box grid per shelf level
const BOX_X_OFFSETS: [f32; 2] = [-0.20, 0.20];

/// Z offsets for the 2-row box grid per shelf level
const BOX_Z_OFFSETS: [f32; 2] = [-0.15, 0.15];

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
    let kind = classify_wall_from_grid(grid, row, col);

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

// ── Unit tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// Helper: construct a Neighborhood from individual booleans.
    fn nb(n: bool, ne: bool, e: bool, se: bool, s: bool, sw: bool, w: bool, nw: bool) -> Neighborhood {
        Neighborhood { n, ne, e, se, s, sw, w, nw }
    }

    fn rotation_of(kind: &WallKind) -> f32 {
        match kind {
            WallKind::Straight(r) | WallKind::CornerInner(r) | WallKind::CornerOuter(r) => *r,
        }
    }

    // ── Neighborhood::from_grid ──

    #[test]
    fn neighborhood_top_left_corner() {
        let grid = vec![
            vec!["#", "#", "."],
            vec!["#", ".", "."],
            vec![".", ".", "."],
        ];
        let n = Neighborhood::from_grid(&grid, 0, 0);
        assert!(!n.n);
        assert!(!n.nw);
        assert!(!n.w);
        assert!(!n.sw);
        assert!(n.e);    // (0,1) = #
        assert!(!n.ne);  // out of bounds
        assert!(n.s);    // (1,0) = #
        assert!(!n.se);  // (1,1) = .
    }

    #[test]
    fn neighborhood_center() {
        let grid = vec![
            vec!["#", "#", "#"],
            vec!["#", "#", "."],
            vec![".", "#", "."],
        ];
        let n = Neighborhood::from_grid(&grid, 1, 1);
        assert!(n.n);
        assert!(n.nw);
        assert!(n.ne);
        assert!(n.w);
        assert!(!n.e);
        assert!(!n.sw);
        assert!(n.s);
        assert!(!n.se);
    }

    #[test]
    fn neighborhood_bottom_right() {
        let grid = vec![
            vec![".", ".", "."],
            vec![".", "#", "#"],
            vec![".", "#", "#"],
        ];
        let n = Neighborhood::from_grid(&grid, 2, 2);
        assert!(!n.e);
        assert!(!n.se);
        assert!(!n.s);
        assert!(n.w);
        assert!(n.n);
        assert!(n.nw);
    }

    // ── Straight walls ──

    #[test]
    fn straight_horizontal_ew() {
        //  # * #   (E and W neighbors)
        let kind = classify_wall(&nb(false, false, true, false, false, false, true, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_EW);
    }

    #[test]
    fn straight_vertical_ns() {
        //  N above, S below
        let kind = classify_wall(&nb(true, false, false, false, true, false, false, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_NS);
    }

    #[test]
    fn straight_isolated_no_neighbors() {
        let kind = classify_wall(&nb(false, false, false, false, false, false, false, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_EW);
    }

    #[test]
    fn straight_single_east_neighbor() {
        let kind = classify_wall(&nb(false, false, true, false, false, false, false, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_EW);
    }

    #[test]
    fn straight_single_south_neighbor() {
        let kind = classify_wall(&nb(false, false, false, false, true, false, false, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_NS);
    }

    // ── Inner corners (diagonal EMPTY between two adjacent cardinal walls) ──

    #[test]
    fn inner_corner_ne() {
        // N and E present, NE empty → inner
        let kind = classify_wall(&nb(true, false, true, false, false, false, false, false));
        assert!(matches!(kind, WallKind::CornerInner(_)), "expected inner, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[0]);
    }

    #[test]
    fn inner_corner_es() {
        // E and S present, SE empty → inner
        let kind = classify_wall(&nb(false, false, true, false, true, false, false, false));
        assert!(matches!(kind, WallKind::CornerInner(_)), "expected inner, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[1]);
    }

    #[test]
    fn inner_corner_sw() {
        // S and W present, SW empty → inner
        let kind = classify_wall(&nb(false, false, false, false, true, false, true, false));
        assert!(matches!(kind, WallKind::CornerInner(_)), "expected inner, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[2]);
    }

    #[test]
    fn inner_corner_wn() {
        // W and N present, NW empty → inner
        let kind = classify_wall(&nb(true, false, false, false, false, false, true, false));
        assert!(matches!(kind, WallKind::CornerInner(_)), "expected inner, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[3]);
    }

    // ── Outer corners (diagonal PRESENT between two adjacent cardinal walls) ──

    #[test]
    fn outer_corner_ne() {
        // N and E present, NE wall → outer
        let kind = classify_wall(&nb(true, true, true, false, false, false, false, false));
        assert!(matches!(kind, WallKind::CornerOuter(_)), "expected outer, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[0]);
    }

    #[test]
    fn outer_corner_es() {
        // E and S present, SE wall → outer
        let kind = classify_wall(&nb(false, false, true, true, true, false, false, false));
        assert!(matches!(kind, WallKind::CornerOuter(_)), "expected outer, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[1]);
    }

    #[test]
    fn outer_corner_sw() {
        // S and W present, SW wall → outer
        let kind = classify_wall(&nb(false, false, false, false, true, true, true, false));
        assert!(matches!(kind, WallKind::CornerOuter(_)), "expected outer, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[2]);
    }

    #[test]
    fn outer_corner_wn() {
        // W and N present, NW wall → outer
        let kind = classify_wall(&nb(true, false, false, false, false, false, true, true));
        assert!(matches!(kind, WallKind::CornerOuter(_)), "expected outer, got {:?}", kind);
        assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[3]);
    }

    // ── T-junctions and 4-way (straight fallback) ──

    #[test]
    fn t_junction_3_neighbors_nse() {
        // N, E, S → straight fallback vertical
        let kind = classify_wall(&nb(true, false, true, false, true, false, false, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_NS);
    }

    #[test]
    fn t_junction_3_neighbors_esw() {
        // E, S, W → straight fallback vertical
        let kind = classify_wall(&nb(false, false, true, false, true, false, true, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_NS);
    }

    #[test]
    fn four_way_cross() {
        // all 4 cardinal → straight fallback vertical
        let kind = classify_wall(&nb(true, false, true, false, true, false, true, false));
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_NS);
    }

    // ── Rotation uniformity ──

    #[test]
    fn corner_rotations_uniform_90_degree_steps() {
        for i in 0..4 {
            let a = CORNER_ROTATIONS[i];
            let b = CORNER_ROTATIONS[(i + 1) % 4];
            let diff = (b - a).rem_euclid(std::f32::consts::TAU);
            assert!(
                (diff - FRAC_PI_2).abs() < 1e-6 || (diff - 3.0 * FRAC_PI_2).abs() < 1e-6,
                "non-uniform step between index {} and {}: diff={}", i, (i + 1) % 4, diff
            );
        }
    }

    // ── Integration: from_grid → classify ──

    #[test]
    fn from_grid_outer_corner_top_left() {
        let grid = vec![
            vec!["#", "#", "#"],
            vec!["#", ".",  "."],
            vec!["#", ".",  "."],
        ];
        // (0,0): E+S with SE=. → inner? No: SE=(1,1)=. so inner.
        // Actually (0,0): E=(0,1)=#, S=(1,0)=#, SE=(1,1)=. → inner
        let kind = classify_wall_from_grid(&grid, 0, 0);
        assert!(matches!(kind, WallKind::CornerInner(_)));
    }

    #[test]
    fn from_grid_outer_corner_in_filled_block() {
        let grid = vec![
            vec!["#", "#", "#"],
            vec!["#", "#", "#"],
            vec!["#", "#", "."],
        ];
        // (2,1): N=(1,1)=#, E=(2,2)=., W=(2,0)=#, NW=(1,0)=# → N+W, NW=# → outer
        let kind = classify_wall_from_grid(&grid, 2, 1);
        // only 1 cardinal of 2 adjacent... N and W, NW=#
        // Actually: N=(1,1)=#, W=(2,0)=# → 2 adjacent cardinals N+W, NW=# → outer
        // E=(2,2)=. and S is out of bounds
        assert!(matches!(kind, WallKind::CornerOuter(_)));
    }

    #[test]
    fn from_grid_straight_run() {
        let grid = vec![
            vec!["#", "#", "#"],
        ];
        let kind = classify_wall_from_grid(&grid, 0, 1);
        assert!(matches!(kind, WallKind::Straight(_)));
        assert_eq!(rotation_of(&kind), STRAIGHT_EW);
    }
}
