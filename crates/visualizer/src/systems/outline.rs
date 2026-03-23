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
use bevy::picking::pointer::PointerButton;
use bevy_mod_outline::{OutlineVolume, OutlineStencil};
use protocol::config::visual::outline as cfg;
use std::collections::HashMap;

use crate::components::{Robot, Shelf, Station, Dropoff, Selected, SidebarHovered};
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

    if let Ok(mut ec) = commands.get_entity(target) {
        ec.insert((
            OutlineVolume {
                visible: true,
                width: cfg::WIDTH,
                colour: hover_color(),
            },
            OutlineStencil::default(),
        ));
    }
}

/// observer: pointer leaves entity - remove hover outline (unless selected or sidebar-hovered)
pub fn on_pointer_out(
    mut event: On<Pointer<Out>>,
    mut commands: Commands,
    meshes: Query<(), With<Mesh3d>>,
    selected: Query<(), With<Selected>>,
    sidebar_hovered: Query<(), With<SidebarHovered>>,
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

    // keep sidebar hover outline when mouse leaves
    if sidebar_hovered.get(target).is_ok() {
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
    robots: Query<(), With<Robot>>,
) {
    let target = event.entity;

    // right-click on a robot: hide its label (reappears when the entity is deselected)
    if event.button == PointerButton::Secondary {
        if meshes.get(target).is_ok() {
            if let Some(logical) = find_interactive_ancestor(target, &parents, &interactives) {
                if robots.get(logical).is_ok() {
                    event.propagate(false);
                    ui_state.hidden_labels.insert(logical);
                }
            }
        }
        return;
    }

    if event.button != PointerButton::Primary {
        return;
    }

    if meshes.get(target).is_err() {
        return;
    }
    let Some(logical) = find_interactive_ancestor(target, &parents, &interactives) else {
        return;
    };

    event.propagate(false);

    // clear hidden label for whichever entity was previously selected so it
    // reappears when deselected (whether via toggle-click or selecting a new entity).
    if let Some(prev_selected) = ui_state.selected_entity {
        ui_state.hidden_labels.remove(&prev_selected);
    }

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
    if let Ok(mut ec) = commands.get_entity(target) {
        ec.insert((
            Selected,
            OutlineVolume {
                visible: true,
                width: cfg::WIDTH,
                colour: select_color(),
            },
            OutlineStencil::default(),
        ));
    }
    ui_state.selected_entity = Some(logical);
    ui_state.camera_following = true;
    ui_state.entity_picked_this_frame = true;
}

// ── Programmatic outline sync (sidebar selection and hover) ──────

/// Keeps track of which mesh entities currently carry programmatic outlines.
#[derive(Default)]
pub struct ProgrammaticOutlineState {
    selected_entity: Option<Entity>,
    hovered_entity: Option<Entity>,
    /// mesh-level children that currently carry Selected + OutlineVolume from this system
    selected_meshes: Vec<Entity>,
    /// mesh-level children that currently carry SidebarHovered + OutlineVolume from this system
    hovered_meshes: Vec<Entity>,
    /// cached mesh descendants for logical entities used by sidebar-driven outlines
    mesh_cache: HashMap<Entity, Vec<Entity>>,
}

/// Walk the entity hierarchy and collect all Mesh3d descendants.
fn collect_mesh_descendants(
    root: Entity,
    children_q: &Query<&Children>,
    meshes_q: &Query<(), With<Mesh3d>>,
) -> Vec<Entity> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if meshes_q.get(e).is_ok() {
            result.push(e);
        }
        if let Ok(children) = children_q.get(e) {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }
    result
}

fn cached_mesh_descendants(
    root: Entity,
    state: &mut ProgrammaticOutlineState,
    children_q: &Query<&Children>,
    meshes_q: &Query<(), With<Mesh3d>>,
) -> Vec<Entity> {
    // if root no longer exists, clear stale cache entry
    if children_q.get(root).is_err() && meshes_q.get(root).is_err() {
        state.mesh_cache.remove(&root);
        return Vec::new();
    }

    if let Some(cached) = state.mesh_cache.get(&root) {
        // if every cached mesh still exists, reuse cache
        if cached.iter().all(|&mesh_e| meshes_q.get(mesh_e).is_ok()) {
            return cached.clone();
        }

        // cached meshes went stale due to despawn/spawn churn
        state.mesh_cache.remove(&root);
    }

    let meshes = collect_mesh_descendants(root, children_q, meshes_q);
    state.mesh_cache.insert(root, meshes.clone());
    meshes
}

/// System: syncs 3D outlines for sidebar selection and sidebar hover.
///
/// Runs every Update frame. When `ui_state.selected_entity` or
/// `ui_state.hovered_entity` change, removes stale outlines and applies
/// new ones to all Mesh3d descendants. This ensures sidebar-driven
/// selection shows the same outline as 3D-click selection.
pub fn sync_programmatic_outlines(
    mut commands: Commands,
    ui_state: Res<UiState>,
    mut state: Local<ProgrammaticOutlineState>,
    children_q: Query<&Children>,
    meshes_q: Query<(), With<Mesh3d>>,
) {
    let sel_changed = ui_state.selected_entity != state.selected_entity;
    let hov_changed = ui_state.hovered_entity != state.hovered_entity;

    if !sel_changed && !hov_changed {
        return;
    }

    // ── handle selected entity change ──
    if sel_changed {
        // remove outlines from previously selected meshes
        for &mesh_e in &state.selected_meshes {
            if let Ok(mut ec) = commands.get_entity(mesh_e) {
                ec.remove::<(Selected, OutlineVolume, OutlineStencil)>();
            }
        }
        state.selected_meshes.clear();

        if let Some(logical) = ui_state.selected_entity {
            let meshes = cached_mesh_descendants(logical, &mut state, &children_q, &meshes_q);
            let mut applied = Vec::new();
            for &mesh_e in &meshes {
                if let Ok(mut ec) = commands.get_entity(mesh_e) {
                    ec.insert((
                        Selected,
                        OutlineVolume {
                            visible: true,
                            width: cfg::WIDTH,
                            colour: select_color(),
                        },
                        OutlineStencil::default(),
                    ));
                    applied.push(mesh_e);
                }
            }
            state.selected_meshes = applied;
        }
        state.selected_entity = ui_state.selected_entity;
    }

    // ── handle hovered entity change ──
    if hov_changed {
        // remove outlines from previously hovered meshes
        for &mesh_e in &state.hovered_meshes {
            if let Ok(mut ec) = commands.get_entity(mesh_e) {
                ec.remove::<(SidebarHovered, OutlineVolume, OutlineStencil)>();
            }
        }
        state.hovered_meshes.clear();

        if let Some(logical) = ui_state.hovered_entity {
            // don't add hover outline if this entity is already selected
            let is_selected = ui_state.selected_entity == Some(logical);
            if !is_selected {
                let meshes = cached_mesh_descendants(logical, &mut state, &children_q, &meshes_q);
                let mut applied = Vec::new();
                for &mesh_e in &meshes {
                    if let Ok(mut ec) = commands.get_entity(mesh_e) {
                        ec.insert((
                            SidebarHovered,
                            OutlineVolume {
                                visible: true,
                                width: cfg::WIDTH,
                                colour: hover_color(),
                            },
                            OutlineStencil::default(),
                        ));
                        applied.push(mesh_e);
                    }
                }
                state.hovered_meshes = applied;
            }
        }
        state.hovered_entity = ui_state.hovered_entity;
    }
}
