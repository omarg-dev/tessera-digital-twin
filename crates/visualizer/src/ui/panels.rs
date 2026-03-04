//! Panel implementations for the Digital Twin Command Center.
//!
//! Each public function renders one panel via egui immediate mode.
//! Button clicks push [`UiAction`] events which the bridge system
//! publishes over Zenoh.

use bevy::prelude::*;
use bevy_egui::egui;
use protocol::config::battery;
use protocol::config::visual::{self, ui as ui_cfg};
use protocol::{Priority, TaskRequest, TaskType};

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
                    ui.checkbox(&mut ui_state.show_ids, "IDs");
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
                    if ui.selectable_label(is_selected, text).clicked() {
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
                    let label = format!("Shelf (cargo {})", shelf.cargo);
                    if !filter.is_empty() && !label.to_lowercase().contains(&filter) {
                        continue;
                    }

                    let is_selected = ui_state.selected_entity == Some(*entity);
                    if ui.selectable_label(is_selected, &label).clicked() {
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
                            ui, entity, shelf, ui_state, shelves, dropoffs, transforms, actions,
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
                visual::SHELF_MAX_CAPACITY
            ))
            .strong(),
        );
    });

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

    // ── Transport Task Dropdown ──
    if !ui_state.transport_dropdown_open {
        if ui.button("Add Transport Task").clicked() {
            ui_state.transport_dropdown_open = true;
            ui_state.transport_shelves_expanded = false;
        }
    } else {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new("Transport to:").strong());
            ui.add_space(4.0);

            // Option 1: Dropoff
            let has_dropoff = !dropoffs.is_empty();
            if ui
                .add_enabled(has_dropoff, egui::Button::new("Dropoff"))
                .clicked()
            {
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
                }
                ui_state.transport_dropdown_open = false;
            }

            ui.add_space(2.0);

            // Option 2: Shelves (expandable)
            let id = ui.make_persistent_id("transport_shelves");
            let mut shelves_open = ui_state.transport_shelves_expanded;
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                shelves_open,
            )
            .show_header(ui, |ui| {
                if ui.selectable_label(shelves_open, "Shelves").clicked() {
                    shelves_open = !shelves_open;
                }
            })
            .body(|ui| {
                let mut sorted_shelves: Vec<_> = all_shelves.iter().collect();
                sorted_shelves.sort_unstable_by_key(|(e, _)| e.index());

                for (dest_entity, dest_shelf) in &sorted_shelves {
                    if *dest_entity == shelf_entity {
                        continue; // skip self
                    }
                    let dest_t = transforms.get(*dest_entity).ok();
                    let dest_label = dest_t
                        .map(|t| {
                            format!(
                                "Shelf @ ({:.0}, {:.0})  cargo {}",
                                t.translation.x, t.translation.z, dest_shelf.cargo
                            )
                        })
                        .unwrap_or_else(|| format!("Shelf (cargo {})", dest_shelf.cargo));

                    if ui.button(&dest_label).clicked() {
                        if let (Some(from_t), Some(to_t)) = (shelf_pos, dest_t) {
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
            ui_state.transport_shelves_expanded = shelves_open;

            ui.add_space(4.0);
            if ui.button("Cancel").clicked() {
                ui_state.transport_dropdown_open = false;
            }
        });
    }
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
