//! Central configuration constants for all Tessera crates
//!
//! This module defines all configurable values in one place.
//! All crates should reference these instead of hardcoding values.
//!
//! ## Module Terminology
//!
//! Configuration is organized by runtime ownership:
//! - **firmware** - Robot movement and battery simulation settings
//! - **coordinator** - Path planning and task execution settings
//! - **scheduler** - Task queue and allocation settings
//! - **visualizer** - Rendering and UI tuning

/// Log directory path for storing timestamped log files
/// Log directory - absolute path from workspace root
/// Uses env var CARGO_MANIFEST_DIR at compile time for crates,
/// but falls back to relative path which works when run from workspace root
pub const LOG_DIR: &str = "logs";

/// Crate log files to exclude when merging (lowercase crate names)
pub const LOG_MERGE_EXCLUDE: &[&str] = &["firmware"];

/// Firmware layer settings
pub mod firmware {
    /// Physics simulation settings
    pub mod physics {
        /// Physics tick interval in milliseconds (20 Hz)
        pub const TICK_INTERVAL_MS: u64 = 50;

        /// Robot movement speed in world units per second
        pub const ROBOT_SPEED: f32 = 2.0;

        /// Distance threshold to consider a robot arrived at target
        pub const ARRIVAL_THRESHOLD: f32 = 0.1;

        /// Robot height offset (Y position above ground)
        pub const ROBOT_HEIGHT: f32 = 0.25;
    }

    /// Battery simulation settings
    pub mod battery {
        /// Battery drain rate (% per second while moving)
        pub const DRAIN_RATE_PER_SEC: f32 = 0.05;

        /// Low battery warning threshold (percentage)
        pub const LOW_THRESHOLD: f32 = 20.0;

        /// Minimum battery level for robot allocation (percentage)
        pub const MIN_BATTERY_FOR_TASK: f32 = 50.0;

        /// Charge rate (% per second while at station)
        pub const CHARGE_RATE: f32 = 1.0;
    }
}

/// Coordinator layer settings (path planning & task execution)
pub mod coordinator {
    /// Pathfinding strategy: "astar" or "whca" (default: "whca" for multi-robot)
    pub const PATHFINDING_STRATEGY: &str = "whca";
    
    /// Main loop sleep interval in milliseconds
    pub const LOOP_INTERVAL_MS: u64 = 10;
    
    /// Path command send interval in milliseconds (20 Hz, matching firmware physics tick)
    pub const PATH_SEND_INTERVAL_MS: u64 = 50;

    /// Path telemetry heartbeat interval in milliseconds.
    ///
    /// Coordinator sends path telemetry immediately on path change and at this
    /// interval while unchanged to keep renderer state fresh without per-tick fanout.
    pub const PATH_TELEMETRY_HEARTBEAT_MS: u64 = 1000;
    
    /// Number of upcoming waypoints to check for reservations before dispatching FollowPath.
    /// Scanning ahead prevents the firmware from blindly driving into a cell that becomes
    /// reserved while the robot is mid-segment; the coordinator has only one tick to react.
    pub const LOOKAHEAD_BLOCK_SCAN_CELLS: usize = 4;
    
    /// Map hash republish interval in seconds
    pub const MAP_HASH_REPUBLISH_SECS: u64 = 5;
    
    /// Timeout for map hash validation in seconds
    pub const MAP_VALIDATION_TIMEOUT_SECS: u64 = 15;
    
    /// Distance threshold to consider robot "arrived" at waypoint
    pub const WAYPOINT_ARRIVAL_THRESHOLD: f32 = 0.2;
    
    /// Default robot movement speed (units per second)
    pub const DEFAULT_SPEED: f32 = 2.0;
    
    /// Cargo pickup delay in seconds (time for robot to load cargo)
    pub const PICKUP_DELAY_SECS: f32 = 2.0;
    
    /// Cargo dropoff delay in seconds (time for robot to unload cargo)
    pub const DROPOFF_DELAY_SECS: f32 = 1.5;
    
    /// Task progress timeout in seconds (must see progress within this time)
    pub const TASK_TIMEOUT_SECS: u64 = 30;
    
    /// How often to check for stalled tasks (ms)
    pub const TIMEOUT_CHECK_INTERVAL_MS: u64 = 5000;

    /// WHCA* pathfinding settings
    pub mod whca {
        /// Planning window size (milliseconds to look ahead)
        pub const WINDOW_SIZE_MS: u64 = 16000;

        /// Maximum wait time before giving up (prevents infinite waits)
        pub const MAX_WAIT_TIME: u32 = 10;
        
        /// Reservation tolerance window (milliseconds)
        /// Reserves cells at predicted_time ± tolerance to handle physics jitter
        pub const RESERVATION_TOLERANCE_MS: i64 = 200;

