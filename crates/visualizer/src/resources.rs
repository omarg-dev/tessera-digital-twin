//! Resources for the Visualizer crate
//!
//! Render layer + Digital Twin Command Center.
//! Subscribes to robot updates, task queue state, and system commands.
//! Publishes control commands (pause/resume, robot control) from the UI.

use bevy::prelude::*;
use protocol::config::visualizer::{bloom as bloom_cfg, network as net_cfg, ui as ui_cfg};
use protocol::grid_map::GridMap;
use protocol::{Priority, QueueState, RobotControl, RobotUpdate, RobotUpdateBatch, SystemCommand, Task, TaskCommand, TaskListSnapshot, TaskRequest, WhcaMetricsTelemetry};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
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
pub struct ZenohReceiver(pub mpsc::Receiver<RobotUpdateBatch>);

/// Stores latest robot updates for systems to consume
#[derive(Resource, Default)]
pub struct RobotUpdates {
    pub updates: Vec<RobotUpdate>,
    pub last_batch_tick: Option<u64>,
}

/// Fast lookup for robot entities by ID
#[derive(Resource, Default)]
pub struct RobotIndex {
    pub by_id: HashMap<u32, Entity>,
}

impl RobotIndex {
    /// Return the Bevy entity for a robot ID, if present.
    pub fn get_entity(&self, robot_id: u32) -> Option<Entity> {
        self.by_id.get(&robot_id).copied()
    }
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

/// Shared mesh+material handles for placeholder entities (station, dropoff).
/// Avoids creating duplicate GPU assets per entity.
#[derive(Resource, Clone)]
pub struct PlaceholderMeshes {
    pub station_mesh: Handle<Mesh>,
    pub station_material: Handle<StandardMaterial>,
    pub dropoff_mesh: Handle<Mesh>,
    pub dropoff_material: Handle<StandardMaterial>,
}

/// Fixed camera viewpoints for visual regression snapshots.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub enum CameraPreset {
    #[default]
    Idle,
    Congestion,
    Routing,
    Shelf,
}

impl CameraPreset {
    pub fn label(self) -> &'static str {
        match self {
            CameraPreset::Idle => "Idle",
            CameraPreset::Congestion => "Congestion",
            CameraPreset::Routing => "Routing",
            CameraPreset::Shelf => "Shelf",
        }
    }
}

/// Per-frame render counters used for analytics and budget tracking.
#[derive(Resource, Default, Clone)]
pub struct RenderPerfCounters {
    pub labels_drawn: usize,
    pub labels_hidden_tier: usize,
    pub labels_hidden_budget: usize,
    pub path_segments_drawn: usize,
    pub paths_faded_drawn: usize,
    pub overlay_tiles_drawn: usize,
    pub overlay_halos_drawn: usize,
    pub overlay_updates: u64,
    pub path_telemetry_messages_processed: u32,
    pub path_telemetry_unique_robots: u32,
    pub path_telemetry_total_waypoints: u32,
    pub path_telemetry_max_waypoints_single: u32,
}

/// UI-facing snapshot of render counters and screenshot markers.
#[derive(Resource, Default, Clone)]
pub struct UiAnalyticsView {
    pub perf: RenderPerfCounters,
    pub snapshot_markers: VecDeque<String>,
    pub backpressure: BackpressureSnapshot,
}

#[derive(Clone, Default)]
pub struct ChannelBackpressureHandle {
    received: Arc<AtomicU64>,
    enqueued: Arc<AtomicU64>,
    dropped_full: Arc<AtomicU64>,
    blocked_send: Arc<AtomicU64>,
}

