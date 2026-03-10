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
use protocol::{Priority, TaskRequest, TaskStatus, TaskType};
use std::collections::HashMap;

use crate::components::{Dropoff, Robot, Shelf};
use crate::resources::{
    ActivePaths, BottomTab, RightTab, LogBuffer, LeftTab, QueueStateData, RobotIndex,
    TaskListData, UiAction, UiState,
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
#[allow(clippy::too_many_arguments)]
pub fn left_panel(
    ctx: &egui::Context,
    ui_state: &mut UiState,
    robot_index: &RobotIndex,
    robots: &Query<(Entity, &Robot)>,
    shelves: &Query<(Entity, &Shelf)>,
    queue_state: &QueueStateData,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    task_list: &TaskListData,
    actions: &mut Vec<UiAction>,
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
                LeftTab::Tasks => tasks_tab(
                    ui, ui_state, queue_state, task_list,
                    shelves, dropoffs, transforms, warehouse_map, actions,
                ),
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

/// Task queue tab — stats summary + task list or Add Task wizard.
#[allow(clippy::too_many_arguments)]
fn tasks_tab(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    queue_state: &QueueStateData,
    task_list: &TaskListData,
    all_shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    // ── Stats header ──
    ui.label(egui::RichText::new("Task Queue").strong());
    ui.add_space(4.0);

    let active_count = task_list.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Assigned { .. } | TaskStatus::InProgress { .. }))
        .count();
    let failed_count = task_list.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Failed { .. } | TaskStatus::Cancelled))
        .count();
    let completed_count = task_list.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Completed))
        .count();

    egui::Grid::new("queue_stats")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Active:");
            ui.label(egui::RichText::new(active_count.to_string()).strong());
            ui.end_row();

            ui.label("Completed:");
            ui.label(completed_count.to_string());
            ui.end_row();

            ui.label("Failed:");
            let failed_color = if failed_count > 0 {
                egui::Color32::from_rgb(220, 80, 80)
            } else {
                ui.visuals().text_color()
            };
            ui.label(egui::RichText::new(failed_count.to_string()).color(failed_color));
            ui.end_row();

            ui.label("Robots:");
            ui.label(queue_state.robots_online.to_string());
            ui.end_row();
        });

    ui.separator();

    if ui_state.task_wizard_active {
        wizard_view(ui, ui_state, all_shelves, dropoffs, transforms, warehouse_map, actions);
    } else {
        task_list_view(ui, ui_state, &task_list.tasks, actions);

        ui.add_space(8.0);
        let add_btn = egui::Button::new(egui::RichText::new("+ Add New Task").strong())
            .min_size(egui::Vec2::new(ui.available_width(), 28.0));
        if ui.add(add_btn).clicked() {
            ui_state.task_wizard_active = true;
            ui_state.wizard_pickup = None;
            ui_state.wizard_dropoff = None;
            ui_state.wizard_priority = Priority::default();
        }
    }
}

/// Render categorised task rows inside a scroll area.
fn task_list_view(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    tasks: &[protocol::Task],
    _actions: &mut Vec<UiAction>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Active
            let active: Vec<_> = tasks.iter()
                .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::Assigned { .. } | TaskStatus::InProgress { .. }))
                .collect();

            let active_id = egui::Id::new("task_list_active");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), active_id, true)
                .show_header(ui, |ui| {
                    ui.label(egui::RichText::new(format!("Active ({})", active.len())).strong());
                })
                .body(|ui| {
                    for task in &active {
                        task_row(ui, task, ui_state);
                    }
                    if active.is_empty() { ui.weak("no active tasks"); }
                });

            ui.add_space(4.0);

            // Failed
            let failed: Vec<_> = tasks.iter()
                .filter(|t| matches!(t.status, TaskStatus::Failed { .. } | TaskStatus::Cancelled))
                .collect();

            let failed_id = egui::Id::new("task_list_failed");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), failed_id, true)
                .show_header(ui, |ui| {
                    ui.label(egui::RichText::new(format!("Failed ({})", failed.len()))
                        .strong()
                        .color(egui::Color32::from_rgb(220, 80, 80)));
                })
                .body(|ui| {
                    for task in &failed {
                        let is_selected = ui_state.selected_task_id == Some(task.id);
                        let label = task_row_label(task);
                        if ui.selectable_label(
                            is_selected,
                            egui::RichText::new(label).color(egui::Color32::from_rgb(220, 80, 80)),
                        ).clicked() {
                            ui_state.selected_task_id = Some(task.id);
                            ui_state.selected_entity = None;
                            ui_state.inspector_tab = RightTab::Details;
                        }
                    }
                    if failed.is_empty() { ui.weak("no failed tasks"); }
                });

            ui.add_space(4.0);

            // Completed
            let completed: Vec<_> = tasks.iter()
                .filter(|t| matches!(t.status, TaskStatus::Completed))
                .collect();

            let completed_id = egui::Id::new("task_list_completed");
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), completed_id, false)
                .show_header(ui, |ui| {
                    ui.label(egui::RichText::new(format!("Completed ({})", completed.len())).strong());
                })
                .body(|ui| {
                    for task in &completed {
                        task_row(ui, task, ui_state);
                    }
                    if completed.is_empty() { ui.weak("no completed tasks"); }
                });
        });
}

