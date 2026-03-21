//! Logs tab: scrollable system log console.

use bevy_egui::egui;

use crate::resources::LogBuffer;

pub const LABEL: &str = "Logs";

/// Scrollable console log view with auto-scroll and clear controls.
pub fn draw(ui: &mut egui::Ui, log_buffer: &mut LogBuffer) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut log_buffer.auto_scroll, "Auto-scroll");
        if ui.button("Clear").clicked() {
            log_buffer.lines.clear();
        }
        ui.weak(format!("{} entries", log_buffer.lines.len()));
    });

    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(log_buffer.auto_scroll)
        .show(ui, |ui| {
            if log_buffer.lines.is_empty() {
                ui.weak("No log entries yet.");
            } else {
                for line in &log_buffer.lines {
                    ui.monospace(line);
                }
            }
        });
}