         // Time step for planning (approximate time to move 1 cell at default speed)
        // 1 grid cell / 2.0 units/sec = 0.5 sec = 500ms
        pub const MOVE_TIME_MS: u64 = 500;

        /// How many recent tiles to keep reserved for stationary robots
        pub const STATIONARY_HISTORY_TILES: usize = 4;

        /// How long to reserve stationary tiles (milliseconds)
        pub const STATIONARY_RESERVATION_MS: u64 = 2500;

        /// Minimum interval between stationary reservation refreshes (milliseconds).
        ///
        /// Reduces per-tick reservation churn for idle/faulted robots while
        /// preserving a rolling reservation window.
        pub const STATIONARY_REFRESH_INTERVAL_MS: u64 = 500;

        /// Collision buffer radius (tiles) reserved around each reserved cell
        /// Set to 0 to avoid over-reserving narrow corridors.
        pub const COLLISION_BUFFER_TILES: usize = 0;
    }

    /// Collision detection and avoidance settings
    pub mod collision {
        /// Radius for inter-robot collision detection (world units)
        pub const ROBOT_COLLISION_RADIUS: f32 = 0.4;
        
        /// Maximum allowed path deviation before triggering replan (tiles)
        pub const MAX_PATH_DEVIATION_TILES: f32 = 2.0;
        
        /// Time without progress before transitioning to Blocked (seconds)
        pub const BLOCKED_TIMEOUT_SECS: u64 = 5;
        
        /// Maximum consecutive replan attempts before transitioning to Faulted
        pub const MAX_REPLAN_ATTEMPTS: u32 = 3;
        
        /// Backoff delay between replan attempts (milliseconds)
        pub const REPLAN_BACKOFF_MS: u64 = 2000;

        /// How long a robot may wait on a reserved cell before forcing a replan (seconds)
        pub const RESERVATION_WAIT_REPLAN_SECS: u64 = 2;

        /// How long a robot may wait before overriding reservation wait (seconds)
        /// Use to break deadlocks in tight corridors.
        pub const RESERVATION_WAIT_OVERRIDE_SECS: u64 = 8;
        
        /// Fault cleanup delay: how long to reserve the faulted position (seconds)
        /// Robot waits this duration before being reset to station
        pub const FAULT_CLEANUP_DELAY_SECS: u64 = 10;
    }
    
    /// Sensor validation and anomaly detection settings
    pub mod sensor {
        /// Maximum position change per physics tick (world units)
        /// Used to detect teleportation/chaos drift anomalies
        pub const MAX_POSITION_DELTA: f32 = 0.5;

        /// Soft limit multiplier for position jumps (allow small overshoots)
        pub const POSITION_JUMP_SOFT_LIMIT_MULT: f32 = 2.0;
        
        /// Soft limit for grid validation (allow small overshoots)
        pub const GRID_VALIDATION_SOFT_LIMIT: f32 = 0.8;
    }
}

/// Scheduler layer settings (task queue & robot allocation)
pub mod scheduler {
    /// Task queue strategy: "fifo" or others (default: "fifo")
    pub const QUEUE_STRATEGY: &str = "fifo";
    
    /// Robot allocator strategy: "closest_idle" or others (default: "closest_idle")
    pub const ALLOCATOR_STRATEGY: &str = "closest_idle";
    
    /// Main loop sleep interval in milliseconds
    pub const LOOP_INTERVAL_MS: u64 = 50;
    
    /// Queue state broadcast interval in seconds
    pub const QUEUE_BROADCAST_SECS: u64 = 2;

    /// Maximum number of active tasks included per task-list broadcast.
    ///
    /// Active includes Pending, Assigned, and InProgress tasks.
    pub const TASK_LIST_ACTIVE_WINDOW: usize = 512;

    /// Maximum number of terminal tasks included per task-list broadcast.
    ///
    /// Terminal includes Completed, Failed, and Cancelled tasks.
    pub const TASK_LIST_RECENT_TERMINAL_WINDOW: usize = 1024;

    /// Default probability for mass-add tasks to target dropoff zones.
    ///
    /// Uses 0.0..=1.0 scale where 0.60 means 60%.
    pub const MASS_ADD_DROPOFF_PROBABILITY: f32 = 0.60;

    /// Upper bound for a single mass-add request.
    ///
    /// Protects scheduler and renderer from pathological task floods.
    pub const MASS_ADD_MAX_COUNT: u32 = 10_000;

    /// Maximum number of pending tasks the allocator evaluates per scheduler tick.
    ///
    /// Keeps loop latency bounded so state broadcasts and UI updates remain responsive
    /// under very large backlogs.
    pub const ALLOCATION_TASK_BUDGET_PER_TICK: usize = 20;

