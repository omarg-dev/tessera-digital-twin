//! Bottom panel views: system log console and analytics placeholder.

use bevy_egui::egui;

use crate::resources::LogBuffer;

/// Scrollable console log view.
pub fn logs_tab(ui: &mut egui::Ui, log_buffer: &mut LogBuffer) {
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
pub fn analytics_tab(ui: &mut egui::Ui) {
    ui.label("Analytics dashboard not yet implemented.");
    ui.label("Future: task throughput graph, battery histograms, heatmap stats.");
}
