//! .glb model definitions and visual spawning utilities
//!
//! Centralizes model asset paths, weighted variant selection for wall types,
//! box placement offsets for shelves, and tile-specific spawn functions.
//! All visual model logic lives here to keep populate_scene focused on layout parsing.

use bevy::prelude::*;
use bevy::picking::prelude::Pickable;
use rand::Rng;
use crate::components::*;
use crate::resources::PlaceholderMeshes;
use protocol::config::visual::{GROUND_Y_OFFSET, PLACEHOLDER_Y_OFFSET, WALL_SEAM_SCALE};
use protocol::config::warehouse::SHELF_MAX_CAPACITY;
use protocol::config::visual::shelf::{SHELF_LEVEL_HEIGHTS,
    BOX_X_OFFSETS, BOX_Z_OFFSETS,BOX_SCALE};
use protocol::config::optimization as opt;

// ── Asset paths ──

pub mod assets {
    /// floor tile
    pub const FLOOR: &str = "models/floor.glb";
    /// standard wall segment
    pub const WALL: &str = "models/wall.glb";
    /// wall segment with window cutout
    pub const WALL_WINDOW: &str = "models/wall-windowed.glb";
    /// corner wall piece (bidirectional - works for both concave and convex)
    pub const CORNER: &str = "models/wall-corner.glb";
    /// T-junction wall piece (3-way intersection)
    pub const T_JUNCTION: &str = "models/wall-T.glb";
    /// endcap for walls with exactly one connecting neighbor
    pub const CAP: &str = "models/wall-cap.glb";
    /// pillar for isolated walls (no connecting neighbors)
    pub const PILLAR: &str = "models/wall-pillar.glb";
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
    /// Corner piece (bidirectional L-turn, with a Y rotation in radians)
    Corner(f32),
    /// T-junction piece (3-way intersection, with a Y rotation in radians)
    TJunction(f32),
    /// Endcap for a wall with exactly one connecting neighbor (with a Y rotation in radians)
    Cap(f32),
    /// Pillar for isolated walls with no connecting neighbors
    Pillar,
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
    /// Build a neighborhood by sampling a wall grid around (row, col).
    /// Each cell is `true` if it's a wall, `false` otherwise.
    /// Out-of-bounds cells are treated as non-wall.
    pub fn from_wall_grid(grid: &[Vec<bool>], row: usize, col: usize) -> Self {
        let rows = grid.len();
        let cols = if rows > 0 { grid[0].len() } else { 0 };

        let is_wall = |r: isize, c: isize| -> bool {
            r >= 0
                && c >= 0
                && (r as usize) < rows
                && (c as usize) < cols
                && grid[r as usize][c as usize]
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
/// Uniform 90-degree steps. Used for the bidirectional corner model.
const CORNER_ROTATIONS: [f32; 4] = [
    FRAC_PI_2,      // N+E
    0.0,            // E+S
    -FRAC_PI_2,     // S+W
    PI,             // W+N
];

/// T-junction rotation lookup indexed by missing cardinal direction.
///   index 0 = missing N (has E,S,W),  index 1 = missing E (has N,S,W)
///   index 2 = missing S (has N,E,W),  index 3 = missing W (has N,E,S)
///
/// Uniform 90-degree steps. Verify with wall_debug example.
const T_ROTATIONS: [f32; 4] = [
    0.0,            // missing N
    -FRAC_PI_2,     // missing W
    PI,             // missing S
    FRAC_PI_2,      // missing E
];

/// Endcap rotation lookup indexed by cardinal direction of the single neighbor.
///   index 0 = only N,  index 1 = only E,  index 2 = only S,  index 3 = only W
///
/// Uniform 90-degree steps. Calibrate with wall_debug example.
const CAP_ROTATIONS: [f32; 4] = [
    FRAC_PI_2,  // only N
    0.0,        // only E
    -FRAC_PI_2, // only S
    PI,         // only W
];

/// Classify a wall tile from its 3x3 neighborhood.
///
/// Rules:
/// - 0 cardinal neighbors → pillar (isolated wall)
/// - 1 cardinal neighbor → endcap (open end faces the single neighbor)
/// - 2 opposite cardinal neighbors (N+S or E+W) → straight
/// - 2 adjacent cardinal neighbors → corner
/// - 3 cardinal neighbors → T-junction (indexed by missing direction)
/// - 4 cardinal neighbors → straight fallback (no cross model)
///
/// Only cardinal (N/E/S/W) neighbors are considered. Diagonals are ignored.
pub fn classify_wall(nb: &Neighborhood) -> WallKind {
    let count = nb.cardinal_count();

    if count == 0 {
        return WallKind::Pillar;
    }

    if count == 1 {
        // endcap: open end faces the single neighbor
        let idx = if nb.n { 0 } else if nb.e { 1 } else if nb.s { 2 } else { 3 };
        return WallKind::Cap(CAP_ROTATIONS[idx]);
    }

    if count == 2 {
        // opposite pairs → straight
        if nb.n && nb.s { return WallKind::Straight(STRAIGHT_NS); }
        if nb.e && nb.w { return WallKind::Straight(STRAIGHT_EW); }

        // adjacent pair → corner
        let idx = if nb.n && nb.e { 0 }
            else if nb.e && nb.s { 1 }
            else if nb.s && nb.w { 2 }
            else { 3 }; // W+N

        return WallKind::Corner(CORNER_ROTATIONS[idx]);
    }

    if count == 3 {
        // T-junction indexed by which cardinal direction is missing
        let idx = if !nb.n { 0 }
            else if !nb.e { 1 }
            else if !nb.s { 2 }
            else { 3 }; // missing W

        return WallKind::TJunction(T_ROTATIONS[idx]);
    }

    // 4 cardinal neighbors → straight fallback (no cross model)
    if nb.n || nb.s {
        WallKind::Straight(STRAIGHT_NS)
    } else {
        WallKind::Straight(STRAIGHT_EW)
    }
}

/// Convenience wrapper: build neighborhood then classify.
pub fn classify_wall_from_grid(grid: &[Vec<bool>], row: usize, col: usize) -> WallKind {
    let nb = Neighborhood::from_wall_grid(grid, row, col);
    classify_wall(&nb)
}

/// Generate local-space positions for up to 12 boxes on a shelf.
/// Layout: 3 levels x 4 boxes (2x2 grid) = 12 total positions.
pub fn box_offsets() -> [Vec3; SHELF_MAX_CAPACITY as usize] {
    let mut offsets = [Vec3::ZERO; SHELF_MAX_CAPACITY as usize];
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
    let entity = commands.spawn((
        SceneRoot(load_scene(asset_server, assets::FLOOR)),
        Transform::from_translation(pos + Vec3::Y * GROUND_Y_OFFSET),
        Ground,
    )).id();
    if opt::DISABLE_TILE_PICKING {
        commands.entity(entity).insert(Pickable::IGNORE);
    }
}

/// Spawn a wall piece (straight, corner, or pillar) with correct rotation
pub fn spawn_wall(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    wall_grid: &[Vec<bool>],
    row: usize,
    col: usize,
) {
    let kind = classify_wall_from_grid(wall_grid, row, col);

    let (model_path, rotation) = match kind {
        WallKind::Straight(rot) => (pick_weighted(WALL_VARIANTS), rot),
        WallKind::Corner(rot)   => (assets::CORNER, rot),
        WallKind::TJunction(rot) => (assets::T_JUNCTION, rot),
        WallKind::Cap(rot)      => (assets::CAP, rot),
        WallKind::Pillar        => (assets::PILLAR, 0.0),
    };

    let entity = commands.spawn((
        SceneRoot(load_scene(asset_server, model_path)),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_y(rotation))
            .with_scale(Vec3::new(WALL_SEAM_SCALE, 1.0, WALL_SEAM_SCALE)),
        Wall,
    )).id();
    if opt::DISABLE_TILE_PICKING {
        commands.entity(entity).insert(Pickable::IGNORE);
    }
}

/// Spawn a shelf unit with cargo boxes as child entities
pub fn spawn_shelf(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    cargo: u32,
    max_capacity: u32,
) {
    let offsets = box_offsets();
    let box_count = (cargo as usize).min(offsets.len());

    commands.spawn((
        SceneRoot(load_scene(asset_server, assets::SHELF)),
        Transform::from_translation(pos),
        Shelf { cargo, max_capacity },
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

/// Spawn a station marker using shared placeholder mesh handles.
/// TODO: replace with .glb model when available
pub fn spawn_station(commands: &mut Commands, handles: &PlaceholderMeshes, pos: Vec3) {
    commands.spawn((
        Mesh3d(handles.station_mesh.clone()),
        MeshMaterial3d(handles.station_material.clone()),
        Transform::from_translation(pos + Vec3::Y * PLACEHOLDER_Y_OFFSET),
        Station,
    ));
}

/// Spawn a dropoff zone marker using shared placeholder mesh handles.
pub fn spawn_dropoff(commands: &mut Commands, handles: &PlaceholderMeshes, pos: Vec3) {
    commands.spawn((
        Mesh3d(handles.dropoff_mesh.clone()),
        MeshMaterial3d(handles.dropoff_material.clone()),
        Transform::from_translation(pos + Vec3::Y * PLACEHOLDER_Y_OFFSET),
        Dropoff,
    ));
}

// ── Unit tests ──

#[cfg(test)]
mod tests {
    use super::*;

    /// shorthand: build Neighborhood from bools in N NE E SE S SW W NW order
    fn nb(n: bool, ne: bool, e: bool, se: bool, s: bool, sw: bool, w: bool, nw: bool) -> Neighborhood {
        Neighborhood { n, ne, e, se, s, sw, w, nw }
    }

    fn rotation_of(kind: &WallKind) -> f32 {
        match kind {
            WallKind::Straight(r) | WallKind::Corner(r) | WallKind::TJunction(r) | WallKind::Cap(r) => *r,
            WallKind::Pillar => 0.0,
        }
    }

    #[test]
    fn neighborhood_sampling() {
        let grid = vec![
            vec![true,  true,  true],
            vec![true,  true,  false],
            vec![false, true,  false],
        ];
        // center cell: N, NW, NE, W, S present; E, SW, SE absent
        let n = Neighborhood::from_wall_grid(&grid, 1, 1);
        assert!(n.n && n.nw && n.ne && n.w && n.s);
        assert!(!n.e && !n.sw && !n.se);
        assert_eq!(n.cardinal_count(), 3);

        // top-left corner: out-of-bounds treated as empty
        let tl = Neighborhood::from_wall_grid(&grid, 0, 0);
        assert!(!tl.n && !tl.nw && !tl.w);
        assert!(tl.e && tl.s);
    }

    #[test]
    fn classify_all_types() {
        // pillar: 0 neighbors
        assert_eq!(classify_wall(&nb(false, false, false, false, false, false, false, false)), WallKind::Pillar);

        // straight: E+W
        let ew = classify_wall(&nb(false, false, true, false, false, false, true, false));
        assert!(matches!(ew, WallKind::Straight(_)));
        assert_eq!(rotation_of(&ew), STRAIGHT_EW);

        // straight: N+S
        let ns = classify_wall(&nb(true, false, false, false, true, false, false, false));
        assert!(matches!(ns, WallKind::Straight(_)));
        assert_eq!(rotation_of(&ns), STRAIGHT_NS);

        // single neighbor (endcap) → Cap, indexed by direction
        let only_e = classify_wall(&nb(false, false, true, false, false, false, false, false));
        assert!(matches!(only_e, WallKind::Cap(_)));
        assert_eq!(rotation_of(&only_e), CAP_ROTATIONS[1], "cap only-E rotation");

        let only_n = classify_wall(&nb(true, false, false, false, false, false, false, false));
        assert_eq!(rotation_of(&only_n), CAP_ROTATIONS[0], "cap only-N rotation");

        let only_s = classify_wall(&nb(false, false, false, false, true, false, false, false));
        assert_eq!(rotation_of(&only_s), CAP_ROTATIONS[2], "cap only-S rotation");

        let only_w = classify_wall(&nb(false, false, false, false, false, false, true, false));
        assert_eq!(rotation_of(&only_w), CAP_ROTATIONS[3], "cap only-W rotation");

        // diagonals alone do not affect cap classification
        let only_e_with_diags = classify_wall(&nb(false, true, true, true, false, true, false, true));
        assert_eq!(rotation_of(&only_e), rotation_of(&only_e_with_diags), "cap diagonal invariance");

        // corners: all 4 rotations, diagonal state ignored
        for (i, (n, e, s, w)) in [(true,true,false,false), (false,true,true,false),
            (false,false,true,true), (true,false,false,true)].iter().enumerate()
        {
            let kind = classify_wall(&nb(*n, false, *e, false, *s, false, *w, false));
            assert!(matches!(kind, WallKind::Corner(_)), "corner {i}: got {kind:?}");
            assert_eq!(rotation_of(&kind), CORNER_ROTATIONS[i], "corner {i} rotation");

            // diagonal doesn't change the result
            let with_diag = classify_wall(&nb(*n, true, *e, true, *s, true, *w, true));
            assert_eq!(rotation_of(&kind), rotation_of(&with_diag), "corner {i} diagonal invariance");
        }

        // T-junctions: 3 neighbors, indexed by missing direction
        let cases: [(bool,bool,bool,bool); 4] = [
            (false, true, true, true), // missing N
            (true, false, true, true), // missing E
            (true, true, false, true), // missing S
            (true, true, true, false), // missing W
        ];
        for (i, (n, e, s, w)) in cases.iter().enumerate() {
            let kind = classify_wall(&nb(*n, false, *e, false, *s, false, *w, false));
            assert!(matches!(kind, WallKind::TJunction(_)), "T {i}: got {kind:?}");
            assert_eq!(rotation_of(&kind), T_ROTATIONS[i], "T {i} rotation");
        }

        // 4-way cross → straight fallback
        let cross = classify_wall(&nb(true, false, true, false, true, false, true, false));
        assert!(matches!(cross, WallKind::Straight(_)));
    }

    #[test]
    fn from_grid_integration() {
        // straight run
        let grid = vec![vec![true, true, true]];
        assert!(matches!(classify_wall_from_grid(&grid, 0, 1), WallKind::Straight(_)));

        // corner at top-left of L-shape
        let grid = vec![
            vec![true,  true,  true],
            vec![true,  false, false],
            vec![true,  false, false],
        ];
        assert!(matches!(classify_wall_from_grid(&grid, 0, 0), WallKind::Corner(_)));
    }

    // ── Layout diagnostic ──

    /// Prints a classified wall map of the active layout.
    /// Run with: cargo test -p visualizer -- layout_wall_diagnostic --nocapture
    ///
    /// Legend:
    ///   ─  straight E-W    │  straight N-S
    ///   └  corner N+E      ┌  corner E+S
    ///   ┐  corner S+W      ┘  corner W+N
    ///   ┬  T missing N     ├  T missing E
    ///   ┴  T missing S     ┤  T missing W
    ///   *  pillar           .  non-wall tile
    #[test]
    fn layout_wall_diagnostic() {
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap().parent().unwrap();
        let layout_path = workspace_root.join(protocol::config::LAYOUT_FILE_PATH);

        let contents = std::fs::read_to_string(&layout_path)
            .unwrap_or_else(|e| panic!("failed to read layout file {:?}: {}", layout_path, e));

        let token_grid: Vec<Vec<&str>> = contents.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('/'))
            .map(|l| l.split_whitespace().collect())
            .collect();

        let wall_grid: Vec<Vec<bool>> = token_grid.iter()
            .map(|row| row.iter().map(|&t| t == "#").collect())
            .collect();

        println!("\n=== Wall Classification Diagnostic ===");
        println!("Layout: {}", protocol::config::LAYOUT_FILE_PATH);
        println!("Grid size: {} rows x {} cols\n", wall_grid.len(),
            wall_grid.first().map_or(0, |r| r.len()));

        let symbol = |kind: &WallKind| -> &str {
            match kind {
                WallKind::Straight(r) => {
                    if (*r - STRAIGHT_EW).abs() < 0.01 { "─" }
                    else if (*r - STRAIGHT_NS).abs() < 0.01 { "│" }
                    else { "?" }
                }
                WallKind::Corner(r) => {
                    if (*r - CORNER_ROTATIONS[0]).abs() < 0.01 { "└" }
                    else if (*r - CORNER_ROTATIONS[1]).abs() < 0.01 { "┌" }
                    else if (*r - CORNER_ROTATIONS[2]).abs() < 0.01 { "┐" }
                    else if (*r - CORNER_ROTATIONS[3]).abs() < 0.01 { "┘" }
                    else { "?" }
                }
                WallKind::TJunction(r) => {
                    if (*r - T_ROTATIONS[0]).abs() < 0.01 { "┬" }
                    else if (*r - T_ROTATIONS[1]).abs() < 0.01 { "├" }
                    else if (*r - T_ROTATIONS[2]).abs() < 0.01 { "┴" }
                    else if (*r - T_ROTATIONS[3]).abs() < 0.01 { "┤" }
                    else { "?" }
                }
                WallKind::Cap(r) => {
                    if (*r - CAP_ROTATIONS[0]).abs() < 0.01 { "╵" }
                    else if (*r - CAP_ROTATIONS[1]).abs() < 0.01 { "╶" }
                    else if (*r - CAP_ROTATIONS[2]).abs() < 0.01 { "╷" }
                    else if (*r - CAP_ROTATIONS[3]).abs() < 0.01 { "╴" }
                    else { "?" }
                }
                WallKind::Pillar => "*",
            }
        };

        for (row, walls) in wall_grid.iter().enumerate() {
            let mut line = String::new();
            for (col, &is_wall) in walls.iter().enumerate() {
                if is_wall {
                    line.push_str(symbol(&classify_wall_from_grid(&wall_grid, row, col)));
                } else {
                    let token = &token_grid[row][col];
                    if *token == "~" { line.push(' '); } else { line.push('.'); }
                }
                line.push(' ');
            }
            println!("  row {:2}: {}", row, line);
        }
        println!();
    }
}
