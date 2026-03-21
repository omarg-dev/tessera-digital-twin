use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

use crate::components::{Dropoff, Robot, Station};
use crate::resources::{CongestionOverlayData, QueueStateData, RenderPerfCounters, TaskListData, UiState};
use protocol::config::visual::{overlays as ov_cfg, path::PATH_Y_OFFSET, TILE_SIZE};

/// Reset per-frame render counters before draw systems execute.
pub fn reset_render_perf_counters(mut counters: ResMut<RenderPerfCounters>) {
    counters.labels_drawn = 0;
    counters.labels_hidden_tier = 0;
    counters.labels_hidden_budget = 0;
    counters.path_segments_drawn = 0;
    counters.paths_faded_drawn = 0;
    counters.overlay_tiles_drawn = 0;
    counters.overlay_halos_drawn = 0;
}

/// Update congestion occupancy map at a throttled cadence.
pub fn update_congestion_overlay_data(
    time: Res<Time>,
    ui_state: Res<UiState>,
    robots: Query<&Robot>,
    mut overlay_data: ResMut<CongestionOverlayData>,
) {
    if !ui_state.show_heatmap {
        return;
    }

    overlay_data.update_accum_secs += time.delta_secs();
    if overlay_data.update_accum_secs < ov_cfg::UPDATE_INTERVAL_SECS {
        return;
    }
    overlay_data.update_accum_secs = 0.0;

    for value in overlay_data.tile_occupancy.values_mut() {
        *value *= ov_cfg::OCCUPANCY_DECAY;
    }

    for robot in &robots {
        if let Some((gx, gy)) = protocol::world_to_grid([
            robot.position.x / TILE_SIZE,
            0.0,
            robot.position.z / TILE_SIZE,
        ]) {
            let entry = overlay_data.tile_occupancy.entry((gx, gy)).or_insert(0.0);
            *entry += ov_cfg::ROBOT_OCCUPANCY_WEIGHT;
        }
    }

    overlay_data
        .tile_occupancy
        .retain(|_, v| *v >= ov_cfg::MIN_OCCUPANCY_KEEP);
    overlay_data.total_updates += 1;
}

/// Draw congestion heat tiles and queue-pressure halos.
pub fn draw_congestion_overlays(
    mut gizmos: Gizmos,
    ui_state: Res<UiState>,
    overlay_data: Res<CongestionOverlayData>,
    queue_state: Res<QueueStateData>,
    task_list: Res<TaskListData>,
    stations: Query<&Transform, With<Station>>,
    dropoffs: Query<&Transform, With<Dropoff>>,
    mut counters: ResMut<RenderPerfCounters>,
) {
    if !ui_state.show_heatmap {
        counters.overlay_updates = overlay_data.total_updates;
        return;
    }

    let mut heat_entries: Vec<((usize, usize), f32)> = overlay_data
        .tile_occupancy
        .iter()
        .map(|(k, v)| (*k, *v))
        .collect();
    heat_entries.sort_by(|a, b| b.1.total_cmp(&a.1));

    for &((gx, gy), score) in heat_entries
        .iter()
        .take(ov_cfg::MAX_HEAT_TILES_PER_FRAME)
    {
        let intensity = (score / 6.0).clamp(0.0, 1.0);
        let color = Color::linear_rgba(
            0.16 + 0.75 * intensity,
            0.24 + 0.52 * (1.0 - intensity),
            0.12,
            0.18 + 0.42 * intensity,
        );

        let center = Vec3::new(
            gx as f32 * TILE_SIZE,
            PATH_Y_OFFSET + ov_cfg::OVERLAY_Y_OFFSET,
            gy as f32 * TILE_SIZE,
        );

        let iso = Isometry3d::new(center, Quat::from_rotation_x(-FRAC_PI_2));
        gizmos.rect(iso, Vec2::splat(TILE_SIZE * 0.88), color);
        counters.overlay_tiles_drawn += 1;
    }

    let load_ratio = if queue_state.robots_online > 0 {
        queue_state.pending as f32 / queue_state.robots_online as f32
    } else {
        0.0
    }
    .clamp(0.0, 1.0);

    // include active task count as a soft pressure amplifier.
    let active_tasks = task_list
        .tasks
        .iter()
        .filter(|t| {
            matches!(
                t.status,
                protocol::TaskStatus::Pending
                    | protocol::TaskStatus::Assigned { .. }
                    | protocol::TaskStatus::InProgress { .. }
            )
        })
        .count() as f32;
    let pressure = (load_ratio + (active_tasks / 80.0)).clamp(0.0, 1.0);

    let halo_color = Color::linear_rgba(
        0.88,
        0.52 + 0.28 * (1.0 - pressure),
        0.20,
        0.34,
    );

    let mut halos_drawn = 0usize;
    let max_halos = ov_cfg::MAX_HALOS_PER_FRAME;

    for transform in &stations {
        if halos_drawn >= max_halos {
            break;
        }
        let center = transform.translation + Vec3::Y * (PATH_Y_OFFSET + ov_cfg::OVERLAY_Y_OFFSET);
        let iso = Isometry3d::new(center, Quat::from_rotation_x(-FRAC_PI_2));
        let radius = ov_cfg::HALO_BASE_RADIUS + pressure * ov_cfg::HALO_RADIUS_GAIN;
        gizmos.circle(iso, radius, halo_color);
        halos_drawn += 1;
    }

    for transform in &dropoffs {
        if halos_drawn >= max_halos {
            break;
        }
        let center = transform.translation + Vec3::Y * (PATH_Y_OFFSET + ov_cfg::OVERLAY_Y_OFFSET);
        let iso = Isometry3d::new(center, Quat::from_rotation_x(-FRAC_PI_2));
        let radius = ov_cfg::HALO_BASE_RADIUS + pressure * ov_cfg::HALO_RADIUS_GAIN;
        gizmos.circle(iso, radius, halo_color);
        halos_drawn += 1;
    }

    counters.overlay_halos_drawn = halos_drawn;
    counters.overlay_updates = overlay_data.total_updates;
}
