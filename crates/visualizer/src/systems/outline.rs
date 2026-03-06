//! Glowing outline system for 3D entity hover and selection.
//!
//! Uses `bevy_mod_outline` for outline rendering and Bevy's native picking
//! (`MeshPickingPlugin`) for pointer events. Outlines are inserted/removed
//! dynamically via global observers:
//!
//! - **Pointer<Over>**: white HDR hover outline (skipped if entity is selected)
//! - **Pointer<Out>**: remove outline (skipped if entity is selected)
//! - **Pointer<Click>**: toggle selection with blue HDR outline, updates `UiState`

use bevy::prelude::*;
use bevy_mod_outline::{OutlineVolume, OutlineStencil};
use protocol::config::visual::outline as cfg;

use crate::components::{Robot, Shelf, Station, Dropoff, Selected};
use crate::resources::UiState;

/// walk up the entity hierarchy to find the nearest interactive ancestor.
/// returns the interactive entity (self or ancestor with Robot/Shelf/Station/Dropoff).
fn find_interactive_ancestor(
    target: Entity,
    parents: &Query<&ChildOf>,
    interactives: &Query<(), Or<(With<Robot>, With<Shelf>, With<Station>, With<Dropoff>)>>,
) -> Option<Entity> {
    // check self
    if interactives.get(target).is_ok() {
        return Some(target);
    }
    // walk up (bounded to prevent infinite loops on cycles)
    let mut current = target;
    for _ in 0..10 {
        if let Ok(child_of) = parents.get(current) {
            let parent = child_of.parent();
            if interactives.get(parent).is_ok() {
                return Some(parent);
            }
            current = parent;
        } else {
            break;
        }
    }
    None
}

fn hover_color() -> Color {
    Color::linear_rgb(cfg::HOVER_COLOR.0, cfg::HOVER_COLOR.1, cfg::HOVER_COLOR.2)
}

fn select_color() -> Color {
    Color::linear_rgb(cfg::SELECT_COLOR.0, cfg::SELECT_COLOR.1, cfg::SELECT_COLOR.2)
}

/// observer: pointer enters entity - show white hover outline
pub fn on_pointer_over(
    mut event: On<Pointer<Over>>,
    mut commands: Commands,
    meshes: Query<(), With<Mesh3d>>,
    selected: Query<(), With<Selected>>,
    parents: Query<&ChildOf>,
    interactives: Query<(), Or<(With<Robot>, With<Shelf>, With<Station>, With<Dropoff>)>>,
) {
    let target = event.entity;

    // only process entities with actual meshes (skip scene root containers)
    if meshes.get(target).is_err() {
        return;
    }

    // only process interactive entities (self or ancestor)
    if find_interactive_ancestor(target, &parents, &interactives).is_none() {
        return;
    }

    // stop bubbling once we found a valid mesh target
    event.propagate(false);

    // don't override select color with hover
    if selected.get(target).is_ok() {
        return;
    }

    commands.entity(target).insert((
        OutlineVolume {
            visible: true,
            width: cfg::WIDTH,
            colour: hover_color(),
        },
        OutlineStencil::default(),
    ));
}

/// observer: pointer leaves entity - remove hover outline (unless selected)
pub fn on_pointer_out(
    mut event: On<Pointer<Out>>,
    mut commands: Commands,
    meshes: Query<(), With<Mesh3d>>,
    selected: Query<(), With<Selected>>,
    parents: Query<&ChildOf>,
    interactives: Query<(), Or<(With<Robot>, With<Shelf>, With<Station>, With<Dropoff>)>>,
) {
    let target = event.entity;

    if meshes.get(target).is_err() {
        return;
    }
    if find_interactive_ancestor(target, &parents, &interactives).is_none() {
        return;
    }

    event.propagate(false);

    // keep select outline when mouse leaves
    if selected.get(target).is_ok() {
        return;
    }

    if let Ok(mut ec) = commands.get_entity(target) {
        ec.remove::<(OutlineVolume, OutlineStencil)>();
    }
}

/// observer: click to select/deselect entity
pub fn on_pointer_click(
    mut event: On<Pointer<Click>>,
    mut commands: Commands,
    mut ui_state: ResMut<UiState>,
    meshes: Query<(), With<Mesh3d>>,
    selected_query: Query<Entity, With<Selected>>,
    parents: Query<&ChildOf>,
    interactives: Query<(), Or<(With<Robot>, With<Shelf>, With<Station>, With<Dropoff>)>>,
) {
    let target = event.entity;

    if meshes.get(target).is_err() {
        return;
    }
    let Some(logical) = find_interactive_ancestor(target, &parents, &interactives) else {
        return;
    };

    event.propagate(false);

    // deselect all currently selected entities (remove outline + marker)
    for prev in selected_query.iter() {
        if let Ok(mut ec) = commands.get_entity(prev) {
            ec.remove::<(Selected, OutlineVolume, OutlineStencil)>();
        }
    }

    // toggle: clicking the same logical entity deselects
    if ui_state.selected_entity == Some(logical) {
        ui_state.selected_entity = None;
        ui_state.camera_following = false;
        return;
    }

    // select the new entity
    commands.entity(target).insert((
        Selected,
        OutlineVolume {
            visible: true,
            width: cfg::WIDTH,
            colour: select_color(),
        },
        OutlineStencil::default(),
    ));
    ui_state.selected_entity = Some(logical);
    ui_state.camera_following = true;
}