/// Single selectable task row. Click → select task and switch to Details tab.
fn task_row(ui: &mut egui::Ui, task: &protocol::Task, ui_state: &mut UiState) {
    let is_selected = ui_state.selected_task_id == Some(task.id);
    if ui.selectable_label(is_selected, task_row_label(task)).clicked() {
        ui_state.selected_task_id = Some(task.id);
        ui_state.selected_entity = None;
        ui_state.inspector_tab = RightTab::Details;
    }
}

/// Short one-line summary for a task row.
fn task_row_label(task: &protocol::Task) -> String {
    let locs = match &task.task_type {
        TaskType::PickAndDeliver { pickup, dropoff, .. } =>
            format!("({},{})→({},{})", pickup.0, pickup.1, dropoff.0, dropoff.1),
        TaskType::Relocate { from, to } =>
            format!("Move ({},{})→({},{})", from.0, from.1, to.0, to.1),
        TaskType::ReturnToStation { robot_id } =>
            format!("Return R#{robot_id}"),
    };
    let robot = match &task.status {
        TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } =>
            format!(" R#{robot_id}"),
        _ => String::new(),
    };
    format!("#{} {}{} [{:?}]", task.id, locs, robot, task.priority)
}

// ── Add Task Wizard ───────────────────────────────────────────────

/// Inline wizard that replaces the task list when `wizard_active`.
#[allow(clippy::too_many_arguments)]
fn wizard_view(
    ui: &mut egui::Ui,
    ui_state: &mut UiState,
    all_shelves: &Query<(Entity, &Shelf)>,
    dropoffs: &Query<(Entity, &Dropoff)>,
    transforms: &Query<&Transform>,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            ui_state.task_wizard_active = false;
        }
        ui.label(egui::RichText::new("Add New Task").strong());
    });

    ui.add_space(4.0);

    // ── Step 1: Pickup ──
    let pickup_done = ui_state.wizard_pickup.is_some();
    let step1_text = if let Some((x, y)) = ui_state.wizard_pickup {
        format!("Step 1: Pickup ✓ ({x},{y})")
    } else {
        "Step 1: Select Pickup Point".to_string()
    };
    ui.label(egui::RichText::new(step1_text).strong());

    if !pickup_done {
        if let Some(grid) = warehouse_map {
            if let Some(clicked) = wizard_minimap_widget(
                ui, grid,
                ui_state.wizard_pickup,
                ui_state.wizard_dropoff,
                true, false, // shelves clickable, dropoffs not
                "wzrd_pickup",
            ) {
                ui_state.wizard_pickup = Some(clicked);
            }
        } else {
            ui.weak("Map not loaded yet.");
        }
    }

    // ── Step 2: Drop-off (only after pickup chosen) ──
    if pickup_done {
        ui.add_space(4.0);
        let dropoff_done = ui_state.wizard_dropoff.is_some();
        let step2_text = if let Some((x, y)) = ui_state.wizard_dropoff {
            format!("Step 2: Drop-off ✓ ({x},{y})")
        } else {
            "Step 2: Select Drop-off Point".to_string()
        };
        ui.label(egui::RichText::new(step2_text).strong());

        if !dropoff_done {
            // build dropoff positions so we can skip the pickup cell
            let _ = (all_shelves, dropoffs, transforms); // captured for the minimap
            if let Some(grid) = warehouse_map {
                if let Some(clicked) = wizard_minimap_widget(
                    ui, grid,
                    ui_state.wizard_pickup,
                    ui_state.wizard_dropoff,
                    true, true, // shelves + dropoffs clickable
                    "wzrd_dropoff",
                ) {
                    // don't let them pick the same cell as pickup
                    if Some(clicked) != ui_state.wizard_pickup {
                        ui_state.wizard_dropoff = Some(clicked);
                    }
                }
            } else {
                ui.weak("Map not loaded yet.");
            }
        }
    }

    ui.add_space(4.0);
    ui.separator();

    // ── Priority selector ──
    ui.horizontal(|ui| {
        ui.label("Priority:");
        egui::ComboBox::from_id_salt("wizard_priority")
            .selected_text(format!("{:?}", ui_state.wizard_priority))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::Low, "Low");
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::Normal, "Normal");
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::High, "High");
                ui.selectable_value(&mut ui_state.wizard_priority, Priority::Critical, "Critical");
            });
    });

    ui.add_space(8.0);

    // ── Submit ──
    let can_submit = ui_state.wizard_pickup.is_some() && ui_state.wizard_dropoff.is_some();
    let add_btn = egui::Button::new(egui::RichText::new("Add Task").strong())
        .min_size(egui::Vec2::new(ui.available_width(), 28.0));
    if ui.add_enabled(can_submit, add_btn).clicked() {
        let pickup = ui_state.wizard_pickup.unwrap();
        let dropoff = ui_state.wizard_dropoff.unwrap();
        actions.push(UiAction::SubmitTransportTask(TaskRequest {
            task_type: TaskType::PickAndDeliver { pickup, dropoff, cargo_id: None },
            priority: ui_state.wizard_priority,
        }));
        ui_state.task_wizard_active = false;
        ui_state.wizard_pickup = None;
        ui_state.wizard_dropoff = None;
    }
}

