//! Analytics tab.
//!
//! TODO: implement task throughput graph, battery histograms, heatmap stats.

use bevy_egui::egui;

use crate::resources::WhcaMetricsData;

pub const LABEL: &str = "Analytics";

/// WHCA analytics dashboard.
pub fn draw(ui: &mut egui::Ui, whca_metrics: &WhcaMetricsData) {
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
        });
}
