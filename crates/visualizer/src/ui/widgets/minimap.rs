//! Reusable mini-map widgets for the task wizard, task inspector, and shelf inspector.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::visual::TILE_SIZE;
use protocol::grid_map::{GridMap, TileType};
use protocol::{Priority, TaskRequest, TaskType};
use std::collections::{HashMap, HashSet};

use crate::resources::{UiAction, UiState};
use super::common::{color_swatch, shelf_fill_band_label, shelf_fill_color_egui};

fn transform_to_grid(transform: &Transform) -> Option<(usize, usize)> {
    protocol::world_to_grid([
        transform.translation.x / TILE_SIZE,
        0.0,
        transform.translation.z / TILE_SIZE,
    ])
}

/// Interactive mini-map used by the task wizard.
/// Tiles of the permitted type are clickable; returns the clicked grid cell.
/// `highlight_a` shows a blue marker (pickup), `highlight_b` a green one (dropoff).
pub fn wizard_minimap_widget(
    ui: &mut egui::Ui,
    grid: &GridMap,
    highlight_a: Option<(usize, usize)>,
    highlight_b: Option<(usize, usize)>,
    clickable_shelves: bool,
    clickable_dropoffs: bool,
    empty_positions: Option<&HashSet<(usize, usize)>>,
    id_salt: &str,
) -> Option<(usize, usize)> {
    const CELL: f32 = 8.0;
    const GAP: f32 = 1.0;
    let step = CELL + GAP;
    let total_size = egui::Vec2::new(grid.width as f32 * step, grid.height as f32 * step);
    let mut clicked = None;

    egui::ScrollArea::both()
        .id_salt(id_salt)
        .max_width(ui.available_width())
        .max_height(160.0)
        .show(ui, |ui| {
            let (grid_rect, _) = ui.allocate_exact_size(total_size, egui::Sense::hover());
            let painter = ui.painter_at(grid_rect);

            // draw base tiles
            for row in 0..grid.height {
                for col in 0..grid.width {
                    let gpos = (col, row);
                    let cell_rect = egui::Rect::from_min_size(
                        grid_rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                        egui::Vec2::splat(CELL),
                    );
                    let bg = if Some(gpos) == highlight_a {
                        egui::Color32::from_rgb(60, 120, 220) // blue: pickup
                    } else if Some(gpos) == highlight_b {
                        egui::Color32::from_rgb(50, 190, 100) // green: dropoff
                    } else {
                        match grid.get_tile(col, row).map(|t| t.tile_type) {
                            Some(TileType::Wall) => egui::Color32::from_gray(35),
                            Some(TileType::Ground) => egui::Color32::from_gray(70),
                            Some(TileType::Station) => egui::Color32::from_rgb(100, 40, 60),
                            Some(TileType::Dropoff) => egui::Color32::from_rgb(20, 130, 70),
                            Some(TileType::Shelf(_)) => {
                                if empty_positions.map_or(false, |s| s.contains(&gpos)) {
                                    egui::Color32::from_gray(45) // empty shelf
                                } else {
                                    egui::Color32::from_rgb(60, 100, 60)
                                }
                            }
                            Some(TileType::Empty) | None => egui::Color32::from_gray(15),
                        }
                    };
                    painter.rect_filled(cell_rect, 1.5, bg);
                }
            }

            // interactions
            for row in 0..grid.height {
                for col in 0..grid.width {
                    let gpos = (col, row);
                    let Some(tile) = grid.get_tile(col, row) else { continue };
                    let is_shelf = matches!(tile.tile_type, TileType::Shelf(_));
                    let is_dropoff = matches!(tile.tile_type, TileType::Dropoff);
                    let is_empty_shelf = is_shelf && empty_positions.map_or(false, |s| s.contains(&gpos));
                    let interactive = (is_shelf && clickable_shelves && !is_empty_shelf) || (is_dropoff && clickable_dropoffs);
                    if !interactive { continue; }

                    let cell_rect = egui::Rect::from_min_size(
                        grid_rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                        egui::Vec2::splat(CELL),
                    );
                    let cell_id = egui::Id::new((id_salt, col, row));
                    let resp = ui.interact(cell_rect, cell_id, egui::Sense::click())
                        .on_hover_text(format!("({col},{row})"));

                    if resp.hovered() {
                        painter.rect_stroke(
                            cell_rect, 1.5,
                            egui::Stroke::new(1.5, egui::Color32::WHITE),
                            egui::StrokeKind::Middle,
                        );
                    }
                    if resp.clicked() {
                        clicked = Some(gpos);
                    }
                }
            }
        });

    clicked
}

