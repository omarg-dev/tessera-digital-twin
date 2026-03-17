//! Analytics tab.
//!
//! TODO: implement task throughput graph, battery histograms, heatmap stats.

use bevy_egui::egui;

use crate::resources::{UiAnalyticsView, WhcaMetricsData};

pub const LABEL: &str = "Analytics";

/// WHCA analytics dashboard.
pub fn draw(
    ui: &mut egui::Ui,
    whca_metrics: &WhcaMetricsData,
    analytics_view: &UiAnalyticsView,
) {
    ui.heading("WHCA Runtime Analytics");

    let Some(metrics) = &whca_metrics.latest else {
        ui.add_space(4.0);
        ui.weak("No WHCA telemetry received yet.");
        ui.weak("Start coordinator to stream metrics on factory/telemetry/whca_metrics.");
        return;
    };

    let success_pct = if metrics.searches_total > 0 {
        (metrics.searches_succeeded as f64 * 100.0) / metrics.searches_total as f64
    } else {
        0.0
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Window: {}s", metrics.window_secs));
                ui.separator();
                ui.label(format!("Last update: {:.1}s", whca_metrics.last_updated_secs));
            });

            ui.add_space(6.0);
            egui::Grid::new("whca_metrics_grid")
                .num_columns(2)
                .spacing([16.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Searches");
                    ui.monospace(format!("{}", metrics.searches_total));
                    ui.end_row();

                    ui.label("Success Rate");
                    ui.monospace(format!("{:.1}%", success_pct));
                    ui.end_row();

                    ui.label("Failed Searches");
                    ui.monospace(format!("{}", metrics.searches_failed));
                    ui.end_row();

                    ui.label("Avg Search Latency");
                    ui.monospace(format!("{} us", metrics.avg_search_time_us));
                    ui.end_row();

                    ui.label("Last Search Latency");
                    ui.monospace(format!("{} us", metrics.last_search_time_us));
                    ui.end_row();

                    ui.label("Node Expansions");
                    ui.monospace(format!("{}", metrics.nodes_expanded_total));
                    ui.end_row();

                    ui.label("Reservation Probes");
                    ui.monospace(format!("{}", metrics.reservation_probe_calls_total));
                    ui.end_row();

                    ui.label("Edge Checks");
                    ui.monospace(format!("{}", metrics.edge_collision_checks_total));
                    ui.end_row();

                    ui.label("Wait Actions");
                    ui.monospace(format!("{}", metrics.wait_actions_added_total));
                    ui.end_row();

                    ui.label("Open Set Peak");
                    ui.monospace(format!("{}", metrics.open_set_peak_observed));
                    ui.end_row();

                    ui.label("Reservation Peak");
                    ui.monospace(format!("{}", metrics.reservation_entries_peak));
                    ui.end_row();

                    ui.label("Labels Drawn");
                    ui.monospace(format!("{}", analytics_view.perf.labels_drawn));
                    ui.end_row();

                    ui.label("Labels Hidden (tier)");
                    ui.monospace(format!("{}", analytics_view.perf.labels_hidden_tier));
                    ui.end_row();

                    ui.label("Labels Hidden (budget)");
                    ui.monospace(format!("{}", analytics_view.perf.labels_hidden_budget));
                    ui.end_row();

                    ui.label("Path Segments Drawn");
                    ui.monospace(format!("{}", analytics_view.perf.path_segments_drawn));
                    ui.end_row();

                    ui.label("Path Fade Segments");
                    ui.monospace(format!("{}", analytics_view.perf.paths_faded_drawn));
                    ui.end_row();

                    ui.label("Overlay Tiles");
                    ui.monospace(format!("{}", analytics_view.perf.overlay_tiles_drawn));
                    ui.end_row();

                    ui.label("Overlay Halos");
                    ui.monospace(format!("{}", analytics_view.perf.overlay_halos_drawn));
                    ui.end_row();

                    ui.label("Overlay Update Ticks");
                    ui.monospace(format!("{}", analytics_view.perf.overlay_updates));
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Screenshot Regression Notes").strong());
            ui.weak("Use top-bar View/Baseline/After controls to save screenshots into logs/screenshots.");

            if analytics_view.snapshot_markers.is_empty() {
                ui.weak("No snapshot markers yet.");
            } else {
                for line in analytics_view.snapshot_markers.iter().rev().take(8) {
                    ui.monospace(line);
                }
            }
        });
}
