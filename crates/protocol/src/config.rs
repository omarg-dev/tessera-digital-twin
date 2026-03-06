//! Central configuration constants for all Hyper-Twin crates
//! 
//! This module defines all configurable values in one place.
//! All crates should reference these instead of hardcoding values.
//!
//! ## Layer Terminology
//! 
//! Configuration is organized by abstraction layer:
//! - **physics** - Robot movement and simulation timing
//! - **battery** - Battery drain and charging rates  
//! - **coordinator** - Path planning and task execution settings
//! - **scheduler** - Task queue and allocation settings
//! - **renderer** - Visualization dimensions and colors

/// Path to the warehouse layout file (relative to workspace root)
pub const LAYOUT_FILE_PATH: &str = "assets/data/layout2.txt";

/// Log directory path for storing timestamped log files
/// Log directory - absolute path from workspace root
/// Uses env var CARGO_MANIFEST_DIR at compile time for crates,
/// but falls back to relative path which works when run from workspace root
pub const LOG_DIR: &str = "logs";

/// Crate log files to exclude when merging (lowercase crate names)
pub const LOG_MERGE_EXCLUDE: &[&str] = &["firmware"];

/// Physics simulation settings
pub mod physics {
    /// Physics tick interval in milliseconds (20 Hz)
    pub const TICK_INTERVAL_MS: u64 = 50;
    
    /// Robot movement speed: units per second
    pub const ROBOT_SPEED: f32 = 2.0;
    
    /// Arrival threshold: distance to consider "arrived" at target
    pub const ARRIVAL_THRESHOLD: f32 = 0.1;
    
    /// Robot height offset (Y position above ground)
    pub const ROBOT_HEIGHT: f32 = 0.25;
}

/// Battery settings
pub mod battery {
    /// Battery drain rate: % per second while moving (random in range)
    pub const DRAIN_RATE_RANGE: (f32, f32) = (0.03, 0.07);
    
    /// Low battery warning threshold (percentage)
    pub const LOW_THRESHOLD: f32 = 20.0;

    /// Minimum battery level for robot allocation (percentage)
    pub const MIN_BATTERY_FOR_TASK: f32 = 50.0;
    
    /// Charge rate: % per second while at station
    pub const CHARGE_RATE: f32 = 1.0;
}

/// Coordinator layer settings (path planning & task execution)
pub mod coordinator {
    /// Pathfinding strategy: "astar" or "whca" (default: "whca" for multi-robot)
    pub const PATHFINDING_STRATEGY: &str = "whca";
    
    /// Main loop sleep interval in milliseconds
    pub const LOOP_INTERVAL_MS: u64 = 10;
    
    /// Path command send interval in milliseconds (10 Hz)
    pub const PATH_SEND_INTERVAL_MS: u64 = 100;
    
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
        pub const STATIONARY_HISTORY_TILES: usize = 2;

        /// How long to reserve stationary tiles (milliseconds)
        pub const STATIONARY_RESERVATION_MS: u64 = 1500;

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
        pub const RESERVATION_WAIT_REPLAN_SECS: u64 = 3;

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
        
        /// Tolerance for position validation against grid (world units)
        /// Robot center can be this far from grid cell center
        pub const GRID_VALIDATION_TOLERANCE: f32 = 0.6;

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
    
    /// Location marker base for shelf encoding (S1 = SHELF_MARKER_BASE + 1)
    pub const SHELF_MARKER_BASE: usize = 10000;
    
    /// Location marker base for dropoff encoding (D1 = DROPOFF_MARKER_BASE + 1)
    pub const DROPOFF_MARKER_BASE: usize = 20000;
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

/// Renderer layer settings (visualization)
pub mod visual {
    /// Tile size in world units
    pub const TILE_SIZE: f32 = 1.0;

    /// XZ scale applied to all wall models to close seams between adjacent tiles.
    /// 1.0 = exact model size. Increase (e.g. 1.02) if gaps are visible at junctions.
    /// Y is not scaled so wall height is unaffected. Fix models in Blender for a permanent solution.
    pub const WALL_SEAM_SCALE: f32 = 1.00;

    /// Robot mesh size
    pub const ROBOT_SIZE: f32 = 0.5;

    /// Shelf mesh dimensions (width, height, depth)
    pub const SHELF_SIZE: (f32, f32, f32) = (0.8, 0.6, 0.8);

    /// Maximum cargo capacity for all shelves (4 levels x 4 boxes = 16)
    pub const SHELF_MAX_CAPACITY: u32 = 16;

    /// Scale factor for cargo boxes on shelves (1.0 = full model size)
    pub const BOX_SCALE: f32 = 0.6;

    /// Y offset for placeholder planes (station/dropoff) to sit above the floor
    pub const PLACEHOLDER_Y_OFFSET: f32 = 0.001;

    /// Maximum squared distance to match a robot pickup/drop to a shelf (1.5 units)
    pub const CARGO_SHELF_DISTANCE_SQ: f32 = 2.25;

    /// Colors (RGB 0.0-1.0)
    pub mod colors {
        /// Ground tile color
        pub const GROUND: (f32, f32, f32) = (0.9, 0.9, 0.9);
        /// Wall color
        pub const WALL: (f32, f32, f32) = (0.1, 0.1, 0.1);
        /// Station color (pink)
        pub const STATION: (f32, f32, f32) = (1.0, 0.4, 0.6);
        /// Dropoff color (green)
        pub const DROPOFF: (f32, f32, f32) = (0.0, 1.0, 0.4);
        /// Shelf color (brown)
        pub const SHELF: (f32, f32, f32) = (0.6, 0.4, 0.2);
        /// Robot color (cyan)
        pub const ROBOT: (f32, f32, f32) = (0.2, 0.7, 0.9);
    }

    /// Outline highlighting settings (hover and selection glow)
    pub mod outline {
        /// HDR hover outline color (bright white, values > 1.0 for bloom glow)
        pub const HOVER_COLOR: (f32, f32, f32) = (5.0, 5.0, 5.0);
        /// HDR select outline color (cyan/blue glow)
        pub const SELECT_COLOR: (f32, f32, f32) = (0.0, 2.5, 5.0);
        /// outline width in logical pixels
        pub const WIDTH: f32 = 3.0;
        /// bloom post-processing intensity (low to avoid blinding)
        pub const BLOOM_INTENSITY: f32 = 0.15;
    }

    /// Lighting settings
    pub mod lighting {
        pub const DIRECTIONAL_ILLUMINANCE: f32 = 15_000.0;
        pub const AMBIENT_BRIGHTNESS: f32 = 500.0;
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
