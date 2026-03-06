//! Wall orientation test bench
//!
//! Visual verification that rotation constants produce correct wall
//! orientations for each neighborhood configuration.
//!
//! Each cell shows:
//!   - The wall .glb model at the classified rotation
//!   - Cyan cubes marking expected neighbor positions
//!   - An on-screen egui label with wall type and rotation
//!
//! A compass gizmo on the left shows N/S/E/W for reference.
//! Row 3 ("raw sweep") has no neighbors - use it to determine the
//! model's default axis at rotation = 0.
//!
//! Run: cargo run -p visualizer --example wall_debug
//!
//! Controls: right-click = orbit, scroll = zoom, middle-click = pan

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass, egui};
use std::f32::consts::{FRAC_PI_2, PI};

fn main() {
    // resolve workspace-root assets/ regardless of working directory
    let assets_dir = std::env::current_exe()
        .ok()
        .and_then(|p| {
            let mut dir = p.parent()?.to_path_buf();
            for _ in 0..10 {
                if dir.join("assets").is_dir() {
                    return Some(dir.join("assets").to_string_lossy().into_owned());
                }
                dir = dir.parent()?.to_path_buf();
            }
            None
        })
        .unwrap_or_else(|| "assets".to_string());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Wall Orientation Test Bench".to_string(),
                resolution: (1400, 900).into(),
                ..default()
            }),
            ..default()
        }).set(AssetPlugin {
            file_path: assets_dir,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .init_resource::<CellLabels>()
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (orbit_camera, draw_compass_gizmo))
        .add_systems(EguiPrimaryContextPass, draw_labels)
        .run();
}

// ── Rotation constants (must match models.rs) ──

const STRAIGHT_EW: f32 = PI;
const STRAIGHT_NS: f32 = FRAC_PI_2 + PI;

const CORNER_ROTATIONS: [f32; 4] = [
    FRAC_PI_2,  // N+E
    0.0,        // E+S
    -FRAC_PI_2, // S+W
    PI,         // W+N
];

const T_ROTATIONS: [f32; 4] = [
    0.0,        // missing N
    -FRAC_PI_2, // missing W
    PI,         // missing S
    FRAC_PI_2,  // missing E
];

const CAP_ROTATIONS: [f32; 4] = [
    0.0,        // only N
    FRAC_PI_2,  // only E
    PI,         // only S
    -FRAC_PI_2, // only W
];

// ── Asset paths (must match models.rs) ──

const WALL: &str = "models/wall.glb";
const CORNER: &str = "models/wall-corner.glb";
const T_JUNCTION: &str = "models/wall-T.glb";
const CAP: &str = "models/wall-cap.glb";
const PILLAR: &str = "models/wall-pillar.glb";
const FLOOR: &str = "models/floor.glb";

const SPACING: f32 = 4.0;
const NEIGHBOR_DIST: f32 = 1.0;

// ── Label registry ──

#[derive(Resource, Default)]
struct CellLabels {
    entries: Vec<(Vec3, String)>,
}

// ── Neighbor offsets (Bevy coords: +X = east, +Z = south) ──

const DIR_N: Vec3  = Vec3::new(0.0, 0.0, -NEIGHBOR_DIST);
const DIR_S: Vec3  = Vec3::new(0.0, 0.0,  NEIGHBOR_DIST);
const DIR_E: Vec3  = Vec3::new(NEIGHBOR_DIST, 0.0, 0.0);
const DIR_W: Vec3  = Vec3::new(-NEIGHBOR_DIST, 0.0, 0.0);

