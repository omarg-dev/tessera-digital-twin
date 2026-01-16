use bevy::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};

/// Marker component for the main warehouse camera
#[derive(Component)]
pub struct Camera;

/// Camera controller state
#[derive(Component)]
pub struct CameraController {
    pub focus: Vec3,
    pub radius: f32,
    pub pitch: f32,  // vertical angle (radians)
    pub yaw: f32,    // horizontal angle (radians)
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            focus: Vec3::new(12.0, 0.0, 6.0), // Center of ~24x12 layout
            radius: 25.0,
            pitch: 0.8,  // ~45 degrees down
            yaw: 0.0,
        }
    }
}

/// Spawns the warehouse camera with orbit controls
pub fn spawn_camera(mut commands: Commands) {
    let controller = CameraController::default();
    let transform = calculate_camera_transform(&controller);

    commands.spawn((
        Camera3d::default(),
        transform,
        Camera,
        controller,
    ));
}

/// System to handle camera pan and zoom
pub fn camera_controls(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<(&mut CameraController, &mut Transform), With<Camera>>,
) {
    let Ok((mut controller, mut transform)) = query.single_mut() else {
        return;
    };

    let mut changed = false;

    // Orbit: Right mouse button drag
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            controller.yaw -= delta.x * 0.005;
            controller.pitch += delta.y * 0.005;
            // Clamp pitch to avoid flipping
            controller.pitch = controller.pitch.clamp(0.1, 1.5);
            changed = true;
        }
    }

    // Pan: Middle mouse button drag
    if mouse_button.pressed(MouseButton::Middle) {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            // Pan relative to camera orientation
            let right = transform.right();
            let forward = transform.forward();
            // Project forward onto XZ plane for horizontal panning
            let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            
            controller.focus -= right * delta.x * 0.05;
            controller.focus += forward_xz * delta.y * 0.05;
            changed = true;
        }
    }

    // Zoom: Scroll wheel
    if mouse_scroll.delta.y != 0.0 {
        let scroll_amount = match mouse_scroll.unit {
            MouseScrollUnit::Line => mouse_scroll.delta.y * 2.0,
            MouseScrollUnit::Pixel => mouse_scroll.delta.y * 0.1,
        };
        controller.radius -= scroll_amount;
        controller.radius = controller.radius.clamp(5.0, 100.0);
        changed = true;
    }

    if changed {
        *transform = calculate_camera_transform(&controller);
    }
}

/// Calculate camera transform from controller state (orbit around focus point)
fn calculate_camera_transform(controller: &CameraController) -> Transform {
    let x = controller.radius * controller.pitch.cos() * controller.yaw.sin();
    let y = controller.radius * controller.pitch.sin();
    let z = controller.radius * controller.pitch.cos() * controller.yaw.cos();

    let position = controller.focus + Vec3::new(x, y, z);
    Transform::from_translation(position).looking_at(controller.focus, Vec3::Y)
}
