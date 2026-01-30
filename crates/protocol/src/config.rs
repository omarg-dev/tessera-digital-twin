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
pub const LAYOUT_FILE_PATH: &str = "assets/data/layout.txt";

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
    /// Battery drain rate: % per second while moving
    pub const DRAIN_RATE: f32 = 0.01;
    
    /// Low battery warning threshold (percentage)
    pub const LOW_THRESHOLD: f32 = 20.0;
    
    /// Charge rate: % per second while at station
    pub const CHARGE_RATE: f32 = 10.0;
}

/// Coordinator layer settings (path planning & task execution)
pub mod coordinator {
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
}

/// Scheduler layer settings (task queue & robot allocation)
pub mod scheduler {
    /// Main loop sleep interval in milliseconds
    pub const LOOP_INTERVAL_MS: u64 = 50;
    
    /// Queue state broadcast interval in seconds
    pub const QUEUE_BROADCAST_SECS: u64 = 2;
    
    /// Minimum battery level for robot allocation (percentage)
    pub const MIN_BATTERY_FOR_TASK: f32 = 20.0;
    
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
    
    /// Robot mesh size
    pub const ROBOT_SIZE: f32 = 0.5;
    
    /// Shelf mesh dimensions (width, height, depth)
    pub const SHELF_SIZE: (f32, f32, f32) = (0.8, 0.6, 0.8);
    
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
    }
}
