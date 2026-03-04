//! UI module – Digital Twin Command Center
//!
//! Four-panel layout built with `bevy_egui`:
//! - **Top:** Simulation controls, KPIs, layer toggles
//! - **Left:** Object Manager (robots & shelves list, task queue)
//! - **Right:** Inspector (context-sensitive entity details)
//! - **Bottom:** System logs & analytics

pub mod panels;

use bevy::prelude::*;
use bevy_egui::EguiContexts;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{LogBuffer, QueueStateData, RobotIndex, UiAction, UiState};

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
    robots: Query<(Entity, &Robot)>,
    shelves: Query<(Entity, &Shelf)>,
    dropoffs: Query<(Entity, &Dropoff)>,
    transforms: Query<&Transform>,
    time: Res<Time>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    let mut pending_actions = Vec::new();

    panels::top_panel(ctx, &mut ui_state, &robot_index, &queue_state, &time, &mut pending_actions);
    panels::left_panel(ctx, &mut ui_state, &robot_index, &robots, &shelves, &queue_state);
    panels::right_panel(ctx, &mut ui_state, &robots, &shelves, &dropoffs, &transforms, &mut pending_actions);
    panels::bottom_panel(ctx, &mut ui_state, &mut log_buffer);

    for action in pending_actions {
        actions.write(action);
    }

    Ok(())
}
