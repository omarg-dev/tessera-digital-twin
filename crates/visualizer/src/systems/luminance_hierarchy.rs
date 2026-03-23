use bevy::prelude::*;
use std::collections::HashSet;

use crate::components::{BoxCargo, Ground, Shelf, Wall};
use protocol::config::visualizer::luminance;

#[derive(Resource, Default)]
pub struct LuminanceMaterialState {
    processed_ground: HashSet<AssetId<StandardMaterial>>,
    processed_wall: HashSet<AssetId<StandardMaterial>>,
    processed_shelf: HashSet<AssetId<StandardMaterial>>,
    processed_box: HashSet<AssetId<StandardMaterial>>,
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

fn apply_brightness_saturation(color: Color, brightness: f32, saturation: f32) -> Color {
    let rgba = color.to_srgba();
    let luma = (rgba.red + rgba.green + rgba.blue) / 3.0;

    let adjust = |channel: f32| -> f32 {
        let saturated = luma + (channel - luma) * saturation;
        (saturated * brightness).clamp(luminance::ALBEDO_MIN, luminance::ALBEDO_MAX)
    };

    Color::srgba(
        adjust(rgba.red),
        adjust(rgba.green),
        adjust(rgba.blue),
        rgba.alpha,
    )
}

fn style_material(
    handle: &Handle<StandardMaterial>,
    materials: &mut Assets<StandardMaterial>,
    brightness: f32,
    saturation: f32,
) {
    if let Some(material) = materials.get_mut(handle) {
        material.base_color = apply_brightness_saturation(material.base_color, brightness, saturation);
    }
}

/// Apply one-time luminance hierarchy styling to floor, wall, shelf, and cargo-box materials.
pub fn apply_luminance_hierarchy(
    mut state: ResMut<LuminanceMaterialState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ground_q: Query<Entity, With<Ground>>,
    wall_q: Query<Entity, With<Wall>>,
    shelf_q: Query<Entity, With<Shelf>>,
    box_q: Query<Entity, With<BoxCargo>>,
    children_q: Query<&Children>,
    material_q: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    let mut handles = Vec::new();

    for root in &ground_q {
        handles.clear();
        collect_material_handles(root, &children_q, &material_q, &mut handles);
        for handle in &handles {
            if state.processed_ground.insert(handle.id()) {
                style_material(
                    handle,
                    &mut materials,
                    luminance::FLOOR_BRIGHTNESS,
                    luminance::FLOOR_SATURATION,
                );
            }
        }
    }

    for root in &wall_q {
        handles.clear();
        collect_material_handles(root, &children_q, &material_q, &mut handles);
        for handle in &handles {
            if state.processed_wall.insert(handle.id()) {
                style_material(
                    handle,
                    &mut materials,
                    luminance::WALL_BRIGHTNESS,
                    luminance::WALL_SATURATION,
                );
            }
        }
    }

    for root in &shelf_q {
        handles.clear();
        collect_material_handles(root, &children_q, &material_q, &mut handles);
        for handle in &handles {
            if state.processed_shelf.insert(handle.id()) {
                style_material(
                    handle,
                    &mut materials,
                    luminance::SHELF_BRIGHTNESS,
                    luminance::SHELF_SATURATION,
                );
            }
        }
    }

    for root in &box_q {
        handles.clear();
        collect_material_handles(root, &children_q, &material_q, &mut handles);
        for handle in &handles {
            if state.processed_box.insert(handle.id()) {
                style_material(
                    handle,
                    &mut materials,
                    luminance::BOX_BRIGHTNESS,
                    luminance::BOX_SATURATION,
                );
            }
        }
    }
}