    /// Maximum number of auto-retry attempts for retryable no-path assignment failures.
    pub const RETRYABLE_NO_PATH_MAX_ATTEMPTS: u32 = 3;

    /// Initial retry backoff in milliseconds for retryable no-path failures.
    pub const RETRYABLE_NO_PATH_BASE_BACKOFF_MS: u64 = 250;

    /// Maximum retry backoff in milliseconds for retryable no-path failures.
    pub const RETRYABLE_NO_PATH_MAX_BACKOFF_MS: u64 = 3000;

    /// Random jitter added to retry backoff (milliseconds).
    ///
    /// Helps avoid synchronized re-assignment storms under corridor congestion.
    pub const RETRYABLE_NO_PATH_JITTER_MS: u64 = 200;
    
    /// Location marker base for shelf encoding (S1 = SHELF_MARKER_BASE + 1)
    pub const SHELF_MARKER_BASE: usize = 10000;
    
    /// Location marker base for dropoff encoding (D1 = DROPOFF_MARKER_BASE + 1)
    pub const DROPOFF_MARKER_BASE: usize = 20000;
}

/// Warehouse simulation constants (shared by scheduler, coordinator, and visualizer)
pub mod warehouse {
    /// Maximum number of cargo items any shelf can hold.
    /// 4 levels x 4 boxes per level = 16. Shelf tokens in the layout file
    /// use single-char hex (1..F, 0=16), with legacy xN still accepted.
    /// define the *initial stock*, not the maximum capacity.
    pub const SHELF_MAX_CAPACITY: u32 = 16;
}

/// Orchestrator settings (process management)
pub mod orchestrator {
    /// Delay after starting coordinator before starting firmware (ms)
    pub const COORDINATOR_STARTUP_DELAY_MS: u64 = 1000;
    
    /// Delay after starting firmware before starting scheduler (ms)
    pub const FIRMWARE_STARTUP_DELAY_MS: u64 = 500;
    
    /// Delay after starting scheduler before starting renderer (ms)
    pub const SCHEDULER_STARTUP_DELAY_MS: u64 = 300;
    
    /// Delay after kill-all before restart (ms)
    pub const RESTART_DELAY_MS: u64 = 500;
}

/// Visualizer layer settings (rendering and UI)
pub mod visualizer {
    /// Tile size in world units
    pub const TILE_SIZE: f32 = 1.0;

    /// Lerp correction factor per render frame at 60 fps applied in interpolate_robots.
    /// Dead-reckoning closes most of the gap; this removes residual drift.
    /// 0.25 converges to <5% error within 2 frames at 60 fps.
    pub const ROBOT_LERP: f32 = 0.25;

    /// Distance above which interpolate_robots snaps instead of lerping.
    /// Catches firmware restarts and initial spawns (robots teleport to station).
    /// Must be larger than MAX_PATH_DEVIATION_TILES to avoid snap-on-deviation.
    pub const ROBOT_TELEPORT_THRESHOLD: f32 = 4.0;

    /// XZ scale applied to all wall models to close seams between adjacent tiles.
    /// 1.0 = exact model size. Increase (e.g. 1.02) if gaps are visible at junctions.
    /// Y is not scaled so wall height is unaffected. Fix models in Blender for a permanent solution.
    pub const WALL_SEAM_SCALE: f32 = 1.00;

    /// Robot mesh size
    pub const ROBOT_SIZE: f32 = 0.5;

    /// Maximum squared distance to match a robot pickup/drop to a shelf (1.5 units)
    pub const CARGO_SHELF_DISTANCE_SQ: f32 = 2.25;
    
    /// Y offset for placeholder planes (station/dropoff) to sit above the floor
    pub const PLACEHOLDER_Y_OFFSET: f32 = 0.001;

    /// Y offset for floor tiles (negative = below wall model's embedded floor plane)
    /// Fixes z-fighting between floor tiles and wall bases.
    pub const GROUND_Y_OFFSET: f32 = -0.001;

    /// Box placement
    pub mod shelf {
        /// Shelf mesh dimensions (width, height, depth)
        pub const SHELF_SIZE: (f32, f32, f32) = (0.8, 0.6, 0.8);

        /// Y-heights of the 4 shelf levels (relative to shelf origin)
        pub const SHELF_LEVEL_HEIGHTS: [f32; 4] = [0.35, 0.7, 1.05, 1.4];

        /// X offsets for the 2-column box grid per shelf level
        pub const BOX_X_OFFSETS: [f32; 2] = [-0.2, 0.2];

        /// Z offsets for the 2-row box grid per shelf level
        pub const BOX_Z_OFFSETS: [f32; 2] = [-0.2, 0.2];

