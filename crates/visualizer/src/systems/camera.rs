use bevy::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::post_process::bloom::Bloom;
use bevy::render::view::Hdr;
use bevy_egui::EguiContexts;
use protocol::config::visual::camera;
use protocol::config::visual::outline as outline_cfg;

use crate::resources::UiState;

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
            focus: Vec3::new(camera::DEFAULT_FOCUS.0, camera::DEFAULT_FOCUS.1, camera::DEFAULT_FOCUS.2),
            radius: camera::DEFAULT_RADIUS,
            pitch: camera::DEFAULT_PITCH,
            yaw: camera::DEFAULT_YAW,
        }
    }
}


/// Spawns the warehouse camera with orbit controls, HDR, and bloom
pub fn spawn_camera(mut commands: Commands) {
    let controller = CameraController::default();
    let transform = calculate_camera_transform(&controller);

    commands.spawn((
        Camera3d::default(),
        transform,
        Hdr,
        Bloom {
            intensity: outline_cfg::BLOOM_INTENSITY,
            ..default()
        },
        Camera,
        controller,
    ));
}

/// System to handle camera pan and zoom.
/// Skips input when egui is capturing the pointer (panels, widgets).
/// Panning (middle mouse) breaks camera follow mode.
pub fn camera_controls(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<(&mut CameraController, &mut Transform), With<Camera>>,
    mut egui_ctx: EguiContexts,
    mut ui_state: ResMut<UiState>,
) {
    // Don't orbit/pan/zoom if the cursor is over an egui panel
    if let Ok(ctx) = egui_ctx.ctx_mut() {
        if ctx.wants_pointer_input() || ctx.is_pointer_over_area() {
            return;
        }
    }
    let Ok((mut controller, mut transform)) = query.single_mut() else {
        return;
    };

    let mut changed = false;

    // Orbit: Right mouse button drag (does NOT break follow)
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            controller.yaw -= delta.x * camera::ORBIT_SENSITIVITY;
            controller.pitch += delta.y * camera::ORBIT_SENSITIVITY;
            // Clamp pitch to avoid flipping
            controller.pitch = controller.pitch.clamp(camera::PITCH_MIN, camera::PITCH_MAX);
            changed = true;
        }
    }

    // Pan: Middle mouse button drag (breaks follow)
    if mouse_button.pressed(MouseButton::Middle) {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            // Pan relative to camera orientation
            let right = transform.right();
            let forward = transform.forward();
            // Project forward onto XZ plane for horizontal panning
            let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();

            controller.focus -= right * delta.x * camera::PAN_SENSITIVITY;
            controller.focus += forward_xz * delta.y * camera::PAN_SENSITIVITY;
            changed = true;

            // Break camera follow on manual pan
            ui_state.camera_following = false;
        }
    }

    // Zoom: Scroll wheel
    if mouse_scroll.delta.y != 0.0 {
        let scroll_amount = match mouse_scroll.unit {
            MouseScrollUnit::Line => mouse_scroll.delta.y * camera::SCROLL_LINE_SPEED,
            MouseScrollUnit::Pixel => mouse_scroll.delta.y * camera::SCROLL_PIXEL_SPEED,
        };
        controller.radius -= scroll_amount;
        controller.radius = controller.radius.clamp(camera::ZOOM_MIN, camera::ZOOM_MAX);
        changed = true;
    }

    if changed {
        *transform = calculate_camera_transform(&controller);
    }
}

/// System that moves the camera to follow the selected entity each frame.
/// When first acquiring a target, zooms in to a comfortable radius.
pub fn camera_follow_selected(
    ui_state: Res<UiState>,
    target_transforms: Query<&Transform, Without<Camera>>,
    mut camera_query: Query<(&mut CameraController, &mut Transform), With<Camera>>,
) {
    if !ui_state.camera_following {
        return;
    }
    let Some(entity) = ui_state.selected_entity else {
        return;
    };
    let Ok(target_transform) = target_transforms.get(entity) else {
        return;
    };
    let Ok((mut controller, mut transform)) = camera_query.single_mut() else {
        return;
    };

    let target_pos = target_transform.translation;

    // Smoothly zoom in if far away
    if controller.radius > camera::FOLLOW_ZOOM_RADIUS + 1.0 {
        controller.radius = controller.radius.lerp(camera::FOLLOW_ZOOM_RADIUS, camera::FOLLOW_ZOOM_LERP);
    }

    // Move focus to entity position (smooth lerp)
    controller.focus = controller.focus.lerp(target_pos, camera::FOLLOW_FOCUS_LERP);

    *transform = calculate_camera_transform(&controller);
}

/// Calculate camera transform from controller state (orbit around focus point)
fn calculate_camera_transform(controller: &CameraController) -> Transform {
    let x = controller.radius * controller.pitch.cos() * controller.yaw.sin();
    let y = controller.radius * controller.pitch.sin();
    let z = controller.radius * controller.pitch.cos() * controller.yaw.cos();

    let position = controller.focus + Vec3::new(x, y, z);
    Transform::from_translation(position).looking_at(controller.focus, Vec3::Y)
}
