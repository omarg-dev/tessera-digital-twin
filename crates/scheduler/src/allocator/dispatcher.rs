//! Allocator strategy dispatcher
//!
//! Allows runtime selection of allocation algorithm via config.
//! Provides enum-based dispatch while maintaining trait flexibility.

use super::{Allocator, ClosestIdleAllocator, RobotInfo};
use protocol::Task;
use std::collections::HashMap;

/// Runtime-selectable allocation strategy
pub enum AllocatorInstance {
    /// Closest idle robot (default, distance-based)
    ClosestIdle(ClosestIdleAllocator),
}

impl AllocatorInstance {
    /// Create allocator based on config strategy
    pub fn from_config() -> Self {
        match protocol::config::scheduler::ALLOCATOR_STRATEGY {
            "closest_idle" | _ => {
                println!("✓ Allocator: Closest Idle (distance-based)");
                AllocatorInstance::ClosestIdle(ClosestIdleAllocator::new())
            }
        }
    }
}

impl Allocator for AllocatorInstance {
    fn allocate(&self, task: &Task, robots: &HashMap<u32, RobotInfo>) -> Option<u32> {
        match self {
            AllocatorInstance::ClosestIdle(allocator) => allocator.allocate(task, robots),
        }
    }
}