        /// Scale factor for cargo boxes on shelves (1.0 = full model size)
        pub const BOX_SCALE: f32 = 0.5;
    }

    /// Robot cargo child visual settings.
    pub mod robot {
        /// visual Y offset applied to robot world transforms.
        /// use this to lift/drop the robot model without changing firmware physics.
        pub const MODEL_Y_OFFSET: f32 = -0.25;
        /// local offset for spawned cargo child box relative to robot root transform.
        pub const CARGO_CHILD_OFFSET: (f32, f32, f32) = (0.0, 0.25, 0.0);
        /// scale for spawned cargo child box model.
        pub const CARGO_CHILD_SCALE: f32 = 0.9;
    }

    pub mod path {
        /// Path color for global active routes (subtle, non-dominant)
        pub const ACTIVE_OTHER_COLOR: (f32, f32, f32) = (0.18, 0.50, 0.58);
        /// Path color for recently completed routes during fade-out
        pub const COMPLETED_COLOR: (f32, f32, f32) = (0.16, 0.30, 0.34);
        /// Path color for the selected robot's active route (bright cyan)
        /// values > 1.0 trigger the camera Bloom post-process
        pub const SELECTED_ACTIVE_COLOR: (f32, f32, f32) = super::semantic::FLOW_ACTIVE;

        /// Radius of the destination circle marker
        pub const DEST_CIRCLE_RADIUS: f32 = 0.25;

        /// Radius multiplier for selected path destination marker pulse
        pub const SELECTED_DEST_RADIUS_MULTIPLIER: f32 = 1.2;

        /// Radius multiplier for non-selected destination markers
        pub const OTHER_DEST_RADIUS_MULTIPLIER: f32 = 0.9;

        /// How long recently completed paths remain visible (seconds)
        pub const COMPLETED_FADE_SECS: f32 = 0.8;

        /// Pulse speed for selected path destination marker (radians / second)
        pub const SELECTED_PULSE_SPEED: f32 = 5.5;

        /// Pulse amplitude for selected path destination marker
        pub const SELECTED_PULSE_AMPLITUDE: f32 = 0.22;

        /// Y offset used when rendering path gizmos above the floor.
        pub const PATH_Y_OFFSET: f32 = 0.05;

        /// Gizmo line width for path trails (pixels)
        pub const LINE_WIDTH: f32 = 5.5;

        /// max active path segments rendered each frame.
        pub const MAX_SEGMENTS_PER_FRAME: usize = 2600;

        /// max completed fade segments rendered each frame.
        pub const MAX_FADE_SEGMENTS_PER_FRAME: usize = 700;
    }

    /// Semantic visual tokens used by both 3D scene and UI accents.
    pub mod semantic {
        /// Bright cyan reserved for active selected path only.
        pub const FLOW_ACTIVE: (f32, f32, f32) = (0.25, 4.4, 5.1);
        /// Accent for selected entities and key interactive states.
        pub const SELECTION: (f32, f32, f32) = (3.8, 2.4, 0.45);
        /// Alert accent for fault or critical warnings.
        pub const ALERT: (f32, f32, f32) = (2.6, 0.25, 0.25);
        /// Station placeholder marker color.
        pub const STATION_MARKER: (f32, f32, f32) = (1.0, 0.4, 0.6);
        /// Dropoff placeholder marker color.
        pub const DROPOFF_MARKER: (f32, f32, f32) = (0.0, 1.0, 0.4);
    }

    /// Outline highlighting settings (hover and selection glow)
    pub mod outline {
        /// HDR hover outline color (bright white, values > 1.0 for bloom glow)
        pub const HOVER_COLOR: (f32, f32, f32) = (5.0, 5.0, 5.0);
        /// HDR select outline color (amber accent, distinct from path cyan)
        pub const SELECT_COLOR: (f32, f32, f32) = super::semantic::SELECTION;
        /// outline width in logical pixels
        pub const WIDTH: f32 = 3.0;
    }

    /// Bloom post-process defaults and runtime control bounds.
    pub mod bloom {
        /// Whether bloom is enabled by default.
        pub const ENABLED_BY_DEFAULT: bool = true;
        /// Default bloom intensity when enabled.
        pub const DEFAULT_INTENSITY: f32 = 0.10;
        /// Minimum runtime bloom intensity.
        pub const MIN_INTENSITY: f32 = 0.0;
        /// Maximum runtime bloom intensity.
        pub const MAX_INTENSITY: f32 = 0.35;
        /// bloom prefilter threshold; values below this do not contribute to bloom.
        pub const PREFILTER_THRESHOLD: f32 = 1.0;
        /// bloom threshold softness in [0,1].
        pub const PREFILTER_THRESHOLD_SOFTNESS: f32 = 0.0;
    }

