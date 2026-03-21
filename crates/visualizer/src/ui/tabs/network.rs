//! Network diagnostics tab.
//!
//! TODO: implement robot connectivity graph, packet loss, signal strength display.

use bevy_egui::egui;

pub const LABEL: &str = "Network";

/// Placeholder for the Network diagnostics tab.
pub fn draw(ui: &mut egui::Ui) {
    ui.label("Network view not yet implemented.");
    ui.add_space(4.0);
    ui.weak("Planned: robot connectivity graph, packet loss, signal strength.");
}