/// Interactive mini-map used by the task wizard.
/// Tiles of the permitted type are clickable; returns the clicked grid cell.
/// `highlight_a` shows a blue marker (pickup), `highlight_b` a green one (dropoff).
fn wizard_minimap_widget(
    ui: &mut egui::Ui,
    grid: &GridMap,
    highlight_a: Option<(usize, usize)>,
    highlight_b: Option<(usize, usize)>,
    clickable_shelves: bool,
    clickable_dropoffs: bool,
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
                            Some(TileType::Shelf(_)) => egui::Color32::from_rgb(60, 100, 60),
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
                    let interactive = (is_shelf && clickable_shelves) || (is_dropoff && clickable_dropoffs);
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

/// Tabbed inspector for the selected entity or task.
#[allow(clippy::too_many_arguments)]
pub fn right_panel(
    ctx: &egui::Context,
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
                    return;
                }
                _ => {}
            }

            // Entity inspector takes priority over task inspector
            if let Some(entity) = ui_state.selected_entity {
                if let Ok((_, robot)) = robots.get(entity) {
                    robot_inspector(ui, robot, actions);
                    return;
                }
                if let Ok((_, shelf)) = shelves.get(entity) {
                    shelf_inspector(
                        ui, entity, shelf, ui_state, shelves, dropoffs, transforms,
                        warehouse_map, actions,
                    );
                    return;
                }
                ui.label(format!("Entity {:?}", entity));
                ui.label("No detailed view for this entity type.");
                return;
            }

            // Task inspector
            if let Some(task_id) = ui_state.selected_task_id {
                if let Some(task) = task_list.tasks.iter().find(|t| t.id == task_id) {
                    task_inspector(ui, task, ui_state, active_paths, warehouse_map, actions);
                } else {
                    ui.label("Task data unavailable (pending sync).");
                    ui.weak("The task list is broadcast every ~2 seconds.");
                }
                return;
            }

            ui.label("Select an entity or task to view details.");
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

// ── Task Inspector ───────────────────────────────────────────────