impl ChannelBackpressureHandle {
    pub fn on_received(&self) {
        self.received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_enqueued(&self) {
        self.enqueued.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_dropped_full(&self) {
        self.dropped_full.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_blocked_send(&self) {
        self.blocked_send.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> (u64, u64, u64, u64) {
        (
            self.received.load(Ordering::Relaxed),
            self.enqueued.load(Ordering::Relaxed),
            self.dropped_full.load(Ordering::Relaxed),
            self.blocked_send.load(Ordering::Relaxed),
        )
    }
}

#[derive(Clone, Copy, Default)]
pub struct ChannelBackpressureSnapshot {
    pub received: u64,
    pub enqueued: u64,
    pub dropped_full: u64,
    pub blocked_send: u64,
    pub queue_len: usize,
    pub queue_peak: usize,
}

#[derive(Clone)]
pub struct ChannelBackpressure {
    pub snapshot: ChannelBackpressureSnapshot,
    pub warn_queue_depth: usize,
    pub last_warning_secs: f64,
    pub last_warned_dropped_full: u64,
    pub last_warned_blocked_send: u64,
    handle: ChannelBackpressureHandle,
}

impl ChannelBackpressure {
    pub fn new(warn_queue_depth: usize) -> Self {
        Self {
            snapshot: ChannelBackpressureSnapshot::default(),
            warn_queue_depth,
            last_warning_secs: 0.0,
            last_warned_dropped_full: 0,
            last_warned_blocked_send: 0,
            handle: ChannelBackpressureHandle::default(),
        }
    }

    pub fn handle(&self) -> ChannelBackpressureHandle {
        self.handle.clone()
    }

    pub fn refresh_from_handle(&mut self) {
        let (received, enqueued, dropped_full, blocked_send) = self.handle.snapshot();
        self.snapshot.received = received;
        self.snapshot.enqueued = enqueued;
        self.snapshot.dropped_full = dropped_full;
        self.snapshot.blocked_send = blocked_send;
    }

    pub fn record_queue_depth(&mut self, queue_len: usize) {
        self.snapshot.queue_len = queue_len;
        self.snapshot.queue_peak = self.snapshot.queue_peak.max(queue_len);
    }
}

#[derive(Clone, Default)]
pub struct BackpressureSnapshot {
    pub robot_updates: ChannelBackpressureSnapshot,
    pub queue_state: ChannelBackpressureSnapshot,
    pub task_list: ChannelBackpressureSnapshot,
    pub path_telemetry: ChannelBackpressureSnapshot,
    pub whca_metrics: ChannelBackpressureSnapshot,
    pub command_bridge: ChannelBackpressureSnapshot,
}

#[derive(Resource, Clone)]
pub struct BackpressureMetrics {
    pub robot_updates: ChannelBackpressure,
    pub queue_state: ChannelBackpressure,
    pub task_list: ChannelBackpressure,
    pub path_telemetry: ChannelBackpressure,
    pub whca_metrics: ChannelBackpressure,
    pub command_bridge: ChannelBackpressure,
}

impl Default for BackpressureMetrics {
    fn default() -> Self {
        Self {
            robot_updates: ChannelBackpressure::new(net_cfg::ROBOT_UPDATES_WARN_QUEUE_DEPTH),
            queue_state: ChannelBackpressure::new(net_cfg::QUEUE_STATE_WARN_QUEUE_DEPTH),
            task_list: ChannelBackpressure::new(net_cfg::TASK_LIST_WARN_QUEUE_DEPTH),
            path_telemetry: ChannelBackpressure::new(net_cfg::PATH_TELEMETRY_WARN_QUEUE_DEPTH),
            whca_metrics: ChannelBackpressure::new(net_cfg::WHCA_METRICS_WARN_QUEUE_DEPTH),
            command_bridge: ChannelBackpressure::new(net_cfg::COMMAND_WARN_QUEUE_DEPTH),
        }
    }
}

impl BackpressureMetrics {
    pub fn refresh_from_handles(&mut self) {
        self.robot_updates.refresh_from_handle();
        self.queue_state.refresh_from_handle();
        self.task_list.refresh_from_handle();
        self.path_telemetry.refresh_from_handle();
        self.whca_metrics.refresh_from_handle();
        self.command_bridge.refresh_from_handle();
    }

    pub fn snapshot(&self) -> BackpressureSnapshot {
        BackpressureSnapshot {
            robot_updates: self.robot_updates.snapshot,
            queue_state: self.queue_state.snapshot,
            task_list: self.task_list.snapshot,
            path_telemetry: self.path_telemetry.snapshot,
            whca_metrics: self.whca_metrics.snapshot,
            command_bridge: self.command_bridge.snapshot,
        }
    }

    fn warn_channel(
        name: &str,
        channel: &mut ChannelBackpressure,
        now_secs: f64,
        log_buffer: &mut LogBuffer,
    ) {
        if now_secs - channel.last_warning_secs < net_cfg::WARNING_COOLDOWN_SECS {
            return;
        }

        let queue_hot = channel.snapshot.queue_len >= channel.warn_queue_depth;
        let dropped_delta = channel
            .snapshot
            .dropped_full
            .saturating_sub(channel.last_warned_dropped_full);
        let blocked_delta = channel
            .snapshot
            .blocked_send
            .saturating_sub(channel.last_warned_blocked_send);

        if !queue_hot
            && dropped_delta < net_cfg::COMMAND_WARN_DROP_DELTA
            && blocked_delta == 0
        {
            return;
        }

        log_buffer.push(format!(
            "[Net] {} pressure: queue={}/{} peak={} dropped_full={} blocked_send={}",
            name,
            channel.snapshot.queue_len,
            channel.warn_queue_depth,
            channel.snapshot.queue_peak,
            channel.snapshot.dropped_full,
            channel.snapshot.blocked_send,
        ));

        channel.last_warning_secs = now_secs;
        channel.last_warned_dropped_full = channel.snapshot.dropped_full;
        channel.last_warned_blocked_send = channel.snapshot.blocked_send;
    }

    pub fn maybe_push_warnings(&mut self, now_secs: f64, log_buffer: &mut LogBuffer) {
        Self::warn_channel("robot_updates", &mut self.robot_updates, now_secs, log_buffer);
        Self::warn_channel("path_telemetry", &mut self.path_telemetry, now_secs, log_buffer);
        Self::warn_channel("queue_state", &mut self.queue_state, now_secs, log_buffer);
        Self::warn_channel("task_list", &mut self.task_list, now_secs, log_buffer);
        Self::warn_channel("whca_metrics", &mut self.whca_metrics, now_secs, log_buffer);
        Self::warn_channel("command_bridge", &mut self.command_bridge, now_secs, log_buffer);
    }
}

/// Per-frame UI input snapshot to keep egui system params compact.
#[derive(Resource, Default, Clone, Copy)]
pub struct UiFrameInputs {
    pub delta_secs: f32,
    pub left_click_just_pressed: bool,
}

/// Congestion heat state updated at a throttled cadence.
#[derive(Resource, Default)]
pub struct CongestionOverlayData {
    pub tile_occupancy: HashMap<(usize, usize), f32>,
    pub update_accum_secs: f32,
    pub total_updates: u64,
}

/// Lightweight snapshot log for visual regression captures.
#[derive(Resource)]
pub struct ScreenshotHarness {
    pub records: VecDeque<String>,
    pub paths: VecDeque<String>,
    pub max_records: usize,
}

impl Default for ScreenshotHarness {
    fn default() -> Self {
        Self {
            records: VecDeque::with_capacity(32),
            paths: VecDeque::with_capacity(32),
            max_records: 32,
        }
    }
}

impl ScreenshotHarness {
    pub fn push(&mut self, line: String) {
        if self.records.len() >= self.max_records {
            self.records.pop_front();
        }
        self.records.push_back(line);
    }

    pub fn push_path(&mut self, line: String) {
        if self.paths.len() >= self.max_records {
            self.paths.pop_front();
        }
        self.paths.push_back(line);
    }
}

/// Runtime visual tuning state shared across camera and render systems.
#[derive(Resource)]
pub struct VisualTuning {
    pub bloom_enabled: bool,
    pub bloom_intensity: f32,
}

impl Default for VisualTuning {
    fn default() -> Self {
        Self {
            bloom_enabled: bloom_cfg::ENABLED_BY_DEFAULT,
            bloom_intensity: bloom_cfg::DEFAULT_INTENSITY,
        }
    }
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
    /// Label density mode: use compact labels outside near tier.
    pub compact_labels: bool,
    /// Show cluster count badges for hidden far/budget-limited labels.
    pub cluster_badges: bool,
    /// Layer toggle: enable bloom post-process
    pub bloom_enabled: bool,
    /// Runtime bloom intensity
    pub bloom_intensity: f32,
    /// selected camera preset for regression screenshot harness.
    pub camera_preset: CameraPreset,
    /// set when UI requests camera to snap to selected preset.
    pub camera_preset_dirty: bool,
    /// mark baseline capture for current preset.
    pub snapshot_mark_baseline: bool,
    /// mark after/variant capture for current preset.
    pub snapshot_mark_after: bool,
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
    /// whether the mass-add form is visible in the tasks panel
    pub mass_add_form_open: bool,
    /// text input for requested mass-add task count
    pub mass_add_count_input: String,
    /// text input for optional mass-add dropoff percentage
    pub mass_add_dropoff_pct_input: String,
    /// selected task ID for the Inspector panel (None = no task selected)
    pub selected_task_id: Option<u64>,
    /// current page in Active tasks section (0-based)
    pub task_page_active: usize,
    /// current page in Failed tasks section (0-based)
    pub task_page_failed: usize,
    /// current page in Completed tasks section (0-based)
    pub task_page_completed: usize,
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
    /// Cached robot count used to invalidate sorted entity cache in Objects tab.
    pub object_cache_robot_count: usize,
    /// Cached shelf count used to invalidate sorted entity cache in Objects tab.
    pub object_cache_shelf_count: usize,
    /// Sorted robot entity cache for Objects tab rendering.
    pub object_sorted_robot_entities: Vec<Entity>,
    /// Sorted shelf entity cache for Objects tab rendering.
    pub object_sorted_shelf_entities: Vec<Entity>,
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
            compact_labels: true,
            cluster_badges: false,
            bloom_enabled: bloom_cfg::ENABLED_BY_DEFAULT,
            bloom_intensity: bloom_cfg::DEFAULT_INTENSITY,
            camera_preset: CameraPreset::default(),
            camera_preset_dirty: false,
            snapshot_mark_baseline: false,
            snapshot_mark_after: false,
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
            mass_add_form_open: false,
            mass_add_count_input: String::new(),
            mass_add_dropoff_pct_input: String::new(),
            selected_task_id: None,
            task_page_active: 0,
            task_page_failed: 0,
            task_page_completed: 0,
            camera_scroll_this_frame: false,
            camera_pan_this_frame: false,
            camera_orbit_this_frame: false,
            left_panel_width: ui_cfg::SIDE_PANEL_DEFAULT_WIDTH,
            right_panel_width: ui_cfg::SIDE_PANEL_DEFAULT_WIDTH,
            bottom_panel_height: ui_cfg::BOTTOM_PANEL_DEFAULT_HEIGHT,
            object_cache_robot_count: 0,
            object_cache_shelf_count: 0,
            object_sorted_robot_entities: Vec::new(),
            object_sorted_shelf_entities: Vec::new(),
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

/// Latest bounded task-list window received from the scheduler
#[derive(Resource, Default)]
pub struct TaskListData {
    /// combined active + recent terminal window used by task details/camera follow
    pub tasks: Vec<Task>,
    /// total active task count on scheduler
    pub active_total: usize,
    /// total completed task count on scheduler
    pub completed_total: usize,
    /// total failed task count on scheduler
    pub failed_total: usize,
    /// total cancelled task count on scheduler
    pub cancelled_total: usize,
    pub last_updated_secs: f64,
}

impl TaskListData {
    /// Failed-like total used by the UI failed bucket (Failed + Cancelled).
    pub fn failed_like_total(&self) -> usize {
        self.failed_total + self.cancelled_total
    }
}

// ── Path Telemetry ───────────────────────────────────────────────

/// Receives RobotPathTelemetry broadcasts from the coordinator via Zenoh
#[derive(Resource)]
pub struct PathTelemetryReceiver(pub mpsc::Receiver<protocol::RobotPathTelemetry>);

/// Active robot paths for gizmo rendering.
/// Keys are robot IDs, values are remaining waypoints in Bevy world space.
#[derive(Resource, Default)]
pub struct ActivePaths(pub HashMap<u32, Vec<bevy::math::Vec3>>);

// ── WHCA Metrics Telemetry ──────────────────────────────────────

/// Receives WHCA metrics telemetry from coordinator via Zenoh.
#[derive(Resource)]
pub struct WhcaMetricsReceiver(pub mpsc::Receiver<WhcaMetricsTelemetry>);

/// Latest WHCA metrics telemetry for analytics UI.
#[derive(Resource, Default)]
pub struct WhcaMetricsData {
    pub latest: Option<WhcaMetricsTelemetry>,
    pub last_updated_secs: f64,
}

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
    /// Submit a mass-add request to the scheduler
    MassAddTasks {
        count: u32,
        dropoff_probability: Option<f32>,
    },
    /// Cancel a task by ID
    CancelTask(u64),
    /// Change task priority
    ChangePriority(u64, Priority),
    /// Toggle bloom and set intensity for runtime A/B checks
    SetBloom {
        enabled: bool,
        intensity: f32,
    },
}
