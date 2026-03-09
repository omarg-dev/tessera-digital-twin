use bevy::prelude::*;
use protocol::RobotState;

/// Robot entity - visual representation of a robot in the warehouse
#[derive(Component)]
pub struct Robot {
    pub id: u32,
    pub state: RobotState,
    /// authoritative world position from the last network update (used by UI)
    pub position: Vec3,
    pub battery: f32,
    /// TODO: Wire to task assignment display in dashboard
    #[allow(dead_code)]
    pub current_task: Option<u32>,
    pub carrying_cargo: Option<u32>,
    /// interpolation target: latest authoritative position from firmware.
    /// the interpolate_robots system lerps transform.translation toward this.
    pub target_position: Vec3,
    /// velocity reported by firmware; used for dead-reckoning between updates.
    pub network_velocity: Vec3,
    /// elapsed seconds when the last network update was applied.
    /// used by the label system to detect offline robots (no recent update).
    pub last_update_secs: f32,
    /// true when the user has right-clicked to suppress this robot's label.
    pub label_hidden: bool,
}

/// Ground tile marker
#[derive(Component)]
pub struct Ground;

/// Wall tile marker
#[derive(Component)]
pub struct Wall;

/// Shelf tile with cargo storage
#[derive(Component)]
pub struct Shelf {
    /// Number of cargo items currently on this shelf
    pub cargo: u32,
    /// Maximum cargo items this shelf can hold (from layout xN token)
    pub max_capacity: u32,
}

/// Charging station marker
#[derive(Component)]
pub struct Station;

/// Dropoff zone marker
#[derive(Component)]
pub struct Dropoff;

/// Cargo box on a shelf (child of a Shelf entity)
#[derive(Component)]
pub struct BoxCargo;

/// Marker for entities currently selected via 3D picking.
/// Added on click, removed on deselect. Drives the outline SELECT_COLOR.
#[derive(Component)]
pub struct Selected;

/// Marker placed on Mesh3d children of a sidebar-hovered entity.
/// Prevents on_pointer_out from removing the outline while the sidebar hover is active.
#[derive(Component)]
pub struct SidebarHovered;
