//! UI module -- Digital Twin Command Center
//!
//! Four-panel layout built with `bevy_egui`:
//! - Top: Simulation controls, KPIs, layer toggles
//! - Left: Object Manager (robots & shelves list, task queue)
//! - Right: Inspector (context-sensitive entity details)
//! - Bottom: System logs & analytics

pub mod gui;
pub mod tabs;
pub mod widgets;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{
    ActivePaths, BackpressureMetrics, LogBuffer, QueueStateData, RenderPerfCounters, RobotIndex,
    ScreenshotHarness, TaskListData, UiAction, UiAnalyticsView, UiFrameInputs, UiState, WarehouseMap,
    WhcaMetricsData,
};

/// sync compact per-frame UI inputs used by the egui pass.
pub fn sync_ui_frame_inputs(
    time: Res<Time>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut ui_frame_inputs: ResMut<UiFrameInputs>,
) {
    ui_frame_inputs.delta_secs = time.delta_secs();
    ui_frame_inputs.left_click_just_pressed = mouse_input.just_pressed(MouseButton::Left);
}

/// sync render counters and snapshot markers for the analytics tab.
pub fn sync_ui_analytics_view(
    perf_counters: Res<RenderPerfCounters>,
    screenshot_harness: Res<ScreenshotHarness>,
    time: Res<Time>,
    mut backpressure: ResMut<BackpressureMetrics>,
    mut log_buffer: ResMut<LogBuffer>,
    mut analytics_view: ResMut<UiAnalyticsView>,
) {
    backpressure.refresh_from_handles();
    backpressure.maybe_push_warnings(time.elapsed_secs_f64(), &mut log_buffer);
    analytics_view.perf = perf_counters.clone();
    analytics_view.snapshot_markers = screenshot_harness.records.clone();
    analytics_view.backpressure = backpressure.snapshot();
}

/// System that renders all four UI panels each frame.
///
/// Registered on `EguiPrimaryContextPass` so it runs inside the egui frame.
/// UI actions (button clicks) are collected and sent as Bevy events,
/// consumed by the `bridge_ui_commands` system in the next frame.
#[derive(SystemParam)]
pub struct UiDrawData<'w, 's> {
    robot_index: Res<'w, RobotIndex>,
    queue_state: Res<'w, QueueStateData>,
    whca_metrics: Res<'w, WhcaMetricsData>,
    analytics_view: Res<'w, UiAnalyticsView>,
    task_list: Res<'w, TaskListData>,
    active_paths: Res<'w, ActivePaths>,
    robots: Query<'w, 's, (Entity, &'static Robot)>,
    shelves: Query<'w, 's, (Entity, &'static Shelf)>,
    dropoffs: Query<'w, 's, (Entity, &'static Dropoff)>,
    transforms: Query<'w, 's, &'static Transform>,
    ui_frame_inputs: Res<'w, UiFrameInputs>,
    warehouse_map: Option<Res<'w, WarehouseMap>>,
}

#[allow(clippy::too_many_arguments)]
pub fn draw_ui(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
    mut log_buffer: ResMut<LogBuffer>,
    mut actions: MessageWriter<UiAction>,
    data: UiDrawData,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    // reset hovered_entity so panels set it fresh each frame
    ui_state.hovered_entity = None;

    let mut pending_actions = Vec::new();

    let wm = data.warehouse_map.as_deref().map(|wm| &wm.0);

    gui::control_bar(
        ctx,
        &mut ui_state,
        &data.robot_index,
        &data.queue_state,
        data.ui_frame_inputs.delta_secs,
        &mut pending_actions,
    );
    gui::object_manager(ctx, &mut ui_state, &data.robot_index, &data.robots, &data.shelves, &data.queue_state, &data.dropoffs, &data.transforms, wm, &data.task_list, &mut pending_actions);
    gui::inspector(ctx, &mut ui_state, &data.robots, &data.shelves, &data.dropoffs, &data.transforms, wm, &data.task_list, &data.active_paths, &mut pending_actions);
    gui::log_console(
        ctx,
        &mut ui_state,
        &mut log_buffer,
        &data.whca_metrics,
        &data.analytics_view,
    );
    gui::realtime_overlay(ctx, &ui_state);

    // background-click deselect: checked AFTER panels are drawn so
    // ctx.is_pointer_over_area() reflects all panel regions registered this frame
    let left_click = data.ui_frame_inputs.left_click_just_pressed;
    let anything_selected = ui_state.selected_entity.is_some() || ui_state.selected_task_id.is_some();
    if left_click
        && !ui_state.entity_picked_this_frame
        && !ctx.is_pointer_over_area()
        && anything_selected
    {
        if let Some(prev) = ui_state.selected_entity {
            ui_state.hidden_labels.remove(&prev);
        }
        ui_state.selected_entity = None;
        ui_state.selected_task_id = None;
        ui_state.camera_following = false;
        ui_state.transport_dropdown_open = false;
    }
    // consume the flag after reading it
    ui_state.entity_picked_this_frame = false;

    for action in pending_actions {
        actions.write(action);
    }

    Ok(())
}
