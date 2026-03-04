//! Resources for the Visualizer crate
//!
//! Render layer + Digital Twin Command Center.
//! Subscribes to robot updates, task queue state, and system commands.
//! Publishes control commands (pause/resume, robot control) from the UI.

use bevy::prelude::*;
use protocol::config::visual::ui as ui_cfg;
use protocol::{QueueState, RobotControl, RobotUpdate, SystemCommand, TaskRequest};
use std::collections::{HashMap, VecDeque};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use zenoh::Session;

/// Shared Zenoh session for all visualizer subscribers
#[derive(Resource, Clone)]
pub struct ZenohSession(pub Session);

/// Open a single Zenoh session for the visualizer (blocking startup).
///
/// This avoids multiple sessions per process and keeps the visualizer lean.
pub fn open_zenoh_session() -> ZenohSession {
    let rt = Runtime::new().expect("Failed to create Tokio runtime for Zenoh session");
    let session = rt.block_on(async {
        zenoh::open(zenoh::Config::default())
            .await
            .expect("Failed to open Zenoh session")
    });

    ZenohSession(session)
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

/// Tracks the last seen position for each robot (for movement detection in zenoh_receiver)
/// Prevents processing duplicate updates when robot hasn't moved
#[derive(Resource, Default)]
pub struct RobotLastPositions {
    pub by_id: HashMap<u32, [f32; 3]>,
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
    /// Whether the simulation is paused
    pub is_paused: bool,
    /// Real-time mode toggle (true = real hardware, false = simulation)
    pub is_realtime: bool,
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
            is_paused: false,
            is_realtime: false,
            object_tab: LeftTab::default(),
            bottom_tab: BottomTab::default(),
            inspector_tab: RightTab::default(),
            camera_following: false,
            transport_dropdown_open: false,
            transport_shelves_expanded: false,
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
    Task(TaskRequest),
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

// ── UI Events ────────────────────────────────────────────────────

/// Actions triggered by UI buttons, consumed by the command bridge system
#[derive(Message)]
pub enum UiAction {
    /// Publish SystemCommand::Pause or Resume
    SetPaused(bool),
    /// Publish RobotControl::Down(id)
    KillRobot(u32),
    /// Publish RobotControl::Restart(id)
    RestartRobot(u32),
    /// Publish RobotControl::Up(id)
    EnableRobot(u32),
    /// Submit a transport task to the scheduler
    SubmitTransportTask(TaskRequest),
}
