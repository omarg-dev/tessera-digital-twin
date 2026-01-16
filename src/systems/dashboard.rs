// use bevy::prelude::*;
// use bevy_egui::{egui, EguiContexts};
// use crate::resources::WarehouseStats;

// pub fn update_dashboard(
//     mut contexts: EguiContexts,
//     stats: Res<WarehouseStats>,
// ) {
//     egui::Window::new("Hyper-Twin Dashboard").show(contexts.ctx_mut(), |ui| {
//         ui.heading("Logistics Metrics");
//         ui.label(format!("Active Robots: {}", stats.active_robots));
//         ui.label(format!("Throughput: {} / hr", stats.package_throughput));
//     });
// }