    /// Luminance and saturation controls to separate floor, walls, and shelves.
    pub mod luminance {
        /// lower albedo clamp for large static surfaces (20-80 rule).
        pub const ALBEDO_MIN: f32 = 0.10;
        /// upper albedo clamp for large static surfaces (20-80 rule).
        pub const ALBEDO_MAX: f32 = 0.90;

        /// Floor brightness multiplier.
        pub const FLOOR_BRIGHTNESS: f32 = 0.94;
        /// Wall brightness multiplier.
        pub const WALL_BRIGHTNESS: f32 = 0.82;
        /// Shelf brightness multiplier.
        pub const SHELF_BRIGHTNESS: f32 = 0.95;
        /// Cargo box brightness multiplier.
        pub const BOX_BRIGHTNESS: f32 = 1.15;

        /// Floor saturation multiplier.
        pub const FLOOR_SATURATION: f32 = 0.60;
        /// Wall saturation multiplier.
        pub const WALL_SATURATION: f32 = 0.45;
        /// Shelf saturation multiplier.
        pub const SHELF_SATURATION: f32 = 0.90;
        /// Cargo box saturation multiplier.
        pub const BOX_SATURATION: f32 = 0.78;
    }

    /// Lighting settings
    pub mod lighting {
        /// if true, skip key light and run ambient-only calibration view.
        pub const AMBIENT_ONLY_CALIBRATION: bool = false;

        /// world background color; prevents pure-black void clipping.
        pub const BACKGROUND_COLOR: (f32, f32, f32) = (0.17, 0.17, 0.19);

        /// key light intensity for depth-only shading without floor blowout.
        pub const DIRECTIONAL_ILLUMINANCE: f32 = 3_500.0;
        /// flat visibility baseline; tuned in ambient-only pass.
        pub const AMBIENT_BRIGHTNESS: f32 = 1_000.0;

        /// key light position; x and z equal to y approximates 45-degree incidence.
        pub const KEY_LIGHT_POSITION: (f32, f32, f32) = (12.0, 15.0, 12.0);
        /// key light target.
        pub const KEY_LIGHT_TARGET: (f32, f32, f32) = (0.0, 0.0, 0.0);
    }

    /// Runtime diagnostics for imported scene materials.
    pub mod diagnostics {
        /// logs imported floor/shelf material properties once after scene load.
        pub const ENABLE_IMPORT_MATERIAL_LOGS: bool = true;
        /// max floor materials logged in one run.
        pub const MAX_FLOOR_MATERIAL_LOGS: usize = 6;
        /// max shelf materials logged in one run.
        pub const MAX_SHELF_MATERIAL_LOGS: usize = 8;
    }

    /// Congestion overlay cadence and render budgets.
    pub mod overlays {
        /// seconds between overlay metric updates.
        pub const UPDATE_INTERVAL_SECS: f32 = 0.20;
        /// occupancy decay applied per update tick.
        pub const OCCUPANCY_DECAY: f32 = 0.86;
        /// occupancy increment added per robot per tick.
        pub const ROBOT_OCCUPANCY_WEIGHT: f32 = 1.0;
        /// minimum retained occupancy score.
        pub const MIN_OCCUPANCY_KEEP: f32 = 0.10;

        /// max heat tiles rendered each frame.
        pub const MAX_HEAT_TILES_PER_FRAME: usize = 140;
        /// max station/dropoff halos rendered each frame.
        pub const MAX_HALOS_PER_FRAME: usize = 20;

        /// y offset for heat/halo gizmos above floor.
        pub const OVERLAY_Y_OFFSET: f32 = 0.03;
        /// base radius for station/dropoff pressure halos.
        pub const HALO_BASE_RADIUS: f32 = 0.45;
        /// max additional halo radius from pressure.
        pub const HALO_RADIUS_GAIN: f32 = 0.55;
    }

    /// Fixed camera presets for visual regression captures.
    pub mod regression {
        /// output directory for visual regression screenshots.
        pub const SCREENSHOT_OUTPUT_DIR: &str = "logs/screenshots";
        /// file extension for saved screenshots.
        pub const SCREENSHOT_FILE_EXTENSION: &str = "png";
        /// idle baseline scenario camera (focus xyz, radius, pitch, yaw).
        pub const PRESET_IDLE: ((f32, f32, f32), f32, f32, f32) = ((12.0, 0.0, 6.0), 27.0, 0.82, 0.0);
        /// congestion scenario camera (focus xyz, radius, pitch, yaw).
        pub const PRESET_CONGESTION: ((f32, f32, f32), f32, f32, f32) = ((24.0, 0.0, 8.0), 20.0, 0.90, -0.52);
        /// active routing scenario camera (focus xyz, radius, pitch, yaw).
        pub const PRESET_ROUTING: ((f32, f32, f32), f32, f32, f32) = ((15.0, 0.0, 9.0), 19.0, 0.88, 0.34);
        /// shelf inspection scenario camera (focus xyz, radius, pitch, yaw).
        pub const PRESET_SHELF: ((f32, f32, f32), f32, f32, f32) = ((13.0, 0.0, 9.0), 12.0, 1.02, 0.18);
    }

