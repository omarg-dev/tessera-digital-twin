//! Panel implementations for the Digital Twin Command Center.
//!
//! Each public function renders one panel via egui immediate mode.
//! Button clicks push [`UiAction`] events which the bridge system
//! publishes over Zenoh.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::battery;
use protocol::config::visual::{ui as ui_cfg, TILE_SIZE};
use protocol::grid_map::{GridMap, TileType};
use protocol::{Priority, TaskRequest, TaskType};
use std::collections::HashMap;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{
    BottomTab, RightTab, LogBuffer, LeftTab, QueueStateData, RobotIndex, UiAction, UiState,
};

// ── Top Panel ─────────────────────────────────────────────────────

/// HUD bar: simulation controls (left), KPIs (center), layer toggles (right).
pub fn top_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    queue_state: &QueueStateData,
    time: &Time,
    actions: &mut Vec<UiAction>,
) {
    egui::TopBottomPanel::top("top_panel")
        .exact_height(ui_cfg::TOP_PANEL_HEIGHT)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // ── Left: Simulation Controls ──
                sim_controls(ui, ui_state, actions);

                ui.separator();

                // Mode toggle: Simulation / Real-time
                let mode_label = if ui_state.is_realtime { "Real-time" } else { "Simulation" };
                let mode_btn = egui::Button::new(mode_label)
                    .selected(ui_state.is_realtime);
                if ui.add(mode_btn).clicked() {
                    ui_state.is_realtime = !ui_state.is_realtime;
                }

                ui.separator();

                // ── Center: KPIs ──
                let active = robot_index.by_id.len();
                ui.label(egui::RichText::new(format!("Robots: {active}")).strong());
                ui.separator();

                let fps = 1.0 / time.delta_secs().max(0.0001);
                ui.label(format!("FPS: {fps:.0}"));
                ui.separator();

                // Live throughput from scheduler QueueState
                if queue_state.total > 0 {
                    let completed = queue_state.total.saturating_sub(queue_state.pending);
                    ui.label(format!(
                        "Tasks: {completed}/{} done  ({} pending)",
                        queue_state.total, queue_state.pending
                    ));
                } else {
                    ui.label("Tasks: --");
                }

                // Push layer toggles to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut ui_state.show_ids, "Labels");
                    ui.checkbox(&mut ui_state.show_heatmap, "Heatmap");
                    ui.checkbox(&mut ui_state.show_paths, "Paths");
                });
            });
        });
}

/// Speed / play-pause cluster. Publishes Pause/Resume over Zenoh.
fn sim_controls(ui: &mut egui::Ui, ui_state: &mut UiState, actions: &mut Vec<UiAction>) {
    let pause_label = if ui_state.is_paused { "\u{25B6}" } else { "\u{23F8}" }; // ▶ / ⏸
    if ui.button(pause_label).clicked() {
        ui_state.is_paused = !ui_state.is_paused;
        actions.push(UiAction::SetPaused(ui_state.is_paused));
    }

    let speeds: &[(&str, f32)] = &[("1x", 1.0), ("10x", 10.0), ("Max", f32::MAX)];
    for &(label, _factor) in speeds {
        let btn = egui::Button::new(label).selected(label == "1x");
        let response = ui.add_enabled(false, btn);
        response.on_disabled_hover_text("Speed control not yet wired");
    }
}

// ── Left Panel (Object Manager) ──────────────────────────────────

/// Tabbed list of robots, shelves, and tasks with a search bar.
pub fn left_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
    queue_state: &QueueStateData,
) {
    egui::SidePanel::left("left_panel")
        .default_width(ui_cfg::SIDE_PANEL_DEFAULT_WIDTH)
        .width_range(ui_cfg::SIDE_PANEL_MIN_WIDTH..=ui_cfg::SIDE_PANEL_MAX_WIDTH)
        .show(ctx, |ui| {
            // ── Tab bar ──
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.object_tab, LeftTab::Objects, "Objects");
                ui.selectable_value(&mut ui_state.object_tab, LeftTab::Tasks, "Tasks");
            });

            ui.separator();

            match ui_state.object_tab {
                LeftTab::Objects => objects_tab(ui, ui_state, robot_index, robots, shelves),
                LeftTab::Tasks => tasks_tab(ui, queue_state),
            }
        });
}