// ── Scene setup ──

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut labels: ResMut<CellLabels>,
) {
    // camera (angled top-down)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(6.0, 20.0, 16.0)
            .looking_at(Vec3::new(6.0, 0.0, 5.0), Vec3::Y),
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

    // neighbor marker (cyan cube)
    let neighbor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 1.0),
        unlit: true,
        ..default()
    });
    let neighbor_mesh = meshes.add(Cuboid::new(0.25, 0.5, 0.25));

    // ── Test cases ──

    struct Case {
        label: String,
        model: &'static str,
        rotation: f32,
        neighbors: Vec<Vec3>,
    }

    let rows: Vec<Vec<Case>> = vec![
        // row 0: straight walls
        vec![
            Case {
                label: "Straight EW\nrot = PI".into(),
                model: WALL,
                rotation: STRAIGHT_EW,
                neighbors: vec![DIR_E, DIR_W],
            },
            Case {
                label: "Straight NS\nrot = 3PI/2".into(),
                model: WALL,
                rotation: STRAIGHT_NS,
                neighbors: vec![DIR_N, DIR_S],
            },
        ],
        // row 1: corners (bidirectional model, 4 rotations)
        vec![
            Case {
                label: "Corner NE\nrot = PI/2".into(),
                model: CORNER,
                rotation: CORNER_ROTATIONS[0],
                neighbors: vec![DIR_N, DIR_E],
            },
            Case {
                label: "Corner ES\nrot = 0".into(),
                model: CORNER,
                rotation: CORNER_ROTATIONS[1],
                neighbors: vec![DIR_E, DIR_S],
            },
            Case {
                label: "Corner SW\nrot = -PI/2".into(),
                model: CORNER,
                rotation: CORNER_ROTATIONS[2],
                neighbors: vec![DIR_S, DIR_W],
            },
            Case {
                label: "Corner WN\nrot = PI".into(),
                model: CORNER,
                rotation: CORNER_ROTATIONS[3],
                neighbors: vec![DIR_W, DIR_N],
            },
        ],
        // row 2: T-junctions (3 neighbors, indexed by missing direction)
        vec![
            Case {
                label: "T miss N\nrot = 0".into(),
                model: T_JUNCTION,
                rotation: T_ROTATIONS[0],
                neighbors: vec![DIR_E, DIR_S, DIR_W],
            },
            Case {
                label: "T miss E\nrot = PI/2".into(),
                model: T_JUNCTION,
                rotation: T_ROTATIONS[1],
                neighbors: vec![DIR_N, DIR_S, DIR_W],
            },
            Case {
                label: "T miss S\nrot = PI".into(),
                model: T_JUNCTION,
                rotation: T_ROTATIONS[2],
                neighbors: vec![DIR_N, DIR_E, DIR_W],
            },
            Case {
                label: "T miss W\nrot = -PI/2".into(),
                model: T_JUNCTION,
                rotation: T_ROTATIONS[3],
                neighbors: vec![DIR_N, DIR_E, DIR_S],
            },
        ],
        // row 3: pillar (isolated wall, no neighbors)
        vec![
            Case {
                label: "Pillar\nrot = 0".into(),
                model: PILLAR,
                rotation: 0.0,
                neighbors: vec![],
            },
        ],
        // row 4: endcaps (exactly one neighbor, indexed by direction)
        vec![
            Case {
                label: "Cap only N\nrot = 0".into(),
                model: CAP,
                rotation: CAP_ROTATIONS[0],
                neighbors: vec![DIR_N],
            },
            Case {
                label: "Cap only E\nrot = PI/2".into(),
                model: CAP,
                rotation: CAP_ROTATIONS[1],
                neighbors: vec![DIR_E],
            },
            Case {
                label: "Cap only S\nrot = PI".into(),
                model: CAP,
                rotation: CAP_ROTATIONS[2],
                neighbors: vec![DIR_S],
            },
            Case {
                label: "Cap only W\nrot = -PI/2".into(),
                model: CAP,
                rotation: CAP_ROTATIONS[3],
                neighbors: vec![DIR_W],
            },
        ],
        // row 5: raw rotation sweep (no neighbors, determine model default axis)
        vec![
            Case { label: "Raw 0 deg".into(), model: WALL, rotation: 0.0, neighbors: vec![] },
            Case { label: "Raw 90 deg".into(), model: WALL, rotation: FRAC_PI_2, neighbors: vec![] },
            Case { label: "Raw 180 deg".into(), model: WALL, rotation: PI, neighbors: vec![] },
            Case { label: "Raw 270 deg".into(), model: WALL, rotation: FRAC_PI_2 * 3.0, neighbors: vec![] },
        ],
    ];

    for (row_idx, row) in rows.iter().enumerate() {
        for (col_idx, case) in row.iter().enumerate() {
            let center = Vec3::new(
                col_idx as f32 * SPACING,
                0.0,
                row_idx as f32 * SPACING,
            );

            // floor tile
            commands.spawn((
                SceneRoot(asset_server.load(format!("{FLOOR}#Scene0"))),
                Transform::from_translation(center),
            ));

            // wall model
            commands.spawn((
                SceneRoot(asset_server.load(format!("{}#Scene0", case.model))),
                Transform::from_translation(center)
                    .with_rotation(Quat::from_rotation_y(case.rotation)),
            ));

            // neighbor markers (cyan cubes)
            for &offset in &case.neighbors {
                commands.spawn((
                    Mesh3d(neighbor_mesh.clone()),
                    MeshMaterial3d(neighbor_mat.clone()),
                    Transform::from_translation(center + offset + Vec3::Y * 0.25),
                ));
            }

            // register label for egui overlay
            labels.entries.push((center + Vec3::Y * 2.5, case.label.clone()));
        }
    }
}

