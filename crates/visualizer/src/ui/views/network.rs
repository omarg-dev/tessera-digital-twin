//! Network diagnostics view.
//!
//! TODO: implement robot connectivity graph, packet loss, signal strength display.

use bevy_egui::egui;

/// Placeholder for the Network diagnostics panel.
pub fn draw(ui: &mut egui::Ui) {
    ui.label("Network view not yet implemented.");
    ui.add_space(4.0);
    ui.weak("Planned: robot connectivity graph, packet loss, signal strength.");
}