/// Objects list: search bar + collapsible robot/shelf sections.
fn objects_tab(
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

/// Task queue tab — displays live QueueState data from the scheduler.
fn tasks_tab(ui: &mut egui::Ui, queue_state: &QueueStateData) {
    ui.label(egui::RichText::new("Task Queue").strong());
    ui.add_space(4.0);

    egui::Grid::new("queue_stats")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Pending:");
            ui.label(egui::RichText::new(format!("{}", queue_state.pending)).strong());
            ui.end_row();

            ui.label("Total:");
            ui.label(format!("{}", queue_state.total));
            ui.end_row();

            let completed = queue_state.total.saturating_sub(queue_state.pending);
            ui.label("Completed:");
            ui.label(format!("{completed}"));
            ui.end_row();

            ui.label("Robots Online:");
            ui.label(format!("{}", queue_state.robots_online));
            ui.end_row();
        });

    if queue_state.total == 0 {
        ui.add_space(8.0);
        ui.weak("No tasks received yet.\nUse the scheduler CLI to add tasks.");
    }
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

/// Select an entity from the list: sets selection, enables camera follow,
/// and resets transport dropdown state.
fn select_entity(ui_state: &mut UiState, entity: Entity) {
    ui_state.selected_entity = Some(entity);
    ui_state.camera_following = true;
    ui_state.transport_dropdown_open = false;
    ui_state.transport_shelves_expanded = false;
}

// ── Right Panel (Inspector) ──────────────────────────────────────

/// Tabbed inspector for the selected entity.
pub fn right_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    egui::SidePanel::right("right_panel")
        .default_width(ui_cfg::SIDE_PANEL_DEFAULT_WIDTH)
        .width_range(ui_cfg::SIDE_PANEL_MIN_WIDTH..=ui_cfg::SIDE_PANEL_MAX_WIDTH)
        .show(ctx, |ui| {
            // ── Tab bar ──
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut ui_state.inspector_tab,
                    RightTab::Details,
                    "Details",
                );
                ui.selectable_value(
                    &mut ui_state.inspector_tab,
                    RightTab::Network,
                    "Network",
                );
            });

            ui.separator();

            match ui_state.inspector_tab {
                RightTab::Network => {
                    ui.label("Network view not yet implemented.");
                    ui.label("Future: show robot connectivity, packet loss graph, signal strength, etc.");
                }
                _ => {}
            }

            let Some(entity) = ui_state.selected_entity else {
                ui.label("Select an entity to view details.");
                return;
            };

            match ui_state.inspector_tab {
                RightTab::Details => {
                    // try robot first (O(1) lookup)
                    if let Ok((_, robot)) = robots.get(entity) {
                        robot_inspector(ui, robot, actions);
                        return;
                    }

                    // try shelf (O(1) lookup)
                    if let Ok((_, shelf)) = shelves.get(entity) {
                        shelf_inspector(
                            ui, entity, shelf, ui_state, shelves, dropoffs, transforms,
                            warehouse_map, actions,
                        );
                        return;
                    }

                    // Unknown entity
                    ui.label(format!("Entity {:?}", entity));
                    ui.label("No detailed view for this entity type.");
                }
                
                RightTab::Network => {
                    ui.label("Entity specific Network view not yet implemented.");
                    ui.label("Future: show robot connectivity, packet loss graph, signal strength, etc.");
                }
            }
        });
}

