use bevy::prelude::*;
use std::collections::HashSet;

use crate::components::{Ground, Shelf};
use crate::resources::LogBuffer;
use protocol::config::visualizer::diagnostics;
use protocol::save_log;

#[derive(Resource, Default)]
pub struct MaterialDiagnosticsState {
    logged_floor: HashSet<AssetId<StandardMaterial>>,
    logged_shelf: HashSet<AssetId<StandardMaterial>>,
    done: bool,
}

fn collect_material_handles(
    root: Entity,
    children_q: &Query<&Children>,
    material_q: &Query<&MeshMaterial3d<StandardMaterial>>,
    out: &mut Vec<Handle<StandardMaterial>>,
) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok(material) = material_q.get(entity) {
            out.push(material.0.clone());
        }
        if let Ok(children) = children_q.get(entity) {
            for &child in children {
                stack.push(child);
            }
        }
    }
}

fn log_material_snapshot(
    class_name: &str,
    handle: &Handle<StandardMaterial>,
    material: &StandardMaterial,
    ui_log: &mut LogBuffer,
) {
    let color = material.base_color.to_srgba();
    let line = format!(
        "material-diagnostics {class_name} id={:?} base=({:.3},{:.3},{:.3},{:.3}) metallic={:.3} roughness={:.3} unlit={} alpha={:?}",
        handle.id(),
        color.red,
        color.green,
        color.blue,
        color.alpha,
        material.metallic,
        material.perceptual_roughness,
        material.unlit,
        material.alpha_mode,
    );

    ui_log.push(line.clone());
    let _ = save_log("Visualizer", &line);
}

/// Logs first-seen imported floor and shelf material properties.
/// This runs as a temporary diagnostics pass to identify asset import issues
/// vs renderer-side tuning issues.
pub fn diagnose_imported_materials(
    mut state: ResMut<MaterialDiagnosticsState>,
    mut ui_log: ResMut<LogBuffer>,
    materials: Res<Assets<StandardMaterial>>,
    ground_q: Query<Entity, With<Ground>>,
    shelf_q: Query<Entity, With<Shelf>>,
    children_q: Query<&Children>,
    material_q: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    if !diagnostics::ENABLE_IMPORT_MATERIAL_LOGS || state.done {
        return;
    }

    let mut floor_logged_this_frame = 0usize;
    let mut shelf_logged_this_frame = 0usize;
    let mut handles = Vec::new();

    for root in &ground_q {
        if state.logged_floor.len() >= diagnostics::MAX_FLOOR_MATERIAL_LOGS {
            break;
        }

        handles.clear();
        collect_material_handles(root, &children_q, &material_q, &mut handles);
        for handle in &handles {
            if state.logged_floor.len() >= diagnostics::MAX_FLOOR_MATERIAL_LOGS {
                break;
            }

            if state.logged_floor.insert(handle.id()) {
                if let Some(material) = materials.get(handle) {
                    log_material_snapshot("floor", handle, material, &mut ui_log);
                    floor_logged_this_frame += 1;
                }
            }
        }
    }

    for root in &shelf_q {
        if state.logged_shelf.len() >= diagnostics::MAX_SHELF_MATERIAL_LOGS {
            break;
        }

        handles.clear();
        collect_material_handles(root, &children_q, &material_q, &mut handles);
        for handle in &handles {
            if state.logged_shelf.len() >= diagnostics::MAX_SHELF_MATERIAL_LOGS {
                break;
            }

            if state.logged_shelf.insert(handle.id()) {
                if let Some(material) = materials.get(handle) {
                    log_material_snapshot("shelf", handle, material, &mut ui_log);
                    shelf_logged_this_frame += 1;
                }
            }
        }
    }

    if floor_logged_this_frame > 0 || shelf_logged_this_frame > 0 {
        let summary = format!(
            "material-diagnostics summary floor_logged={} shelf_logged={} floor_total={} shelf_total={}",
            floor_logged_this_frame,
            shelf_logged_this_frame,
            state.logged_floor.len(),
            state.logged_shelf.len(),
        );
        ui_log.push(summary.clone());
        let _ = save_log("Visualizer", &summary);
    }

    if state.logged_floor.len() >= diagnostics::MAX_FLOOR_MATERIAL_LOGS
        && state.logged_shelf.len() >= diagnostics::MAX_SHELF_MATERIAL_LOGS
    {
        state.done = true;
        let done_line = "material-diagnostics complete; disable visual::diagnostics::ENABLE_IMPORT_MATERIAL_LOGS after capture".to_string();
        ui_log.push(done_line.clone());
        let _ = save_log("Visualizer", &done_line);
    }
}