    /// Camera defaults
    pub mod camera {
        /// Camera focus point (center of view)
        pub const DEFAULT_FOCUS: (f32, f32, f32) = (12.0, 0.0, 6.0);
        pub const DEFAULT_RADIUS: f32 = 25.0;
        pub const DEFAULT_PITCH: f32 = 0.8; // ~45 degrees
        pub const DEFAULT_YAW: f32 = 0.0;

        /// Pitch limits (radians) - prevents camera flipping
        pub const PITCH_MIN: f32 = 0.1;
        pub const PITCH_MAX: f32 = 1.5;

        /// Zoom limits (radius)
        pub const ZOOM_MIN: f32 = 5.0;
        pub const ZOOM_MAX: f32 = 100.0;

        /// Radius the camera zooms to when following an entity
        pub const FOLLOW_ZOOM_RADIUS: f32 = 12.0;
        /// Lerp factor for camera focus tracking (0.0 = no movement, 1.0 = instant)
        pub const FOLLOW_FOCUS_LERP: f32 = 0.15;
        /// Lerp factor for radius zoom-in while following
        pub const FOLLOW_ZOOM_LERP: f32 = 0.08;

        /// Radius the camera uses when following a task's cargo / robot
        pub const TASK_FOLLOW_ZOOM_RADIUS: f32 = 18.0;
        /// Lerp factor for returning to default view on task deselect
        pub const DEFAULT_RESET_LERP: f32 = 0.05;

        /// Mouse orbit sensitivity (radians per pixel)
        pub const ORBIT_SENSITIVITY: f32 = 0.005;
        /// Mouse pan sensitivity (world units per pixel)
        pub const PAN_SENSITIVITY: f32 = 0.05;
        /// Scroll zoom speed (units per scroll line)
        pub const SCROLL_LINE_SPEED: f32 = 2.0;
        /// Scroll zoom speed (units per pixel for trackpad)
        pub const SCROLL_PIXEL_SPEED: f32 = 0.1;
    }

    /// UI panel layout settings
    pub mod ui {
        /// Top HUD bar height
        pub const TOP_PANEL_HEIGHT: f32 = 36.0;
        /// Side panel default width (left and right)
        pub const SIDE_PANEL_DEFAULT_WIDTH: f32 = 280.0;
        /// Side panel minimum width
        pub const SIDE_PANEL_MIN_WIDTH: f32 = 200.0;
        /// Side panel maximum width
        pub const SIDE_PANEL_MAX_WIDTH: f32 = 400.0;
        /// Bottom panel default height
        pub const BOTTOM_PANEL_DEFAULT_HEIGHT: f32 = 180.0;
        /// Bottom panel minimum height
        pub const BOTTOM_PANEL_MIN_HEIGHT: f32 = 80.0;
        /// Bottom panel maximum height
        pub const BOTTOM_PANEL_MAX_HEIGHT: f32 = 400.0;
        /// Log buffer ring capacity
        pub const LOG_BUFFER_CAPACITY: usize = 512;

        /// Number of task rows rendered per page in the Tasks tab.
        pub const TASK_LIST_PAGE_SIZE: usize = 50;

        /// Minimap palette and highlight tokens used by visualizer inspector/task widgets.
        pub mod minimap {
            /// base wall tile gray value.
            pub const WALL_GRAY: u8 = 35;
            /// base ground tile gray value.
            pub const GROUND_GRAY: u8 = 70;
            /// base empty tile gray value.
            pub const EMPTY_GRAY: u8 = 15;
            /// gray for empty shelf tiles in pickup mode.
            pub const SHELF_EMPTY_GRAY: u8 = 45;
            /// gray for shelf fallback when fill data is missing.
            pub const SHELF_UNKNOWN_GRAY: u8 = 55;
            /// gray for source shelf marker in relocation minimap.
            pub const SOURCE_SHELF_GRAY: u8 = 90;

            /// station tile RGB color.
            pub const STATION: (u8, u8, u8) = (100, 40, 60);
            /// dropoff tile RGB color.
            pub const DROPOFF: (u8, u8, u8) = (20, 130, 70);
            /// base shelf tile RGB color used when capacity overlay is disabled.
            pub const SHELF_BASE: (u8, u8, u8) = (60, 100, 60);