// ── Compass gizmo (drawn every frame) ──

fn draw_compass_gizmo(mut gizmos: Gizmos) {
    let o = Vec3::new(-2.5, 0.1, 5.0);
    let len = 1.2;

    gizmos.arrow(o, o + Vec3::new(0.0, 0.0, -len), Color::srgb(1.0, 0.2, 0.2)); // N = -Z
    gizmos.arrow(o, o + Vec3::new(0.0, 0.0, len),  Color::srgb(1.0, 0.6, 0.1)); // S = +Z
    gizmos.arrow(o, o + Vec3::new(len, 0.0, 0.0),  Color::srgb(0.2, 1.0, 0.2)); // E = +X
    gizmos.arrow(o, o + Vec3::new(-len, 0.0, 0.0), Color::srgb(0.3, 0.5, 1.0)); // W = -X
}

// ── Egui label overlay (3D positions projected to screen) ──

fn draw_labels(
    mut contexts: bevy_egui::EguiContexts,
    labels: Res<CellLabels>,
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
) {
    let Ok((camera, camera_gt)) = camera_q.single() else { return };
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // compass direction labels
    let compass_o = Vec3::new(-2.5, 0.1, 5.0);
    for (offset, text) in [
        (Vec3::new(0.0, 0.0, -1.8), "N"),
        (Vec3::new(0.0, 0.0,  1.8), "S"),
        (Vec3::new(1.8, 0.0,  0.0), "E"),
        (Vec3::new(-1.8, 0.0, 0.0), "W"),
    ] {
        if let Ok(sp) = camera.world_to_viewport(camera_gt, compass_o + offset) {
            egui::Area::new(egui::Id::new(format!("compass_{text}")))
                .fixed_pos(egui::pos2(sp.x - 6.0, sp.y - 10.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .color(egui::Color32::WHITE)
                            .size(16.0)
                            .strong(),
                    );
                });
        }
    }

    // row labels (left side)
    for (row_idx, text) in [
        (0, "Straight"),
        (1, "Corner"),
        (2, "T-Junction"),
        (3, "Pillar"),
        (4, "Cap"),
        (5, "Raw Sweep"),
    ] {
        let world = Vec3::new(-1.5, 0.5, row_idx as f32 * SPACING);
        if let Ok(sp) = camera.world_to_viewport(camera_gt, world) {
            egui::Area::new(egui::Id::new(format!("row_{row_idx}")))
                .fixed_pos(egui::pos2(sp.x - 50.0, sp.y - 8.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .color(egui::Color32::from_rgb(180, 180, 255))
                            .size(14.0)
                            .strong(),
                    );
                });
        }
    }

    // cell labels (above each wall)
    for (i, (world_pos, text)) in labels.entries.iter().enumerate() {
        if let Ok(sp) = camera.world_to_viewport(camera_gt, *world_pos) {
            egui::Area::new(egui::Id::new(format!("cell_{i}")))
                .fixed_pos(egui::pos2(sp.x - 40.0, sp.y - 16.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(text)
                            .color(egui::Color32::YELLOW)
                            .size(12.0)
                            .strong(),
                    );
                });
        }
    }
}

// ── Orbit camera ──

fn orbit_camera(
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut scroll: MessageReader<bevy::input::mouse::MouseWheel>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut tf) = camera.single_mut() else { return };
    let pivot = Vec3::new(6.0, 0.0, 5.0);

    // right-click orbit
    if mouse.pressed(MouseButton::Right) {
        for ev in motion.read() {
            let yaw = Quat::from_rotation_y(-ev.delta.x * 0.005);
            let pitch = Quat::from_rotation_x(-ev.delta.y * 0.005);
            let offset = tf.translation - pivot;
            tf.translation = pivot + yaw * pitch * offset;
            tf.look_at(pivot, Vec3::Y);
        }
    }

    // middle-click pan
    if mouse.pressed(MouseButton::Middle) {
        for ev in motion.read() {
            let right = tf.right().as_vec3();
            let up = tf.up().as_vec3();
            tf.translation += (-right * ev.delta.x + up * ev.delta.y) * 0.03;
        }
    }

    // scroll zoom
    for ev in scroll.read() {
        let forward = tf.forward().as_vec3();
        tf.translation += forward * ev.y * 0.8;
    }
}
