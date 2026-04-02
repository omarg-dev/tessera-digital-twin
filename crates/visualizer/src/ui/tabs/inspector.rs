//! Inspector tab: context-sensitive inspector routing.
//!
//! Routes to the appropriate sub-inspector based on what is selected:
//! robot entity → robot_inspector, shelf entity → shelf_inspector,
//! task id → task_inspector, nothing → empty-state message.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::grid_map::GridMap;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{ActivePaths, TaskListData, UiAction, UiState};

use super::{robot_inspector, shelf_inspector, task_inspector};

pub const LABEL: &str = "Inspector";

/// Render the appropriate sub-inspector for the current selection.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    task_list: &TaskListData,
    active_paths: &ActivePaths,
    actions: &mut Vec<UiAction>,
) {
    // entity inspector takes priority over task inspector
    if let Some(entity) = ui_state.selected_entity {
        if let Ok((_, robot)) = robots.get(entity) {
            let active_path = active_paths.0.get(&robot.id).map(Vec::as_slice);
            robot_inspector::draw(ui, robot, active_path, actions);
            return;
        }
        if let Ok((_, shelf)) = shelves.get(entity) {
            shelf_inspector::draw(
                ui, entity, shelf, ui_state, shelves, dropoffs, transforms,
                warehouse_map, actions,
            );
            return;
        }
        ui.label(format!("Entity {:?}", entity));
        ui.label("No detailed view for this entity type.");
        return;
    }

    // task inspector
    if let Some(task_id) = ui_state.selected_task_id {
        if let Some(task) = task_list.tasks.iter().find(|t| t.id == task_id) {
            task_inspector::draw(ui, task, ui_state, active_paths, warehouse_map, actions);
        } else {
            ui.label("Task data unavailable (pending sync).");
            ui.weak("The task list is broadcast every ~2 seconds.");
        }
        return;
    }

    ui.label("Select an entity or task to view details.");
}
