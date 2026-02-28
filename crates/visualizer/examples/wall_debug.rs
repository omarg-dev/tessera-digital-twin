//! Wall debug test bench
//!
//! Spawns a controlled grid of wall pieces at known rotations so you can
//! visually verify model orientation without loading the full warehouse.
//!
//! Run with: cargo run -p visualizer --example wall_debug
//!
//! Controls:
//!   - Orbit: right-click + drag
//!   - Zoom: scroll wheel
//!   - Pan: middle-click + drag
//!
//! Layout (top-down, each cell is 2 units apart):
//!
//!   Column 0        Column 1        Column 2       Column 3
//!   ────────        ────────        ────────       ────────
//!   Row 0: Straight EW (PI)        Straight NS (PI/2+PI)  <reference arrows>
//!   Row 1: Inner NE (idx0)  Inner ES (idx1) Inner SW (idx2) Inner WN (idx3)
//!   Row 2: Outer NE (idx0)  Outer ES (idx1) Outer SW (idx2) Outer WN (idx3)
//!   Row 3: Straight rot=0   Straight rot=PI/2  Straight rot=PI  Straight rot=3PI/2
//!
//! Each wall piece has:
//!   - RED arrow: local +X direction of the model
//!   - BLUE arrow: local +Z direction of the model
//!   - GREEN plane: floor reference
//!   - Text label above showing the rotation

use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Wall Debug Test Bench".to_string(),
                resolution: (1280, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup_scene)
        .add_systems(Update, orbit_camera)
        .run();
}

// ── Rotation constants (must match models.rs) ──

const STRAIGHT_EW: f32 = PI;
const STRAIGHT_NS: f32 = FRAC_PI_2 + PI;

const CORNER_ROTATIONS: [f32; 4] = [
    0.0,
    -FRAC_PI_2,
    PI,
    FRAC_PI_2,
];

// ── Asset paths (must match models.rs) ──

const WALL: &str = "models/wall.glb";
const CORNER_INNER: &str = "models/structure-corner-inner.glb";
const CORNER_OUTER: &str = "models/structure-corner-outer.glb";

