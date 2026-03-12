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

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{ActivePaths, LogBuffer, QueueStateData, RobotIndex, TaskListData, UiAction, UiState, WarehouseMap};

/// System that renders all four UI panels each frame.
///
/// Registered on `EguiPrimaryContextPass` so it runs inside the egui frame.
/// UI actions (button clicks) are collected and sent as Bevy events,
/// consumed by the `bridge_ui_commands` system in the next frame.
#[allow(clippy::too_many_arguments)]
pub fn draw_ui(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<UiState>,
    mut log_buffer: ResMut<LogBuffer>,
    mut actions: MessageWriter<UiAction>,
    robot_index: Res<RobotIndex>,
    queue_state: Res<QueueStateData>,
    task_list: Res<TaskListData>,
    active_paths: Res<ActivePaths>,
    robots: Query<(Entity, &Robot)>,
    shelves: Query<(Entity, &Shelf)>,
    dropoffs: Query<(Entity, &Dropoff)>,
    transforms: Query<&Transform>,
    time: Res<Time>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    warehouse_map: Option<Res<WarehouseMap>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    // reset hovered_entity so panels set it fresh each frame
    ui_state.hovered_entity = None;

    let mut pending_actions = Vec::new();

    let wm = warehouse_map.as_deref().map(|wm| &wm.0);

    gui::control_bar(ctx, &mut ui_state, &robot_index, &queue_state, &time, &mut pending_actions);
    gui::object_manager(ctx, &mut ui_state, &robot_index, &robots, &shelves, &queue_state, &dropoffs, &transforms, wm, &task_list, &mut pending_actions);
    gui::inspector(ctx, &mut ui_state, &robots, &shelves, &dropoffs, &transforms, wm, &task_list, &active_paths, &mut pending_actions);
    gui::log_console(ctx, &mut ui_state, &mut log_buffer);
    gui::realtime_overlay(ctx, &ui_state);

    // background-click deselect: checked AFTER panels are drawn so
    // ctx.is_pointer_over_area() reflects all panel regions registered this frame
    let left_click = mouse_input.just_pressed(MouseButton::Left);
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
