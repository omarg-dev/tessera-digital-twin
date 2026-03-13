//! Resources for the Visualizer crate
//!
//! Render layer + Digital Twin Command Center.
//! Subscribes to robot updates, task queue state, and system commands.
//! Publishes control commands (pause/resume, robot control) from the UI.

use bevy::prelude::*;
use protocol::config::visual::ui as ui_cfg;
use protocol::grid_map::GridMap;
use protocol::{Priority, QueueState, RobotControl, RobotUpdate, SystemCommand, Task, TaskCommand, TaskListSnapshot, TaskRequest};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zenoh::Session;

/// Shared Zenoh session and Tokio runtime for all visualizer background tasks.
/// One runtime is created at startup and shared by all subscribers/publishers.
#[derive(Resource, Clone)]
pub struct ZenohSession {
    pub session: Session,
    pub runtime: Arc<Runtime>,
}

/// Open a single Zenoh session and create one shared Tokio runtime.
///
/// All background subscribers and publishers use `runtime.spawn()` instead
/// of creating their own `thread::spawn` + `Runtime::new()` pairs.
pub fn open_zenoh_session() -> ZenohSession {
    let rt = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
    let session = rt.block_on(async {
        zenoh::open(zenoh::Config::default())
            .await
            .expect("Failed to open Zenoh session")
    });

    ZenohSession { session, runtime: rt }
}

/// Receives robot updates from Zenoh (firmware publishes, we display)
#[derive(Resource)]
pub struct ZenohReceiver(pub mpsc::Receiver<RobotUpdate>);

/// Stores latest robot updates for systems to consume
#[derive(Resource, Default)]
pub struct RobotUpdates {
    pub updates: Vec<RobotUpdate>,
}

/// Fast lookup for robot entities by ID
#[derive(Resource, Default)]
pub struct RobotIndex {
    pub by_id: HashMap<u32, Entity>,
}

/// Tracks the last seen position and state for each robot (for dedup in zenoh_receiver).
/// Prevents processing duplicate updates when neither position nor state changed.
#[derive(Resource, Default)]
pub struct RobotLastPositions {
    pub by_id: HashMap<u32, [f32; 3]>,
    pub state_by_id: HashMap<u32, protocol::RobotState>,
}

/// Warehouse grid map, shared as a Bevy resource for tile lookups.
#[derive(Resource)]
pub struct WarehouseMap(pub GridMap);

/// Shared mesh+material handles for placeholder entities (station, dropoff, robot).
/// Avoids creating duplicate GPU assets per entity.
#[derive(Resource, Clone)]
pub struct PlaceholderMeshes {
    pub station_mesh: Handle<Mesh>,
    pub station_material: Handle<StandardMaterial>,
    pub dropoff_mesh: Handle<Mesh>,
    pub dropoff_material: Handle<StandardMaterial>,
    /// TODO: replace with .glb robot model when available
    pub robot_mesh: Handle<Mesh>,
    pub robot_material: Handle<StandardMaterial>,
}

// ── UI State ──────────────────────────────────────────────────────

/// Active tab in the left Object Manager panel
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum LeftTab {
    #[default]
    Objects,
    Tasks,
}

/// Active tab in the bottom panel
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum BottomTab {
    #[default]
    Logs,
    Analytics,
}

/// Active tab in the right Inspector panel
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum RightTab {
    #[default]
    Details,
    Network,
}