/// Inspector for a single robot with functional action buttons.
fn robot_inspector(ui: &mut egui::Ui, robot: &Robot, actions: &mut Vec<UiAction>) {
    ui.label(
        egui::RichText::new(format!("Robot #{}", robot.id))
            .heading()
            .strong(),
    );

    ui.add_space(8.0);

    // State
    ui.horizontal(|ui| {
        ui.label("State:");
        ui.label(egui::RichText::new(format!("{:?}", robot.state)).strong());
    });

    // Position
    ui.horizontal(|ui| {
        ui.label("Position:");
        ui.label(format!(
            "[{:.1}, {:.1}, {:.1}]",
            robot.position.x, robot.position.y, robot.position.z
        ));
    });

    // Battery
    ui.add_space(4.0);
    ui.label("Battery:");
    let battery_frac = robot.battery / 100.0;
    let bar = egui::ProgressBar::new(battery_frac)
        .text(format!("{:.1}%", robot.battery))
        .fill(if robot.battery < battery::LOW_THRESHOLD {
            egui::Color32::from_rgb(220, 50, 50)
        } else if robot.battery < battery::MIN_BATTERY_FOR_TASK {
            egui::Color32::from_rgb(220, 180, 50)
        } else {
            egui::Color32::from_rgb(50, 200, 80)
        });
    ui.add(bar);

    // Cargo
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Cargo:");
        match robot.carrying_cargo {
            Some(id) => ui.label(format!("#{id}")),
            None => ui.label("None"),
        };
    });

    // Task
    ui.horizontal(|ui| {
        ui.label("Task:");
        match robot.current_task {
            Some(id) => ui.label(format!("#{id}")),
            None => ui.label("None"),
        };
    });

    ui.add_space(12.0);
    ui.separator();
    ui.label(egui::RichText::new("Actions").strong());

    // Functional action buttons — publish RobotControl commands via Zenoh
    ui.horizontal(|ui| {
        if ui
            .button(egui::RichText::new("Kill").color(egui::Color32::from_rgb(220, 60, 60)))
            .clicked()
        {
            actions.push(UiAction::KillRobot(robot.id));
        }
        if ui.button("Restart").clicked() {
            actions.push(UiAction::RestartRobot(robot.id));
        }
        if ui.button("Enable").clicked() {
            actions.push(UiAction::EnableRobot(robot.id));
        }
    });
}

/// Inspector for a shelf: cargo display and transport task creation.
#[allow(clippy::too_many_arguments)]
fn shelf_inspector(
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

/// Render a fill-level color swatch for the legend.
fn color_swatch(ui: &mut egui::Ui, color: egui::Color32) {
    let size = egui::Vec2::splat(10.0);
    let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(r, 2.0, color);
}

/// Convert cargo fill ratio into a green-to-red egui color.
fn shelf_fill_color_egui(cargo: u32, max: u32) -> egui::Color32 {
    if max == 0 {
        return egui::Color32::from_gray(60);
    }
    let ratio = (cargo as f32 / max as f32).clamp(0.0, 1.0);
    let r = (ratio * 210.0) as u8;
    let g = ((1.0 - ratio) * 160.0 + 50.0) as u8;
    egui::Color32::from_rgb(r, g, 30)
}

/// Render the compact warehouse mini-map for shelf destination picking.
#[allow(clippy::too_many_arguments)]
fn shelf_minimap_widget(
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
                    .on_hover_text(format!("({col},{row})  {cargo}/{max}"));

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
}

// ── Bottom Panel (Logs / Analytics) ──────────────────────────────

/// Tabbed bottom panel: system logs and analytics placeholder.
pub fn bottom_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    log_buffer: &mut LogBuffer,
) {
    egui::TopBottomPanel::bottom("bottom_panel")
        .default_height(ui_cfg::BOTTOM_PANEL_DEFAULT_HEIGHT)
        .height_range(ui_cfg::BOTTOM_PANEL_MIN_HEIGHT..=ui_cfg::BOTTOM_PANEL_MAX_HEIGHT)
        .resizable(true)
        .show(ctx, |ui| {
            // Tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.bottom_tab, BottomTab::Logs, "Logs");
                ui.selectable_value(&mut ui_state.bottom_tab, BottomTab::Analytics, "Analytics");
            });

            ui.separator();

            match ui_state.bottom_tab {
                BottomTab::Logs => logs_tab(ui, log_buffer),
                BottomTab::Analytics => analytics_tab(ui),
            }
        });
}

/// Scrollable console log view.
fn logs_tab(ui: &mut egui::Ui, log_buffer: &mut LogBuffer) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut log_buffer.auto_scroll, "Auto-scroll");
        if ui.button("Clear").clicked() {
            log_buffer.lines.clear();
        }
        ui.weak(format!("{} entries", log_buffer.lines.len()));
    });

    ui.separator();

    let scroll = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(log_buffer.auto_scroll);

    scroll.show(ui, |ui| {
        if log_buffer.lines.is_empty() {
            ui.weak("No log entries yet.");
        } else {
            for line in &log_buffer.lines {
                ui.monospace(line);
            }
        }
    });
}

/// Analytics placeholder.
fn analytics_tab(ui: &mut egui::Ui) {
    ui.label("Analytics dashboard not yet implemented.");
    ui.label("Future: task throughput graph, battery histograms, heatmap stats.");
}
