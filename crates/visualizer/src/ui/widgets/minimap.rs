//! Reusable mini-map widgets for the task wizard, task inspector, and shelf inspector.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::visualizer::TILE_SIZE;
use protocol::config::visualizer::ui::minimap as minimap_cfg;
use protocol::grid_map::{GridMap, TileType};
use protocol::{Priority, TaskRequest, TaskType};
use std::collections::{HashMap, HashSet};

use crate::resources::{UiAction, UiState};
use super::common::{color_swatch, shelf_fill_band_label, shelf_fill_color_egui};

fn rgb(color: (u8, u8, u8)) -> egui::Color32 {
    egui::Color32::from_rgb(color.0, color.1, color.2)
}

fn base_tile_color(tile: Option<TileType>) -> egui::Color32 {
    match tile {
        Some(TileType::Wall) => egui::Color32::from_gray(minimap_cfg::WALL_GRAY),
        Some(TileType::Ground) => egui::Color32::from_gray(minimap_cfg::GROUND_GRAY),
        Some(TileType::Station) => rgb(minimap_cfg::STATION),
        Some(TileType::Dropoff) => rgb(minimap_cfg::DROPOFF),
        Some(TileType::Shelf(_)) => rgb(minimap_cfg::SHELF_BASE),
        Some(TileType::Empty) | None => egui::Color32::from_gray(minimap_cfg::EMPTY_GRAY),
    }
}

fn pickup_fill_color() -> egui::Color32 {
    rgb(minimap_cfg::PICKUP_FILL)
}

fn dropoff_fill_color() -> egui::Color32 {
    rgb(minimap_cfg::DROPOFF_FILL)
}

fn pickup_outline_color() -> egui::Color32 {
    rgb(minimap_cfg::PICKUP_OUTLINE)
}

fn dropoff_outline_color() -> egui::Color32 {
    rgb(minimap_cfg::DROPOFF_OUTLINE)
}

fn source_shelf_color() -> egui::Color32 {
    egui::Color32::from_gray(minimap_cfg::SOURCE_SHELF_GRAY)
}

fn unknown_shelf_color() -> egui::Color32 {
    egui::Color32::from_gray(minimap_cfg::SHELF_UNKNOWN_GRAY)
}

fn empty_shelf_color() -> egui::Color32 {
    egui::Color32::from_gray(minimap_cfg::SHELF_EMPTY_GRAY)
}

pub fn wizard_minimap_legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        color_swatch(ui, pickup_fill_color());
        ui.label(egui::RichText::new("pickup").small());
        color_swatch(ui, dropoff_fill_color());
        ui.label(egui::RichText::new("dropoff").small());
        color_swatch(ui, rgb(minimap_cfg::DROPOFF));
        ui.label(egui::RichText::new("drop zone").small());
        color_swatch(ui, empty_shelf_color());
        ui.label(egui::RichText::new("empty shelf").small());
        color_swatch(ui, shelf_fill_color_egui(2, 16));
        ui.label(egui::RichText::new("low").small());
        color_swatch(ui, shelf_fill_color_egui(8, 16));
        ui.label(egui::RichText::new("ok").small());
        color_swatch(ui, shelf_fill_color_egui(15, 16));
        ui.label(egui::RichText::new("full").small());
    });
}

fn task_detail_minimap_legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        color_swatch(ui, pickup_fill_color());
        ui.label(egui::RichText::new("pickup").small());
        color_swatch(ui, dropoff_fill_color());
        ui.label(egui::RichText::new("dropoff").small());
        color_swatch(ui, rgb(minimap_cfg::SHELF_BASE));
        ui.label(egui::RichText::new("shelf").small());
        color_swatch(ui, rgb(minimap_cfg::DROPOFF));
        ui.label(egui::RichText::new("drop zone").small());
        color_swatch(ui, egui::Color32::from_gray(minimap_cfg::GROUND_GRAY));
        ui.label(egui::RichText::new("ground").small());
    });
}

pub fn shelf_minimap_legend(ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        color_swatch(ui, shelf_fill_color_egui(0, 16));
        ui.label(egui::RichText::new("empty").small());
        color_swatch(ui, shelf_fill_color_egui(2, 16));
        ui.label(egui::RichText::new("low").small());
        color_swatch(ui, shelf_fill_color_egui(8, 16));
        ui.label(egui::RichText::new("ok").small());
        color_swatch(ui, shelf_fill_color_egui(15, 16));
        ui.label(egui::RichText::new("full").small());
        color_swatch(ui, source_shelf_color());
        ui.label(egui::RichText::new("src").small());
    });
}

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
    shelf_capacity: Option<&HashMap<(usize, usize), (u32, u32)>>,
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
                        pickup_fill_color()
                    } else if Some(gpos) == highlight_b {
                        dropoff_fill_color()
                    } else {
                        match grid.get_tile(col, row).map(|t| t.tile_type) {
                            Some(TileType::Shelf(_)) => {
                                if empty_positions.is_some_and(|s| s.contains(&gpos)) {
                                    empty_shelf_color()
                                } else if let Some(&(cargo, max)) =
                                    shelf_capacity.and_then(|m| m.get(&gpos))
                                {
                                    shelf_fill_color_egui(cargo, max)
                                } else {
                                    rgb(minimap_cfg::SHELF_BASE)
                                }
                            }
                            other => base_tile_color(other),
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
                            egui::Stroke::new(1.5, rgb(minimap_cfg::HOVER_OUTLINE)),
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
                        pickup_fill_color()
                    } else if Some(gpos) == dropoff {
                        dropoff_fill_color()
                    } else {
                        base_tile_color(grid.get_tile(col, row).map(|t| t.tile_type))
                    };
                    painter.rect_filled(cell_rect, 1.5, bg);
                }
            }

            // colored outline on highlighted cells
            for &(cell, color) in &[
                (pickup, pickup_outline_color()),
                (dropoff, dropoff_outline_color()),
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

    task_detail_minimap_legend(ui);
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
                        Some(TileType::Shelf(_)) => {
                            if Some(gpos) == source_grid {
                                source_shelf_color()
                            } else if let Some(&(_, cargo, max)) = entity_map.get(&gpos) {
                                shelf_fill_color_egui(cargo, max)
                            } else {
                                unknown_shelf_color()
                            }
                        }
                        other => base_tile_color(other),
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
                        egui::Stroke::new(1.5, rgb(minimap_cfg::HOVER_OUTLINE)),
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