/// Central UI state resource – drives all egui panels
#[derive(Resource)]
pub struct UiState {
    /// Currently selected entity in the 3D scene (for the Inspector panel)
    pub selected_entity: Option<Entity>,
    /// Search filter for the robot/shelf list
    pub filter_query: String,
    /// Layer toggle: draw robot path trails
    pub show_paths: bool,
    /// Layer toggle: draw traffic heatmap overlay
    pub show_heatmap: bool,
    /// Layer toggle: draw debug grid
    /// TODO: Wire to 3D gizmo system
    #[allow(dead_code)]
    pub show_debug_grid: bool,
    /// Layer toggle: show robot ID labels
    pub show_ids: bool,
    /// Simulation speed multiplier (1.0 = real-time)
    pub sim_speed: f32,
    /// Custom speed text field is being edited
    pub custom_speed_editing: bool,
    /// Custom speed text buffer
    pub custom_speed_text: String,
    /// Whether the simulation is paused
    pub is_paused: bool,
    /// Real-time mode toggle (true = real hardware, false = simulation)
    pub is_realtime: bool,
    /// Pause state captured when entering real-time mode.
    /// Used to restore pause/resume behavior when leaving real-time mode.
    pub paused_before_realtime: Option<bool>,
    /// Active tab in left Object Manager panel
    pub object_tab: LeftTab,
    /// Active tab in bottom panel
    pub bottom_tab: BottomTab,
    /// Active tab in right Inspector panel
    pub inspector_tab: RightTab,
    /// Camera follows the selected entity
    pub camera_following: bool,
    /// Transport task dropdown is open in shelf inspector
    pub transport_dropdown_open: bool,
    /// Shelves sub-menu is expanded in transport dropdown
    pub transport_shelves_expanded: bool,
    /// Entity being hovered over in the sidebar object list (drives 3D hover outline)
    pub hovered_entity: Option<Entity>,
    /// Set to true by on_pointer_click when a 3D interactive entity absorbs a click.
    /// Used in draw_ui to prevent background-click deselect from firing.
    pub entity_picked_this_frame: bool,
    /// Entities whose robot label is explicitly hidden by right-click.
    /// Cleared automatically when the entity is deselected.
    pub hidden_labels: HashSet<Entity>,
    /// true when the Add Task wizard replaces the task list
    pub task_wizard_active: bool,
    /// wizard step 1: chosen pickup grid cell
    pub wizard_pickup: Option<(usize, usize)>,
    /// wizard step 2: chosen dropoff/shelf grid cell
    pub wizard_dropoff: Option<(usize, usize)>,
    /// wizard priority selection
    pub wizard_priority: Priority,
    /// selected task ID for the Details inspector (None = no task selected)
    pub selected_task_id: Option<u64>,
    /// the user scrolled the scroll wheel this frame (set by camera_controls, read by follow systems)
    /// cleared at the start of camera_controls so it only fires in the same frame as the scroll
    pub camera_scroll_this_frame: bool,
    /// the user panned the camera this frame (set by camera_controls, read by follow systems)
    pub camera_pan_this_frame: bool,
    /// the user orbited the camera this frame (right drag) — cancels entity focus lerp
    pub camera_orbit_this_frame: bool,
    /// actual width of the left side panel in egui logical pixels — updated each frame by gui.rs
    pub left_panel_width: f32,
    /// actual width of the right side panel in egui logical pixels — updated each frame by gui.rs
    pub right_panel_width: f32,
    /// actual height of the bottom panel in egui logical pixels — updated each frame by gui.rs
    pub bottom_panel_height: f32,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            filter_query: String::new(),
            show_paths: false,
            show_heatmap: false,
            show_debug_grid: false,
            show_ids: true,
            sim_speed: 1.0,
            custom_speed_editing: false,
            custom_speed_text: String::new(),
            is_paused: false,
            is_realtime: false,
            paused_before_realtime: None,
            object_tab: LeftTab::default(),
            bottom_tab: BottomTab::default(),
            inspector_tab: RightTab::default(),
            camera_following: false,
            transport_dropdown_open: false,
            transport_shelves_expanded: false,
            hovered_entity: None,
            entity_picked_this_frame: false,
            hidden_labels: HashSet::new(),
            task_wizard_active: false,
            wizard_pickup: None,
            wizard_dropoff: None,
            wizard_priority: Priority::default(),
            selected_task_id: None,
            camera_scroll_this_frame: false,
            camera_pan_this_frame: false,
            camera_orbit_this_frame: false,
            left_panel_width: ui_cfg::SIDE_PANEL_DEFAULT_WIDTH,
            right_panel_width: ui_cfg::SIDE_PANEL_DEFAULT_WIDTH,
            bottom_panel_height: ui_cfg::BOTTOM_PANEL_DEFAULT_HEIGHT,
        }
    }
}