const SPACING: f32 = 3.0;

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // camera: angled top-down
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 18.0, 14.0).looking_at(Vec3::new(5.0, 0.0, 4.0), Vec3::Y),
    ));

    // lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
        ..default()
    });

    // shared materials
    let red_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.0, 0.0),
        unlit: true,
        ..default()
    });
    let blue_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.0, 1.0),
        unlit: true,
        ..default()
    });
    let green_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.8, 0.2),
        ..default()
    });
    let yellow_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.0),
        unlit: true,
        ..default()
    });
    let _label_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 1.0),
        unlit: true,
        ..default()
    });

    // arrow mesh for direction indicators
    let arrow_mesh = meshes.add(Cuboid::new(0.8, 0.05, 0.1));
    let arrow_head = meshes.add(Cuboid::new(0.15, 0.06, 0.25));
    let floor_mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));

    // ── World reference arrows at origin ──
    // +X arrow (RED) at world origin
    commands.spawn((
        Mesh3d(arrow_mesh.clone()),
        MeshMaterial3d(red_mat.clone()),
        Transform::from_xyz(-2.0, 0.1, -1.5)
    ));
    commands.spawn((
        Mesh3d(arrow_head.clone()),
        MeshMaterial3d(red_mat.clone()),
        Transform::from_xyz(-1.6, 0.1, -1.5)
    ));
    // +Z arrow (BLUE) at world origin
    commands.spawn((
        Mesh3d(arrow_mesh.clone()),
        MeshMaterial3d(blue_mat.clone()),
        Transform::from_xyz(-2.0, 0.1, -1.0)
            .with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
    ));
    commands.spawn((
        Mesh3d(arrow_head.clone()),
        MeshMaterial3d(blue_mat.clone()),
        Transform::from_xyz(-2.0, 0.1, -0.6),
    ));

    // ── Row labels (using small cubes as markers) ──
    struct WallTestEntry {
        label: &'static str,
        model: &'static str,
        rotation: f32,
    }

    // Row 0: straight walls at the actual rotation constants
    let row0 = vec![
        WallTestEntry { label: "EW (PI)", model: WALL, rotation: STRAIGHT_EW },
        WallTestEntry { label: "NS (PI/2+PI)", model: WALL, rotation: STRAIGHT_NS },
        WallTestEntry { label: "rot=0", model: WALL, rotation: 0.0 },
        WallTestEntry { label: "rot=PI/2", model: WALL, rotation: FRAC_PI_2 },
    ];

    // Row 1: inner corners
    let row1 = vec![
        WallTestEntry { label: "inn NE (0)", model: CORNER_INNER, rotation: CORNER_ROTATIONS[0] },
        WallTestEntry { label: "inn ES (-PI/2)", model: CORNER_INNER, rotation: CORNER_ROTATIONS[1] },
        WallTestEntry { label: "inn SW (PI)", model: CORNER_INNER, rotation: CORNER_ROTATIONS[2] },
        WallTestEntry { label: "inn WN (PI/2)", model: CORNER_INNER, rotation: CORNER_ROTATIONS[3] },
    ];

    // Row 2: outer corners
    let row2 = vec![
        WallTestEntry { label: "out NE (0)", model: CORNER_OUTER, rotation: CORNER_ROTATIONS[0] },
        WallTestEntry { label: "out ES (-PI/2)", model: CORNER_OUTER, rotation: CORNER_ROTATIONS[1] },
        WallTestEntry { label: "out SW (PI)", model: CORNER_OUTER, rotation: CORNER_ROTATIONS[2] },
        WallTestEntry { label: "out WN (PI/2)", model: CORNER_OUTER, rotation: CORNER_ROTATIONS[3] },
    ];

    // Row 3: raw rotation sweep (straight wall at 0, 90, 180, 270 degrees)
    let row3 = vec![
        WallTestEntry { label: "raw 0°", model: WALL, rotation: 0.0 },
        WallTestEntry { label: "raw 90°", model: WALL, rotation: FRAC_PI_2 },
        WallTestEntry { label: "raw 180°", model: WALL, rotation: PI },
        WallTestEntry { label: "raw 270°", model: WALL, rotation: FRAC_PI_2 * 3.0 },
    ];

    let rows = vec![row0, row1, row2, row3];

    for (row_idx, row) in rows.iter().enumerate() {
        for (col_idx, entry) in row.iter().enumerate() {
            let x = col_idx as f32 * SPACING;
            let z = row_idx as f32 * SPACING;
            let pos = Vec3::new(x, 0.0, z);

            // green floor reference
            commands.spawn((
                Mesh3d(floor_mesh.clone()),
                MeshMaterial3d(green_mat.clone()),
                Transform::from_translation(pos + Vec3::Y * 0.01),
            ));

            // the wall model
            let scene_handle: Handle<Scene> = asset_server
                .load(format!("{}#Scene0", entry.model));
            commands.spawn((
                SceneRoot(scene_handle),
                Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_y(entry.rotation)),
            ));

            // RED arrow: model's local +X after rotation
            let rot = Quat::from_rotation_y(entry.rotation);
            let local_x = rot * Vec3::X;
            let arrow_pos = pos + Vec3::Y * 2.5 + local_x * 0.3;
            commands.spawn((
                Mesh3d(arrow_mesh.clone()),
                MeshMaterial3d(red_mat.clone()),
                Transform::from_translation(arrow_pos)
                    .with_rotation(Quat::from_rotation_y(entry.rotation)),
            ));

            // BLUE arrow: model's local +Z after rotation
            let local_z = rot * Vec3::Z;
            let arrow_pos_z = pos + Vec3::Y * 2.3 + local_z * 0.3;
            commands.spawn((
                Mesh3d(arrow_mesh.clone()),
                MeshMaterial3d(blue_mat.clone()),
                Transform::from_translation(arrow_pos_z)
                    .with_rotation(Quat::from_rotation_y(entry.rotation + FRAC_PI_2)),
            ));

            // YELLOW label marker at the top (small sphere for identification)
            let label_pos = pos + Vec3::Y * 3.0;
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.08))),
                MeshMaterial3d(yellow_mat.clone()),
                Transform::from_translation(label_pos),
            ));

            // print label to console for cross-reference
            println!(
                "  [{},{}] pos=({:.0},{:.0}) rot={:.2}rad ({:.0}°) => {}",
                row_idx, col_idx, x, z,
                entry.rotation,
                entry.rotation.to_degrees(),
                entry.label,
            );
        }
    }

    // print legend
    println!("\n=== Wall Debug Test Bench ===");
    println!("RED arrow  = model local +X direction");
    println!("BLUE arrow = model local +Z direction");
    println!("GREEN tile = floor reference");
    println!("YELLOW dot = label marker (top)");
    println!();
    println!("Row 0: Straight walls at EW/NS constants + rot=0 + rot=PI/2");
    println!("Row 1: Inner corners at each CORNER_ROTATIONS index");
    println!("Row 2: Outer corners at each CORNER_ROTATIONS index");
    println!("Row 3: Raw rotation sweep (0° 90° 180° 270°)");
    println!();
    println!("KEY QUESTION: At rot=0 (row 3, col 0), which direction");
    println!("does the wall model stretch along? That's the model's");
    println!("default axis. Compare with what the code assumes (+Z face).");
    println!();
}

// ── Simple orbit camera ──

fn orbit_camera(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut transform) = camera.single_mut() else { return };

    // right-click orbit
    if mouse.pressed(MouseButton::Right) {
        for ev in motion.read() {
            let yaw = Quat::from_rotation_y(-ev.delta.x * 0.005);
            let pitch = Quat::from_rotation_x(-ev.delta.y * 0.005);

            let pivot = Vec3::new(5.0, 0.0, 4.0);
            let offset = transform.translation - pivot;
            let rotated = yaw * pitch * offset;
            transform.translation = pivot + rotated;
            transform.look_at(pivot, Vec3::Y);
        }
    }

    // scroll zoom
    for ev in scroll.read() {
        let forward = transform.forward().as_vec3();
        transform.translation += forward * ev.y * 0.5;
    }
}
