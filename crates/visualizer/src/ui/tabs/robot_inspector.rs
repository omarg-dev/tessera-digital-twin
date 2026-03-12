//! Robot inspector panel: battery, state, position, and action buttons.

use bevy_egui::egui;
use protocol::config::battery;

use crate::components::Robot;
use crate::resources::UiAction;

/// Inspector for a single robot with functional action buttons.
pub fn draw(ui: &mut egui::Ui, robot: &Robot, actions: &mut Vec<UiAction>) {
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
        // contextual enable/disable based on robot state
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
}