/// Details pane displayed when a task is selected in the task list.
fn task_inspector(
    ui: &mut egui::Ui,
    task: &protocol::Task,
    ui_state: &mut UiState,
    active_paths: &ActivePaths,
    warehouse_map: Option<&GridMap>,
    actions: &mut Vec<UiAction>,
) {
    ui.label(egui::RichText::new(format!("Task #{}", task.id)).heading().strong());
    ui.add_space(8.0);

    // type
    ui.horizontal(|ui| {
        ui.label("Type:");
        let kind = match &task.task_type {
            TaskType::PickAndDeliver { .. } => "Pick & Deliver",
            TaskType::Relocate { .. } => "Relocate",
            TaskType::ReturnToStation { .. } => "Return to Station",
        };
        ui.label(kind);
    });

    // locations + minimap
    let pickup = task.pickup_location();
    let dropoff = task.target_location();
    if let Some((px, py)) = pickup {
        ui.horizontal(|ui| {
            ui.label("Pickup:");
            ui.label(format!("({px},{py})"));
        });
    }
    if let Some((dx, dy)) = dropoff {
        ui.horizontal(|ui| {
            ui.label("Drop-off:");
            ui.label(format!("({dx},{dy})"));
        });
    }
    if pickup.is_some() || dropoff.is_some() {
        if let Some(grid) = warehouse_map {
            ui.add_space(4.0);
            task_detail_minimap(ui, grid, pickup, dropoff);
        }
    }

    ui.add_space(4.0);

    // assignment
    ui.horizontal(|ui| {
        ui.label("Robot:");
        match &task.status {
            TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } => {
                ui.label(format!("#{robot_id}"));
            }
            _ => { ui.weak("Pending"); }
        }
    });

    // status
    ui.horizontal(|ui| {
        ui.label("Status:");
        let s = match &task.status {
            TaskStatus::Pending => "Pending".to_string(),
            TaskStatus::Assigned { .. } => "Assigned".to_string(),
            TaskStatus::InProgress { .. } => "In Progress".to_string(),
            TaskStatus::Completed => "Completed".to_string(),
            TaskStatus::Failed { reason } => format!("Failed: {reason}"),
            TaskStatus::Cancelled => "Cancelled".to_string(),
        };
        ui.label(s);
    });

    // created timestamp
    ui.horizontal(|ui| {
        ui.label("Created:");
        let secs = task.created_at / 1000;
        ui.label(format!("{:02}:{:02}:{:02} UTC", (secs / 3600) % 24, (secs / 60) % 60, secs % 60));
    });

    // ETA (only when a robot is actively working on it)
    if matches!(task.status, TaskStatus::InProgress { .. } | TaskStatus::Assigned { .. }) {
        let eta_str = if let TaskStatus::Assigned { robot_id } | TaskStatus::InProgress { robot_id } = &task.status {
            if let Some(path) = active_paths.0.get(robot_id) {
                if path.is_empty() {
                    "Arriving".to_string()
                } else {
                    let eta_secs = path.len() as f32 / protocol::config::physics::ROBOT_SPEED;
                    format!("~{:.0}s", eta_secs)
                }
            } else {
                "N/A".to_string()
            }
        } else {
            "N/A".to_string()
        };
        ui.horizontal(|ui| {
            ui.label("ETA:");
            ui.label(eta_str);
        });
    }

    ui.add_space(4.0);

    // priority (editable)
    ui.horizontal(|ui| {
        ui.label("Priority:");
        let mut current = task.priority;
        let old = current;
        egui::ComboBox::from_id_salt(format!("task_prio_{}", task.id))
            .selected_text(format!("{:?}", current))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut current, Priority::Low, "Low");
                ui.selectable_value(&mut current, Priority::Normal, "Normal");
                ui.selectable_value(&mut current, Priority::High, "High");
                ui.selectable_value(&mut current, Priority::Critical, "Critical");
            });
        if current != old {
            actions.push(UiAction::ChangePriority(task.id, current));
        }
    });

    ui.add_space(12.0);
    ui.separator();
    ui.label(egui::RichText::new("Actions").strong());
    ui.add_space(4.0);

    if ui.button(egui::RichText::new("Remove Task").color(egui::Color32::from_rgb(220, 60, 60))).clicked() {
        actions.push(UiAction::CancelTask(task.id));
        ui_state.selected_task_id = None;
    }
}

/// Read-only mini-map for the task inspector showing pickup (blue) and dropoff (green).
fn task_detail_minimap(
    ui: &mut egui::Ui,
    grid: &GridMap,
    pickup: Option<(usize, usize)>,
    dropoff: Option<(usize, usize)>,
) {
    const CELL: f32 = 8.0;
    const GAP: f32 = 1.0;
    let step = CELL + GAP;
    let total_size = egui::Vec2::new(grid.width as f32 * step, grid.height as f32 * step);

    egui::ScrollArea::both()
        .id_salt("task_detail_minimap")
        .max_width(ui.available_width())
        .max_height(120.0)
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
                            Some(TileType::Shelf(_)) => egui::Color32::from_gray(80),
                            Some(TileType::Empty) | None => egui::Color32::from_gray(15),
                        }
                    };
                    painter.rect_filled(cell_rect, 1.5, bg);
                }
            }

            // white outline on highlighted cells
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