/// Ring buffer for system log lines displayed in the bottom panel
#[derive(Resource)]
pub struct LogBuffer {
    pub lines: VecDeque<String>,
    pub capacity: usize,
    /// auto-scroll to bottom when new entries arrive
    pub auto_scroll: bool,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            lines: VecDeque::with_capacity(ui_cfg::LOG_BUFFER_CAPACITY),
            capacity: ui_cfg::LOG_BUFFER_CAPACITY,
            auto_scroll: true,
        }
    }
}

impl LogBuffer {
    pub fn push(&mut self, line: String) {
        if self.lines.len() >= self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }
}

// ── Command Publishing ───────────────────────────────────────────

/// A command originating from the UI to be published over Zenoh
pub enum OutboundCommand {
    System(SystemCommand),
    Robot(RobotControl),
    Task(TaskCommand),
}

/// Sends outbound commands to the background Zenoh publisher thread
#[derive(Resource)]
pub struct CommandSender(pub mpsc::Sender<OutboundCommand>);

// ── Queue State ──────────────────────────────────────────────────

/// Receives QueueState broadcasts from the scheduler via Zenoh
#[derive(Resource)]
pub struct QueueStateReceiver(pub mpsc::Receiver<QueueState>);

/// Latest task queue state received from the scheduler
#[derive(Resource, Default)]
pub struct QueueStateData {
    pub pending: usize,
    pub total: usize,
    pub robots_online: usize,
}

// ── Task List ────────────────────────────────────────────

/// Receives TaskListSnapshot broadcasts from the scheduler via Zenoh
#[derive(Resource)]
pub struct TaskListReceiver(pub mpsc::Receiver<TaskListSnapshot>);

/// Latest task list received from the scheduler
#[derive(Resource, Default)]
pub struct TaskListData {
    pub tasks: Vec<Task>,
    pub last_updated_secs: f64,
}

// ── Path Telemetry ───────────────────────────────────────────────

/// Receives RobotPathTelemetry broadcasts from the coordinator via Zenoh
#[derive(Resource)]
pub struct PathTelemetryReceiver(pub mpsc::Receiver<protocol::RobotPathTelemetry>);

/// Active robot paths for gizmo rendering.
/// Keys are robot IDs, values are remaining waypoints in Bevy world space (y = 0.05).
#[derive(Resource, Default)]
pub struct ActivePaths(pub HashMap<u32, Vec<bevy::math::Vec3>>);

// ── UI Events ────────────────────────────────────────────────────

/// Actions triggered by UI buttons, consumed by the command bridge system
#[derive(Message)]
pub enum UiAction {
    /// Publish SystemCommand::Pause or Resume
    SetPaused(bool),
    /// Toggle real-time mode and synchronize simulation pause state
    SetRealtime(bool),
    /// Publish RobotControl::Down(id)
    KillRobot(u32),
    /// Publish RobotControl::Restart(id)
    RestartRobot(u32),
    /// Publish RobotControl::Up(id)
    EnableRobot(u32),
    /// Publish RobotControl::Down(id) - explicitly disable a robot
    DisableRobot(u32),
    /// Publish SystemCommand::SetTimeScale(f32)
    SetTimeScale(f32),
    /// Submit a transport task to the scheduler
    SubmitTransportTask(TaskRequest),
    /// Cancel a task by ID
    CancelTask(u64),
    /// Change task priority
    ChangePriority(u64, Priority),
}
