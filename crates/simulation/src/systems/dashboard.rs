use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::resources::DebugHUD;
use std::time::Instant;

/// Draws a small HUD at the top-left with the latest robot update info
pub fn debug_hud(
    contexts: EguiContexts,
    hud: Res<DebugHUD>,
    mut last_change: Local<Option<Instant>>,
) {
    // If HUD changed, update the timer and display message
    if hud.is_changed() {
        *last_change = Some(Instant::now());
        if let Some(text) = &hud.last_message {
            update_hud(contexts, text);
        }
        return;
    }

    // If no updates for 5 seconds, show staleness message
    if let Some(last) = *last_change {
        if last.elapsed().as_secs() > 5 {
            update_hud(contexts, &"Network is chilling for a while...".to_string());
            return;
        }
    }

    // Otherwise, redraw the existing message
    if let Some(text) = &hud.last_message {
        update_hud(contexts, text);
    }
}

fn update_hud(mut contexts: EguiContexts<'_, '_>, text: &String) {
    egui::Area::new(egui::Id::new("debug_hud"))
                .fixed_pos(egui::pos2(10.0, 10.0))
                .show(contexts.ctx_mut().expect("Failed to get egui context"), |ui| {
                    ui.label(text);
                });
}
