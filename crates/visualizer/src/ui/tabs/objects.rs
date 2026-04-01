//! Objects tab: collapsible robot and shelf lists with search.

use bevy::prelude::*;
use bevy_egui::egui;

use crate::components::{Robot, Shelf};
use crate::resources::{RobotIndex, UiState};

pub const LABEL: &str = "Entities";

/// Collapsible robot and shelf lists with search filter.
pub fn draw(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
) {
    refresh_object_caches(ui_state, robots, shelves);

    // Search
    ui.horizontal(|ui| {
        ui.label("🔍");
        egui::TextEdit::singleline(&mut ui_state.filter_query)
            .hint_text("Search...")
            .desired_width(ui.available_width())
            .show(ui);
    });

    ui.separator();

    let filter = ui_state.filter_query.to_lowercase();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── Robots section (collapsible) ──
            let robot_count = robot_index.by_id.len();
            egui::CollapsingHeader::new(
                egui::RichText::new(format!("Robots ({robot_count})")).strong(),
            )
            .default_open(true)
            .show(ui, |ui| {
                let mut stale_cache = false;
                let cached_robots = ui_state.object_sorted_robot_entities.clone();
                for entity in cached_robots {
                    let Ok((entity, robot)) = robots.get(entity) else {
                        stale_cache = true;
                        continue;
                    };
                    let label = format!("#{}", robot.id);
                    if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                        continue;
                    }

                    let state_icon = state_icon(robot);
                    let text = format!("{state_icon} Robot {label}  {:?}", robot.state);

                    let is_selected = ui_state.selected_entity == Some(entity);
                    let response = ui.selectable_label(is_selected, text);
                    if response.hovered() {
                        ui_state.hovered_entity = Some(entity);
                    }
                    if response.clicked() {
                        select_entity(ui_state, entity);
                    }
                }

                if stale_cache {
                    ui_state.object_cache_robot_count = usize::MAX;
                }
            });

            ui.add_space(4.0);

            // ── Shelves section (collapsible) ──
            let shelf_count = shelves.iter().count();
            egui::CollapsingHeader::new(
                egui::RichText::new(format!("Shelves ({shelf_count})")).strong(),
            )
            .default_open(true)
            .show(ui, |ui| {
                let mut stale_cache = false;
                let cached_shelves = ui_state.object_sorted_shelf_entities.clone();
                for entity in cached_shelves {
                    let Ok((entity, shelf)) = shelves.get(entity) else {
                        stale_cache = true;
                        continue;
                    };
                    let label = format!("Shelf ({}/{})", shelf.cargo, shelf.max_capacity);
                    if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                        continue;
                    }

                    let is_selected = ui_state.selected_entity == Some(entity);
                    let response = ui.selectable_label(is_selected, &label);
                    if response.hovered() {
                        ui_state.hovered_entity = Some(entity);
                    }
                    if response.clicked() {
                        select_entity(ui_state, entity);
                    }
                }

                if stale_cache {
                    ui_state.object_cache_shelf_count = usize::MAX;
                }
            });
        });
}

fn refresh_object_caches(
    ui_state: &mut UiState,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
) {
    let robot_count = robots.iter().count();
    if ui_state.object_cache_robot_count != robot_count {
        let mut sorted: Vec<(u32, Entity)> = robots
            .iter()
            .map(|(entity, robot)| (robot.id, entity))
            .collect();
        sorted.sort_unstable_by_key(|(id, _)| *id);
        ui_state.object_sorted_robot_entities = sorted.into_iter().map(|(_, entity)| entity).collect();
        ui_state.object_cache_robot_count = robot_count;
    }

    let shelf_count = shelves.iter().count();
    if ui_state.object_cache_shelf_count != shelf_count {
        let mut sorted: Vec<Entity> = shelves.iter().map(|(entity, _)| entity).collect();
        sorted.sort_unstable_by_key(|entity| entity.index());
        ui_state.object_sorted_shelf_entities = sorted;
        ui_state.object_cache_shelf_count = shelf_count;
    }
}

/// Tiny state-indicator icon for the robot list.
fn state_icon(robot: &Robot) -> &'static str {
    use protocol::RobotState::*;
    match robot.state {
        Idle => "●",           // ●  idle
        Charging => "⚡",       // ⚡ charging
        MovingToPickup | MovingToDrop | MovingToStation => "▶", // ▶ moving
        Picking => "⬆",        // ⬆ picking
        LowBattery => "🔋",    // 🔋 low battery
        Blocked => "⛔",        // ⛔ blocked
        Faulted => "⚠",        // ⚠ faulted
    }
}

/// Select an entity: sets selection, enables camera follow, resets transport dropdown.
fn select_entity(ui_state: &mut UiState, entity: Entity) {
    ui_state.selected_entity = Some(entity);
    ui_state.camera_following = true;
    ui_state.transport_dropdown_open = false;
    ui_state.transport_shelves_expanded = false;
}
