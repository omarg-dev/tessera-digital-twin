//! Robot allocation abstraction
//!
//! This module defines the `Allocator` trait and provides implementations.
//! To add a new allocation strategy, create a new file and implement the trait.

mod closest;
mod dispatcher;

pub use closest::ClosestIdleAllocator;
pub use dispatcher::AllocatorInstance;

use protocol::{RobotState, RobotUpdate, Task};
use std::collections::HashMap;

/// Robot state as known to scheduler
#[derive(Debug, Clone)]
pub struct RobotInfo {
    pub id: u32,
    pub position: [f32; 3],
    pub state: RobotState,
    pub battery: f32,
    pub assigned_task: Option<u64>,
}

impl From<&RobotUpdate> for RobotInfo {
    fn from(update: &RobotUpdate) -> Self {
        RobotInfo {
            id: update.id,
            position: update.position,
            state: update.state.clone(),
            battery: update.battery,
            assigned_task: None,
        }
    }
}

/// Trait for robot allocation strategies
///
/// Implement this trait to create custom allocation logic:
/// - `ClosestIdleAllocator` - Distance-based (default)
/// - `LoadBalancedAllocator` - Distributes work evenly
/// - `BatteryAwareAllocator` - Prefers high-battery robots
/// - `MLAllocator` - ML-driven optimal assignment
pub trait Allocator: Send + Sync {
    /// Find the best robot for a given task
    fn allocate(&self, task: &Task, robots: &HashMap<u32, RobotInfo>) -> Option<u32>;
}