            /// pickup selection fill RGB color.
            pub const PICKUP_FILL: (u8, u8, u8) = (60, 120, 220);
            /// dropoff selection fill RGB color.
            pub const DROPOFF_FILL: (u8, u8, u8) = (50, 190, 100);
            /// pickup outline RGB color.
            pub const PICKUP_OUTLINE: (u8, u8, u8) = (120, 180, 255);
            /// dropoff outline RGB color.
            pub const DROPOFF_OUTLINE: (u8, u8, u8) = (100, 240, 150);
            /// hover outline RGB color.
            pub const HOVER_OUTLINE: (u8, u8, u8) = (255, 255, 255);
        }
    }

    /// Overhead robot label settings
    pub mod labels {
        /// World-unit Y offset above robot mesh top for the label anchor point
        pub const Y_OFFSET: f32 = 0.45;

        /// Seconds since last received update before a robot is shown as offline
        pub const OFFLINE_TIMEOUT_SECS: f32 = 3.0;

        /// egui font size for the small robot ID (`#3`)
        /// tune this value to adjust default label size (see also ICON_SIZE below)
        pub const FONT_SIZE: f32 = 8.0;

        /// egui font size for the large goal/status icon
        /// tune this value to adjust default label size
        pub const ICON_SIZE: f32 = 11.0;

        /// Compact-tier font size used at medium/far zoom.
        pub const COMPACT_FONT_SIZE: f32 = 9.0;

        /// Maximum world-space distance for full label tier.
        pub const FULL_TIER_MAX_DISTANCE: f32 = 16.0;

        /// Maximum world-space distance for compact label tier.
        /// Robots farther than this are hidden unless selected/hovered.
        pub const COMPACT_TIER_MAX_DISTANCE: f32 = 34.0;

        /// Maximum number of non-forced labels rendered each frame.
        pub const MAX_LABELS_PER_FRAME: usize = 42;

        /// Screen-space bucket size in logical pixels for cluster badges.
        pub const CLUSTER_BUCKET_PX: f32 = 110.0;

        /// Minimum hidden robot count before a cluster badge is drawn.
        pub const CLUSTER_MIN_COUNT: usize = 3;

        /// Label background color (fully opaque dark)
        pub const BG_COLOR: (u8, u8, u8, u8) = (18, 18, 18, 245);

        /// Border stroke width (logical pixels) drawn in the state color
        pub const STROKE_WIDTH: f32 = 1.5;

        /// Label frame corner radius
        pub const CORNER_RADIUS: f32 = 4.0;

        /// Label frame corner radius for compact label chips.
        pub const COMPACT_CORNER_RADIUS: f32 = 3.0;

        /// Horizontal inner padding (logical pixels)
        pub const PADDING_H: f32 = 5.0;

        /// Vertical inner padding (logical pixels)
        pub const PADDING_V: f32 = 3.0;

        // state colors (R, G, B) in 0-255 range for egui
        /// Faulted / collision
        pub const COLOR_FAULTED: (u8, u8, u8) = (220, 60, 60);
        /// Low battery warning
        pub const COLOR_LOW_BATT: (u8, u8, u8) = (255, 160, 30);
        /// Blocked / rerouting
        pub const COLOR_BLOCKED: (u8, u8, u8) = (80, 130, 255);
        /// Charging at station
        pub const COLOR_CHARGING: (u8, u8, u8) = (60, 210, 100);
        /// No updates received (offline / link dead)
        pub const COLOR_OFFLINE: (u8, u8, u8) = (130, 130, 130);
        /// Actively picking cargo
        pub const COLOR_PICKING: (u8, u8, u8) = (255, 210, 60);
        /// Normal operation (moving, idle)
        pub const COLOR_NORMAL: (u8, u8, u8) = (200, 200, 200);

        /// Executing movement/delivery state.
        pub const COLOR_EXECUTING: (u8, u8, u8) = (120, 205, 255);

        /// Label pulse frequency for faulted state.
        pub const PULSE_FAULT_HZ: f32 = 2.2;
        /// Label pulse amplitude for faulted state.
        pub const PULSE_FAULT_AMPLITUDE: f32 = 0.36;

        /// Label pulse frequency for blocked state.
        pub const PULSE_BLOCKED_HZ: f32 = 1.7;
        /// Label pulse amplitude for blocked state.
        pub const PULSE_BLOCKED_AMPLITUDE: f32 = 0.24;

        /// Label pulse frequency for charging state.
        pub const PULSE_CHARGING_HZ: f32 = 0.85;
        /// Label pulse amplitude for charging state.
        pub const PULSE_CHARGING_AMPLITUDE: f32 = 0.12;

