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
    // Search
    ui.horizontal(|ui| {
        ui.label("\u{1F50D}"); // 🔍
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
                let mut robot_list: Vec<_> = robots.iter().collect();
                robot_list.sort_unstable_by_key(|(_, r)| r.id);

                for (entity, robot) in &robot_list {
                    let label = format!("#{}", robot.id);
                    if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                        continue;
                    }

                    let state_icon = state_icon(robot);
                    let text = format!("{state_icon} Robot {label}  {:?}", robot.state);

                    let is_selected = ui_state.selected_entity == Some(*entity);
                    let response = ui.selectable_label(is_selected, text);
                    if response.hovered() {
                        ui_state.hovered_entity = Some(*entity);
                    }
                    if response.clicked() {
                        select_entity(ui_state, *entity);
                    }
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
                let mut shelf_list: Vec<_> = shelves.iter().collect();
                shelf_list.sort_unstable_by_key(|(e, _)| e.index());

                for (entity, shelf) in &shelf_list {
                    let label = format!("Shelf ({}/{})", shelf.cargo, shelf.max_capacity);
                    if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                        continue;
                    }

                    let is_selected = ui_state.selected_entity == Some(*entity);
                    let response = ui.selectable_label(is_selected, &label);
                    if response.hovered() {
                        ui_state.hovered_entity = Some(*entity);
                    }
                    if response.clicked() {
                        select_entity(ui_state, *entity);
                    }
                }
            });
        });
}

/// Tiny state-indicator icon for the robot list.
fn state_icon(robot: &Robot) -> &'static str {
    use protocol::RobotState::*;
    match robot.state {
        Idle => "\u{25CF}",           // ●  idle
        Charging => "\u{26A1}",       // ⚡ charging
        MovingToPickup | MovingToDrop | MovingToStation => "\u{25B6}", // ▶ moving
        Picking => "\u{2B06}",        // ⬆ picking
        LowBattery => "\u{1F50B}",    // 🔋 low battery
        Blocked => "\u{26D4}",        // ⛔ blocked
        Faulted => "\u{26A0}",        // ⚠ faulted
    }
}

/// Select an entity: sets selection, enables camera follow, resets transport dropdown.
fn select_entity(ui_state: &mut UiState, entity: Entity) {
    ui_state.selected_entity = Some(entity);
    ui_state.camera_following = true;
    ui_state.transport_dropdown_open = false;
    ui_state.transport_shelves_expanded = false;
}
