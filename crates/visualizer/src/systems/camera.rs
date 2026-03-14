use bevy::prelude::*;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::post_process::bloom::Bloom;
use bevy::render::view::Hdr;
use bevy_egui::EguiContexts;
use protocol::config::visual::camera;
use protocol::config::visual::outline as outline_cfg;
use protocol::TaskStatus;

use crate::resources::{RobotIndex, TaskListData, UiState};

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
    // clear per-frame input signals unconditionally — must happen before any early return
    // so follow systems always see a clean slate at the start of each frame
    ui_state.camera_scroll_this_frame = false;
    ui_state.camera_pan_this_frame = false;
    ui_state.camera_orbit_this_frame = false;

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

    // Orbit: Right mouse button drag
    // signals orbit so the entity follow system can pause the focus lerp while orbiting
    if mouse_button.pressed(MouseButton::Right) {
        let delta = mouse_motion.delta;
        if delta != Vec2::ZERO {
            controller.yaw -= delta.x * camera::ORBIT_SENSITIVITY;
            controller.pitch += delta.y * camera::ORBIT_SENSITIVITY;
            // Clamp pitch to avoid flipping
            controller.pitch = controller.pitch.clamp(camera::PITCH_MIN, camera::PITCH_MAX);
            changed = true;
            ui_state.camera_orbit_this_frame = true;
        }
    }

    // Pan: Middle mouse button drag — breaks all camera follow
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

            // Break all camera follow modes
            ui_state.camera_following = false;
            ui_state.camera_pan_this_frame = true;
        }
    }

    // Zoom: Scroll wheel — user takes over radius, cancel any zoom-in lerp
    if mouse_scroll.delta.y != 0.0 {
        let scroll_amount = match mouse_scroll.unit {
            MouseScrollUnit::Line => mouse_scroll.delta.y * camera::SCROLL_LINE_SPEED,
            MouseScrollUnit::Pixel => mouse_scroll.delta.y * camera::SCROLL_PIXEL_SPEED,
        };
        controller.radius -= scroll_amount;
        controller.radius = controller.radius.clamp(camera::ZOOM_MIN, camera::ZOOM_MAX);
        changed = true;
        ui_state.camera_scroll_this_frame = true;
    }

    if changed {
        *transform = calculate_camera_transform(&controller);
    }
}

