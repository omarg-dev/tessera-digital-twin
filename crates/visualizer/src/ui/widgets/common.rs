//! Small reusable egui widgets shared across panels.

use bevy_egui::egui;

/// Render a fill-level color swatch for the legend.
pub fn color_swatch(ui: &mut egui::Ui, color: egui::Color32) {
    let size = egui::Vec2::splat(10.0);
    let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter().rect_filled(r, 2.0, color);
}

/// Convert cargo fill ratio into a green-to-red egui color.
pub fn shelf_fill_color_egui(cargo: u32, max: u32) -> egui::Color32 {
    if max == 0 {
        return egui::Color32::from_gray(60);
    }
    let ratio = (cargo as f32 / max as f32).clamp(0.0, 1.0);
    let r = (ratio * 210.0) as u8;
    let g = ((1.0 - ratio) * 160.0 + 50.0) as u8;
    egui::Color32::from_rgb(r, g, 30)
}
