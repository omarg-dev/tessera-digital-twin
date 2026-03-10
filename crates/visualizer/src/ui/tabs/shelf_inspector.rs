//! Shelf inspector panel: cargo display and transport task creation.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::visual::TILE_SIZE;
use protocol::grid_map::GridMap;
use protocol::{Priority, TaskRequest, TaskType};
use std::collections::HashMap;

use crate::components::{Dropoff, Shelf};
use crate::resources::{UiAction, UiState};
use crate::ui::widgets::{color_swatch, shelf_fill_color_egui, shelf_minimap_widget};

/// Inspector for a shelf: cargo display and transport task creation.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    shelf_entity: Entity,
    shelf: &Shelf,
    ui_state: &mut UiState,
    all_shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    let shelf_pos = transforms.get(shelf_entity).ok();
    let pos_label = shelf_pos
        .map(|t| format!("({:.0}, {:.0})", t.translation.x, t.translation.z))
        .unwrap_or_else(|| "??".into());

    ui.label(
        egui::RichText::new(format!("Shelf @ {pos_label}"))
            .heading()
            .strong(),
    );

    ui.add_space(8.0);

    // Cargo on shelf / max capacity
    ui.horizontal(|ui| {
        ui.label("Cargo:");
        ui.label(
            egui::RichText::new(format!(
                "{} / {}",
                shelf.cargo,
                shelf.max_capacity
            ))
            .strong(),
        );
    });

    // Cargo bar
    let cargo_frac = if shelf.max_capacity == 0 {
        0.0
    } else {
        shelf.cargo as f32 / shelf.max_capacity as f32
    };
    let bar = egui::ProgressBar::new(cargo_frac)
        .desired_width(ui.available_width())
        .fill(shelf_fill_color_egui(shelf.cargo, shelf.max_capacity));
    ui.add(bar);

    // Position
    if let Some(t) = shelf_pos {
        ui.horizontal(|ui| {
            ui.label("Position:");
            ui.label(format!(
                "[{:.1}, {:.1}, {:.1}]",
                t.translation.x, t.translation.y, t.translation.z
            ));
        });
    }

    ui.add_space(12.0);
    ui.separator();
    ui.label(egui::RichText::new("Actions").strong());
    ui.add_space(4.0);

    // ── Transport Task Picker ──
    if !ui_state.transport_dropdown_open {
        if ui.button("Add Transport Task").clicked() {
            ui_state.transport_dropdown_open = true;
        }
    } else {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new("Transport to:").strong());
            ui.add_space(4.0);

            // Option 1: Dropoff
            let has_dropoff = !dropoffs.is_empty();
            if ui.add_enabled(has_dropoff, egui::Button::new("\u{1F4E6} Dropoff zone")).clicked() {
                if let (Some(from_t), Some((_, drop_t))) = (
                    shelf_pos,
                    dropoffs.iter().next().and_then(|(e, _)| {
                        transforms.get(e).ok().map(|t| (e, t))
                    }),
                ) {
                    let pickup = (
                        from_t.translation.x.round() as usize,
                        from_t.translation.z.round() as usize,
                    );
                    let dropoff = (
                        drop_t.translation.x.round() as usize,
                        drop_t.translation.z.round() as usize,
                    );
                    actions.push(UiAction::SubmitTransportTask(TaskRequest {
                        task_type: TaskType::PickAndDeliver {
                            pickup,
                            dropoff,
                            cargo_id: None,
                        },
                        priority: Priority::Normal,
                    }));
                    ui_state.transport_dropdown_open = false;
                }
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Option 2: Shelf picker (mini-map + scrollable list)
            ui.label(egui::RichText::new("Relocate to shelf:").small().strong());
            ui.add_space(2.0);

            // build entity map: grid pos -> (entity, cargo, max)
            let mut entity_map: HashMap<(usize, usize), (Entity, u32, u32)> = HashMap::new();
            for (e, dest_shelf) in all_shelves.iter() {
                if let Ok(t) = transforms.get(e) {
                    let gx = (t.translation.x / TILE_SIZE).round() as usize;
                    let gy = (t.translation.z / TILE_SIZE).round() as usize;
                    entity_map.insert((gx, gy), (e, dest_shelf.cargo, dest_shelf.max_capacity));
                }
            }

            let source_grid = shelf_pos.map(|t| {
                let gx = (t.translation.x / TILE_SIZE).round() as usize;
                let gy = (t.translation.z / TILE_SIZE).round() as usize;
                (gx, gy)
            });

            // ── Mini-map ──
            if let Some(grid) = warehouse_map {
                shelf_minimap_widget(
                    ui, grid, &entity_map, source_grid, shelf_pos, transforms,
                    ui_state, actions,
                );
                ui.add_space(4.0);
            }

            // legend
            ui.horizontal(|ui| {
                color_swatch(ui, egui::Color32::from_rgb(30, 160, 50));
                ui.label(egui::RichText::new("empty").small());
                color_swatch(ui, egui::Color32::from_rgb(200, 160, 30));
                ui.label(egui::RichText::new("half").small());
                color_swatch(ui, egui::Color32::from_rgb(210, 50, 30));
                ui.label(egui::RichText::new("full").small());
                color_swatch(ui, egui::Color32::from_gray(90));
                ui.label(egui::RichText::new("src").small());
            });
            ui.add_space(4.0);

            // ── Scrollable list ──
            egui::ScrollArea::vertical()
                .id_salt("shelf_picker_list")
                .max_height(80.0)
                .show(ui, |ui| {
                    // collect owned tuples so patterns are simple
                    let mut sorted_shelves: Vec<((usize, usize), (Entity, u32, u32))> =
                        entity_map.iter().map(|(&k, &v)| (k, v)).collect();
                    sorted_shelves.sort_by_key(|&((gx, gy), _)| (gy, gx));

                    for ((gx, gy), (dest_entity, cargo, max)) in &sorted_shelves {
                        let (gx, gy) = (*gx, *gy);
                        let (dest_entity, cargo, max) = (*dest_entity, *cargo, *max);
                        if Some((gx, gy)) == source_grid {
                            continue;
                        }
                        let cell_color = shelf_fill_color_egui(cargo, max);
                        let label = format!("({gx},{gy})  {cargo}/{max}");
                        let btn = egui::Button::new(
                            egui::RichText::new(&label).color(egui::Color32::WHITE).small(),
                        )
                        .fill(cell_color)
                        .min_size(egui::Vec2::new(ui.available_width(), 0.0));

                        let resp = ui.add(btn);
                        if resp.hovered() {
                            ui_state.hovered_entity = Some(dest_entity);
                        }
                        if resp.clicked() {
                            if let (Some(from_t), Ok(to_t)) = (shelf_pos, transforms.get(dest_entity)) {
                                let from = (
                                    from_t.translation.x.round() as usize,
                                    from_t.translation.z.round() as usize,
                                );
                                let to = (
                                    to_t.translation.x.round() as usize,
                                    to_t.translation.z.round() as usize,
                                );
                                actions.push(UiAction::SubmitTransportTask(TaskRequest {
                                    task_type: TaskType::Relocate { from, to },
                                    priority: Priority::Normal,
                                }));
                            }
                            ui_state.transport_dropdown_open = false;
                        }
                    }
                });

            ui.add_space(4.0);
            if ui.button("Cancel").clicked() {
                ui_state.transport_dropdown_open = false;
            }
        });
    }
}
