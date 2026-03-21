//! Robot inspector panel: battery, state, position, and action buttons.

use bevy_egui::egui;
use protocol::config::battery;

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

/// Inspector for a single robot with functional action buttons.
pub fn draw(ui: &mut egui::Ui, robot: &Robot, actions: &mut Vec<UiAction>) {
    ui.label(
        egui::RichText::new(format!("Robot #{}", robot.id))
            .heading()
            .strong(),
    );

    ui.add_space(8.0);

    detail_row(ui, "State:", format!("{:?}", robot.state), true);
    ui.colored_label(state_color(&robot.state), "● live state");

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
