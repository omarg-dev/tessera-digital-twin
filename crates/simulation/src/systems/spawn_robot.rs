use bevy::prelude::*;
use crate::components::Robot;

/// Startup system that spawns the initial robot(s)
pub fn spawn_robot(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Spawn a robot at the origin
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.7, 0.9),
            ..default()
        })),
        Robot {id: 0, state: protocol::RobotState::Idle, position: Vec3::ZERO},
    ));
}