/// Read-only mini-map for the task inspector showing pickup (blue) and dropoff (green).
pub fn task_detail_minimap(
    ui: &mut egui::Ui,
    grid: &GridMap,
    pickup: Option<(usize, usize)>,
    dropoff: Option<(usize, usize)>,
) {
    const CELL: f32 = 11.0;
    const GAP: f32 = 1.0;
    let step = CELL + GAP;
    let total_size = egui::Vec2::new(grid.width as f32 * step, grid.height as f32 * step);

    egui::ScrollArea::both()
        .id_salt("task_detail_minimap")
        .max_width(ui.available_width())
        .max_height(200.0)
        .show(ui, |ui| {
            let (grid_rect, _) = ui.allocate_exact_size(total_size, egui::Sense::hover());
            let painter = ui.painter_at(grid_rect);

            for row in 0..grid.height {
                for col in 0..grid.width {
                    let gpos = (col, row);
                    let cell_rect = egui::Rect::from_min_size(
                        grid_rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                        egui::Vec2::splat(CELL),
                    );
                    let bg = if Some(gpos) == pickup {
                        egui::Color32::from_rgb(60, 120, 220)
                    } else if Some(gpos) == dropoff {
                        egui::Color32::from_rgb(50, 190, 100)
                    } else {
                        match grid.get_tile(col, row).map(|t| t.tile_type) {
                            Some(TileType::Wall) => egui::Color32::from_gray(35),
                            Some(TileType::Ground) => egui::Color32::from_gray(70),
                            Some(TileType::Station) => egui::Color32::from_rgb(100, 40, 60),
                            Some(TileType::Dropoff) => egui::Color32::from_rgb(20, 130, 70),
                            Some(TileType::Shelf(_)) => egui::Color32::from_rgb(132, 100, 62),
                            Some(TileType::Empty) | None => egui::Color32::from_gray(15),
                        }
                    };
                    painter.rect_filled(cell_rect, 1.5, bg);
                }
            }

            // colored outline on highlighted cells
            for &(cell, color) in &[
                (pickup, egui::Color32::from_rgb(120, 180, 255)),
                (dropoff, egui::Color32::from_rgb(100, 240, 150)),
            ] {
                if let Some((col, row)) = cell {
                    let cell_rect = egui::Rect::from_min_size(
                        grid_rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                        egui::Vec2::splat(CELL),
                    );
                    painter.rect_stroke(
                        cell_rect, 1.0,
                        egui::Stroke::new(1.5, color),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        });

    // legend
    ui.horizontal(|ui| {
        color_swatch(ui, egui::Color32::from_rgb(60, 120, 220));
        ui.label(egui::RichText::new("pickup").small());
        color_swatch(ui, egui::Color32::from_rgb(50, 190, 100));
        ui.label(egui::RichText::new("dropoff").small());
    });
}

/// Render the compact warehouse mini-map for shelf destination picking.
#[allow(clippy::too_many_arguments)]
pub fn shelf_minimap_widget(
    ui: &mut egui::Ui,
    grid: &GridMap,
    entity_map: &HashMap<(usize, usize), (Entity, u32, u32)>,
    source_grid: Option<(usize, usize)>,
    shelf_pos: Option<&Transform>,
    transforms: &Query<&Transform>,
    ui_state: &mut UiState,
    actions: &mut Vec<UiAction>,
) {
    const CELL: f32 = 8.0;
    const GAP: f32 = 1.0;
    let step = CELL + GAP;
    let grid_w = grid.width;
    let grid_h = grid.height;
    let total_size = egui::Vec2::new(grid_w as f32 * step, grid_h as f32 * step);

    // wrap in a scroll area so very large warehouses don't overflow the panel
    egui::ScrollArea::both()
        .id_salt("shelf_minimap_scroll")
        .max_width(ui.available_width())
        .max_height(160.0)
        .show(ui, |ui| {
            let (grid_rect, _) = ui.allocate_exact_size(total_size, egui::Sense::hover());
            let painter = ui.painter_at(grid_rect);

            // draw all tiles
            for row in 0..grid_h {
                for col in 0..grid_w {
                    let gpos = (col, row);
                    let cell_rect = egui::Rect::from_min_size(
                        grid_rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                        egui::Vec2::splat(CELL),
                    );

                    let bg = match grid.get_tile(col, row).map(|t| t.tile_type) {
                        Some(TileType::Wall) => egui::Color32::from_gray(35),
                        Some(TileType::Ground) => egui::Color32::from_gray(70),
                        Some(TileType::Station) => egui::Color32::from_rgb(100, 40, 60),
                        Some(TileType::Dropoff) => egui::Color32::from_rgb(20, 120, 60),
                        Some(TileType::Shelf(_)) => {
                            if Some(gpos) == source_grid {
                                egui::Color32::from_gray(90) // source: neutral grey
                            } else if let Some(&(_, cargo, max)) = entity_map.get(&gpos) {
                                shelf_fill_color_egui(cargo, max)
                            } else {
                                egui::Color32::from_gray(55)
                            }
                        }
                        Some(TileType::Empty) | None => egui::Color32::from_gray(15),
                    };
                    painter.rect_filled(cell_rect, 1.5, bg);
                }
            }

            // handle interactions on shelf tiles (non-source)
            for (&(col, row), &(dest_entity, cargo, max)) in entity_map {
                if Some((col, row)) == source_grid {
                    // draw an X on source shelf
                    let cell_rect = egui::Rect::from_min_size(
                        grid_rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                        egui::Vec2::splat(CELL),
                    );
                    let stroke = egui::Stroke::new(1.5, egui::Color32::from_gray(180));
                    painter.line_segment([cell_rect.min, cell_rect.max], stroke);
                    painter.line_segment(
                        [cell_rect.right_top(), cell_rect.left_bottom()],
                        stroke,
                    );
                    continue;
                }

                let cell_rect = egui::Rect::from_min_size(
                    grid_rect.min + egui::vec2(col as f32 * step, row as f32 * step),
                    egui::Vec2::splat(CELL),
                );
                let cell_id = egui::Id::new(("minimap", col, row));
                let resp = ui.interact(cell_rect, cell_id, egui::Sense::click())
                    .on_hover_text(format!(
                        "({col},{row})  {cargo}/{max} ({})",
                        shelf_fill_band_label(cargo, max)
                    ));

                if resp.hovered() {
                    painter.rect_stroke(
                        cell_rect,
                        1.5,
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                        egui::StrokeKind::Middle,
                    );
                    ui_state.hovered_entity = Some(dest_entity);
                }

                if resp.clicked() {
                    if let (Some(from_t), Ok(to_t)) = (shelf_pos, transforms.get(dest_entity)) {
                        if let (Some(from), Some(to)) = (
                            transform_to_grid(from_t),
                            transform_to_grid(to_t),
                        ) {
                            actions.push(UiAction::SubmitTransportTask(TaskRequest {
                                task_type: TaskType::Relocate { from, to },
                                priority: Priority::Normal,
                            }));
                        }
                    }
                    ui_state.transport_dropdown_open = false;
                }
            }
        });
}