        /// Label pulse frequency for low battery warning state.
        pub const PULSE_LOW_BATT_HZ: f32 = 1.1;
        /// Label pulse amplitude for low battery warning state.
        pub const PULSE_LOW_BATT_AMPLITUDE: f32 = 0.16;

        /// Label pulse frequency for normal/idle state.
        pub const PULSE_NORMAL_HZ: f32 = 0.0;
        /// Label pulse amplitude for normal/idle state.
        pub const PULSE_NORMAL_AMPLITUDE: f32 = 0.0;
    }
}

/// Notification sound settings
pub mod notify {
    /// Default arpeggio note sequence: (frequency_hz, duration_ms).
    /// Played by `notifier::play_default()` after a successful build.
    /// Edit to change the melody — any frequency/duration pairs are valid.
    pub const DEFAULT_SEQUENCE: &[(f32, u64)] = &[
        (523.25, 110), // C5
        (659.25, 110), // E5
        (783.99, 110), // G5
        (1046.50, 280), // C6
    ];

    /// Volume for synthesized notes (0.0 – 1.0).
    /// Pure sine waves are perceptually loud; keep this low.
    pub const AMPLITUDE: f32 = 0.05;
}

/// Chaos testing settings - inject faults to test system resilience
/// 
/// Set ENABLED = true to activate chaos engineering.
/// Each feature can be individually toggled and tuned.
pub mod chaos {
    /// Master switch for all chaos features
    pub const ENABLED: bool = false;
    
    // ============ Network Chaos ============
    
    /// Probability of dropping outgoing messages (0.0 = never, 1.0 = always)
    pub const PACKET_LOSS_ENABLED: bool = true;
    pub const PACKET_LOSS_RATE: f32 = 0.05;
    
    /// Random delay range for messages in milliseconds (min, max)
    pub const MESSAGE_DELAY_ENABLED: bool = true;
    pub const MESSAGE_DELAY_MS: (u64, u64) = (0, 100);
    
    // ============ Firmware Chaos ============
    
    /// Probability of ignoring/rejecting a command (0.0 = never, 1.0 = always)
    pub const COMMAND_REJECT_ENABLED: bool = true;
    pub const COMMAND_REJECT_RATE: f32 = 0.02;
    
    /// Probability of sending stale/old state data
    pub const STALE_STATE_ENABLED: bool = true;
    pub const STALE_STATE_RATE: f32 = 0.03;
    
    /// Position drift range per update in world units (simulates odometry errors)
    pub const POSITION_DRIFT_ENABLED: bool = false;
    pub const POSITION_DRIFT_RANGE: f32 = 0.02;
    
    // ============ Battery Chaos ============
    
    /// Probability of battery sensor glitch (false low reading)
    pub const BATTERY_GLITCH_ENABLED: bool = true;
    pub const BATTERY_GLITCH_RATE: f32 = 0.01;
    
    // ============ System Chaos ============
    
    /// Probability of process self-termination per loop iteration
    /// Enable crash chaos (BE CAREFUL - this will kill processes!)
    pub const CRASH_ENABLED: bool = false;
    pub const CRASH_PROBABILITY: f32 = 0.0001;
}

/// Performance vs visual-quality trade-off toggles.
///
/// All flags default to `true` (optimization active). Set to `false` for better
/// visuals when hardware allows.
pub mod optimization {
    /// Disable shadow map generation on the directional light.
    /// the shadow map must re-render every tile on every frame.
    /// costly but improves visuals tenfold by adding depth.
    pub const DISABLE_DIRECTIONAL_SHADOWS: bool = false;

    /// Mark floor and wall SceneRoot entities with `Pickable::IGNORE` so the
    /// picking backend skips event dispatch over non-interactive tiles.
    /// Note: eliminating the per-mesh raycast cost also requires a
    /// SceneInstanceReady propagation system to reach child meshes.
    /// TODO: wire up child-mesh propagation for full raycast exclusion.
    pub const DISABLE_TILE_PICKING: bool = true;

    /// Mark floor and wall child meshes as `NotShadowCaster`.
    /// Requires a SceneInstanceReady propagation system to reach child meshes.
    /// TODO: implement propagation system; for now use DISABLE_DIRECTIONAL_SHADOWS.
    pub const DISABLE_TILE_SHADOW_CAST: bool = false;

    /// Mark floor tile child meshes as `NotShadowReceiver`.
    /// Requires a SceneInstanceReady propagation system to reach child meshes.
    /// TODO: implement propagation system.
    pub const DISABLE_FLOOR_SHADOW_RECEIVE: bool = false;
}
