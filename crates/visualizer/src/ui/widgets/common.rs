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
        return egui::Color32::from_gray(70);
    }
    if cargo == 0 {
        return egui::Color32::from_rgb(36, 148, 92);
    }

    let ratio = (cargo as f32 / max as f32).clamp(0.0, 1.0);
    if ratio <= 0.33 {
        egui::Color32::from_rgb(220, 185, 40)
    } else if ratio < 0.85 {
        egui::Color32::from_rgb(128, 180, 52)
    } else {
        egui::Color32::from_rgb(214, 68, 48)
    }
}

/// Return category label for shelf fill ratio.
pub fn shelf_fill_band_label(cargo: u32, max: u32) -> &'static str {
    if max == 0 {
        return "n/a";
    }
    if cargo == 0 {
        return "empty";
    }

    let ratio = (cargo as f32 / max as f32).clamp(0.0, 1.0);
    if ratio <= 0.33 {
        "low"
    } else if ratio < 0.85 {
        "ok"
    } else {
        "full"
    }
}
