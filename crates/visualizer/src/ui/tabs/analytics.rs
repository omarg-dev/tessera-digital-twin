//! Analytics tab.
//!
//! TODO: implement task throughput graph, battery histograms, heatmap stats.

use bevy_egui::egui;

pub const LABEL: &str = "Analytics";

/// Placeholder for the Analytics dashboard tab.
pub fn draw(ui: &mut egui::Ui) {
    ui.label("Analytics dashboard not yet implemented.");
    ui.add_space(4.0);
    ui.weak("Planned: task throughput graph, battery histograms, heatmap stats.");
}