/// System that moves the camera to follow the selected entity each frame.
/// On first selection, smoothly lerps radius down to FOLLOW_ZOOM_RADIUS if the
/// camera is farther away.
/// - Scroll: cancels the zoom-in lerp so the user's position is respected
/// - Orbit (right drag): pauses focus lerp so the user can orbit freely
/// - Pan (middle drag): breaks follow entirely (handled by camera_controls)
pub fn camera_follow_selected(
    ui_state: Res<UiState>,
    target_transforms: Query<&Transform, Without<Camera>>,
    mut camera_query: Query<(&mut CameraController, &mut Transform), With<Camera>>,
    mut last_followed: Local<Option<Entity>>,
    mut zooming_in: Local<bool>,
) {
    if !ui_state.camera_following {
        *last_followed = None;
        *zooming_in = false;
        return;
    }
    let Some(entity) = ui_state.selected_entity else {
        *last_followed = None;
        *zooming_in = false;
        return;
    };
    let Ok(target_transform) = target_transforms.get(entity) else {
        return;
    };
    let Ok((mut controller, mut transform)) = camera_query.single_mut() else {
        return;
    };

    // user scrolled — release zoom-in so their radius is respected
    if ui_state.camera_scroll_this_frame {
        *zooming_in = false;
    }

    // new entity selected: trigger zoom-in if too far away
    if *last_followed != Some(entity) {
        *last_followed = Some(entity);
        if controller.radius > camera::FOLLOW_ZOOM_RADIUS + 1.0 {
            *zooming_in = true;
        }
    }

    // lerp radius toward target — stopped by user scroll
    if *zooming_in {
        if controller.radius > camera::FOLLOW_ZOOM_RADIUS + 0.1 {
            controller.radius = controller.radius.lerp(camera::FOLLOW_ZOOM_RADIUS, camera::FOLLOW_ZOOM_LERP);
        } else {
            controller.radius = camera::FOLLOW_ZOOM_RADIUS;
            *zooming_in = false;
        }
    }

    // lerp focus toward entity — paused during orbit so the user can orbit freely
    // without the camera snapping back every frame
    if !ui_state.camera_orbit_this_frame {
        let target_pos = target_transform.translation;
        controller.focus = controller.focus.lerp(target_pos, camera::FOLLOW_FOCUS_LERP);
    }

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

/// System that moves the camera to follow the selected task's cargo or robot.
///
/// - Pending task: focuses on the pickup grid cell (where the cargo is)
/// - Assigned/InProgress: follows the robot carrying the cargo
/// - Terminal task selected fresh: camera is left alone (already done)
/// - Live task that becomes terminal: camera lerps back to default once
/// - Task deselected after being live: camera lerps back to default
///
/// This system only fires when a task is selected and the entity inspector is
/// not also active — entity follow (`camera_follow_selected`) takes precedence.
pub fn camera_follow_task(
    ui_state: Res<UiState>,
    task_list: Res<TaskListData>,
    robot_index: Res<RobotIndex>,
    target_transforms: Query<&Transform, Without<Camera>>,
    mut camera_query: Query<(&mut CameraController, &mut Transform), With<Camera>>,
    mut prev_task_id: Local<Option<u64>>,
    mut resetting: Local<bool>,
    mut zooming_in: Local<bool>,
    // true while we are actively following this task as a live (non-terminal) task.
    // the camera only lerps back to default when this transitions false -> terminal.
    mut was_live: Local<bool>,
    mut cached_task_idx: Local<Option<usize>>,
    mut cached_task_list_updated: Local<f64>,
) {
    // entity follow has priority — don't fight camera_follow_selected
    if ui_state.selected_entity.is_some() {
        return;
    }

    let Ok((mut controller, mut transform)) = camera_query.single_mut() else {
        return;
    };

    // any user input cancels an in-progress default-view reset
    if *resetting && (ui_state.camera_pan_this_frame || ui_state.camera_scroll_this_frame || ui_state.camera_orbit_this_frame) {
        *resetting = false;
    }

    // task was deselected — return to default only if we were tracking it as live
    if ui_state.selected_task_id.is_none() && prev_task_id.is_some() {
        *prev_task_id = None;
        *zooming_in = false;
        if *was_live {
            *resetting = true;
        }
        *was_live = false;
        *cached_task_idx = None;
    }

    // smoothly return to default view
    if *resetting {
        let def_focus = Vec3::new(camera::DEFAULT_FOCUS.0, camera::DEFAULT_FOCUS.1, camera::DEFAULT_FOCUS.2);
        let focus_delta = (def_focus - controller.focus).length();
        let radius_delta = (camera::DEFAULT_RADIUS - controller.radius).abs();
        let pitch_delta = (camera::DEFAULT_PITCH - controller.pitch).abs();

        if focus_delta < 0.05 && radius_delta < 0.05 && pitch_delta < 0.01 {
            // snap to exact defaults once close enough
            controller.focus = def_focus;
            controller.radius = camera::DEFAULT_RADIUS;
            controller.pitch = camera::DEFAULT_PITCH;
            *transform = calculate_camera_transform(&controller);
            *resetting = false;
        } else {
            controller.focus = controller.focus.lerp(def_focus, camera::DEFAULT_RESET_LERP);
            controller.radius = controller.radius.lerp(camera::DEFAULT_RADIUS, camera::DEFAULT_RESET_LERP);
            controller.pitch = controller.pitch.lerp(camera::DEFAULT_PITCH, camera::DEFAULT_RESET_LERP);
            *transform = calculate_camera_transform(&controller);
        }
        return;
    }

    let Some(task_id) = ui_state.selected_task_id else {
        return;
    };

    if *cached_task_list_updated != task_list.last_updated_secs {
        *cached_task_idx = None;
        *cached_task_list_updated = task_list.last_updated_secs;
    }

    if cached_task_idx
        .map(|idx| idx >= task_list.tasks.len() || task_list.tasks[idx].id != task_id)
        .unwrap_or(true)
    {
        *cached_task_idx = task_list.tasks.iter().position(|t| t.id == task_id);
    }

    let Some(task_idx) = *cached_task_idx else {
        return;
    };
    let task = &task_list.tasks[task_idx];

    let is_terminal = matches!(task.status, TaskStatus::Completed | TaskStatus::Failed { .. } | TaskStatus::Cancelled);

    // new task selected
    if *prev_task_id != Some(task_id) {
        *prev_task_id = Some(task_id);
        *resetting = false;
        *zooming_in = false;
        *was_live = false;
        *cached_task_idx = None;
        // only initiate zoom-in for live tasks — terminal tasks are browsed passively
        if !is_terminal && controller.radius > camera::TASK_FOLLOW_ZOOM_RADIUS + 1.0 {
            *zooming_in = true;
        }
    }

    // terminal task — only reset if we were following it as live (i.e., it just completed)
    if is_terminal {
        if *was_live {
            *resetting = true;
            *zooming_in = false;
            *was_live = false;
        }
        return;
    }

    // actively following a live task
    *was_live = true;

    // user scroll cancels the zoom-in lerp
    if ui_state.camera_scroll_this_frame {
        *zooming_in = false;
    }

    // determine follow target
    let target_pos: Option<Vec3> = match &task.status {
        TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } => {
            robot_index.get_entity(*robot_id)
                .and_then(|entity| target_transforms.get(entity).ok())
                .map(|t| t.translation)
        }
        TaskStatus::Pending => {
            task.pickup_location()
                .map(|(col, row)| Vec3::new(col as f32, 0.0, row as f32))
        }
        // already handled above
        TaskStatus::Completed | TaskStatus::Failed { .. } | TaskStatus::Cancelled => return,
    };

    let Some(target) = target_pos else {
        return;
    };

    // zoom in on first follow
    if *zooming_in {
        if controller.radius > camera::TASK_FOLLOW_ZOOM_RADIUS + 0.1 {
            controller.radius = controller.radius.lerp(camera::TASK_FOLLOW_ZOOM_RADIUS, camera::FOLLOW_ZOOM_LERP);
        } else {
            controller.radius = camera::TASK_FOLLOW_ZOOM_RADIUS;
            *zooming_in = false;
        }
    }

    controller.focus = controller.focus.lerp(target, camera::FOLLOW_FOCUS_LERP);
    *transform = calculate_camera_transform(&controller);
}
