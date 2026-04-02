//! Robot inspector panel: battery, state, position, and action buttons.

use bevy::prelude::Vec3;
use bevy_egui::egui;
use protocol::config::firmware::battery;
use protocol::RobotState;

use crate::components::Robot;
use crate::resources::UiAction;

fn state_color(state: &protocol::RobotState) -> egui::Color32 {
    match state {
        protocol::RobotState::Faulted => egui::Color32::from_rgb(225, 72, 72),
        protocol::RobotState::Blocked => egui::Color32::from_rgb(95, 145, 255),
        protocol::RobotState::Charging => egui::Color32::from_rgb(70, 210, 120),
        protocol::RobotState::LowBattery => egui::Color32::from_rgb(245, 170, 55),
        protocol::RobotState::Picking
        | protocol::RobotState::MovingToPickup
        | protocol::RobotState::MovingToDrop
        | protocol::RobotState::MovingToStation => egui::Color32::from_rgb(120, 205, 255),
        protocol::RobotState::Idle => egui::Color32::from_rgb(205, 205, 205),
    }
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: impl Into<String>, primary: bool) {
    ui.horizontal(|ui| {
        if primary {
            ui.label(egui::RichText::new(label).strong());
            ui.label(egui::RichText::new(value.into()).strong());
        } else {
            ui.weak(label);
            ui.label(egui::RichText::new(value.into()).color(egui::Color32::from_gray(190)));
        }
    });
}

fn distance_xz(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn format_point_xz(point: Vec3) -> String {
    format!("[{:.2}, {:.2}]", point.x, point.z)
}

/// Inspector for a single robot with functional action buttons.
pub fn draw(
    ui: &mut egui::Ui,
    robot: &Robot,
    active_path: Option<&[Vec3]>,
    actions: &mut Vec<UiAction>,
) {
    ui.label(
        egui::RichText::new(format!("Robot #{}", robot.id))
            .heading()
            .strong(),
    );

    ui.add_space(8.0);

    detail_row(ui, "State:", format!("{:?}", robot.state), true);
    ui.colored_label(state_color(&robot.state), "live state");

    // Position
    detail_row(
        ui,
        "Position:",
        format!("[{:.1}, {:.1}, {:.1}]", robot.position.x, robot.position.y, robot.position.z),
        false,
    );

    // Battery
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Battery").strong());
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
    ui.add_space(8.0);
    detail_row(
        ui,
        "Cargo:",
        match robot.carrying_cargo {
            Some(id) => format!("#{id}"),
            None => "None".to_string(),
        },
        false,
    );

    // Task
    detail_row(
        ui,
        "Task:",
        match robot.current_task {
            Some(id) => format!("#{id}"),
            None => "None".to_string(),
        },
        false,
    );

    // Pathfinding telemetry
    ui.add_space(10.0);
    ui.separator();
    ui.label(egui::RichText::new("Pathfinding").strong());

    let speed_xz = Vec3::new(robot.network_velocity.x, 0.0, robot.network_velocity.z).length();

    match active_path {
        Some(path_points) if !path_points.is_empty() => {
            detail_row(
                ui,
                "Telemetry:",
                format!("Active ({} waypoints)", path_points.len()),
                false,
            );

            if let Some(next) = path_points.first() {
                detail_row(ui, "Next:", format_point_xz(*next), false);
            }
            if let Some(dest) = path_points.last() {
                detail_row(ui, "Destination:", format_point_xz(*dest), false);
            }

            let mut remaining_dist = 0.0;
            let mut cursor = Vec3::new(robot.position.x, 0.0, robot.position.z);
            for &waypoint in path_points {
                let target = Vec3::new(waypoint.x, 0.0, waypoint.z);
                remaining_dist += distance_xz(cursor, target);
                cursor = target;
            }

            detail_row(ui, "Remaining dist:", format!("{remaining_dist:.2} m"), false);
            detail_row(ui, "Speed (XZ):", format!("{speed_xz:.2} m/s"), false);

            let eta = if speed_xz > 0.05 {
                format!("{:.1} s", remaining_dist / speed_xz)
            } else {
                "n/a (speed below threshold)".to_string()
            };
            detail_row(ui, "ETA:", eta, false);

            ui.add_space(4.0);
            ui.weak("Upcoming waypoints:");
            for (idx, waypoint) in path_points.iter().take(5).enumerate() {
                ui.label(
                    egui::RichText::new(format!("{:>2}. {}", idx + 1, format_point_xz(*waypoint)))
                        .small(),
                );
            }
            if path_points.len() > 5 {
                ui.weak(format!("... +{} more", path_points.len() - 5));
            }
        }
        _ => {
            detail_row(ui, "Telemetry:", "No active path", false);
            detail_row(ui, "Speed (XZ):", format!("{speed_xz:.2} m/s"), false);

            if matches!(
                robot.state,
                RobotState::MovingToPickup
                    | RobotState::MovingToDrop
                    | RobotState::MovingToStation
                    | RobotState::Blocked
                    | RobotState::Picking
            ) {
                ui.colored_label(
                    egui::Color32::from_rgb(245, 170, 55),
                    "Path telemetry missing while robot is in a movement-related state.",
                );
            }
        }
    }

    ui.add_space(12.0);
    ui.separator();
    ui.label(egui::RichText::new("Actions").strong());

    // Functional action buttons -- publish RobotControl commands via Zenoh
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

        // temporary mitigation: keep the control visible but non-interactive until
        // disabled-robot semantics are fully enforced across scheduler/coordinator.
        ui.add_enabled_ui(false, |ui| {
            if robot.enabled {
                if ui.button("Disable").clicked() {
                    actions.push(UiAction::DisableRobot(robot.id));
                }
            } else {
                if ui.button("Enable").clicked() {
                    actions.push(UiAction::EnableRobot(robot.id));
                }
            }
        });
    });
}
