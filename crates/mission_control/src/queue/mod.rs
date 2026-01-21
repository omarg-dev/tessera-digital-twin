//! Task queue abstraction
//!
//! This module defines the `TaskQueue` trait and provides implementations.
//! To add a new queue strategy, create a new file and implement the trait.

mod fifo;

pub use fifo::FifoQueue;

use protocol::{Task, TaskId};

/// Trait for task queue implementations
///
/// Implement this trait to create custom queueing strategies:
/// - `FifoQueue` - Simple FIFO with priority support (default)
/// - `PriorityQueue` - Heap-based priority queue
/// - `MLOptimizedQueue` - ML-driven task ordering
#[allow(dead_code)]
pub trait TaskQueue: Send + Sync {
    /// Add a new task to the queue
    fn enqueue(&mut self, task: Task);

    /// Remove and return the next task to process
    fn dequeue(&mut self) -> Option<Task>;

    /// Peek at the next task without removing it
    fn peek(&self) -> Option<&Task>;

    /// Get a task by ID
    fn get(&self, id: TaskId) -> Option<&Task>;

    /// Get a mutable reference to a task by ID
    fn get_mut(&mut self, id: TaskId) -> Option<&mut Task>;

    /// Remove a task by ID (for cancellation)
    fn remove(&mut self, id: TaskId) -> Option<Task>;

    /// Number of pending tasks
    fn pending_count(&self) -> usize;

    /// Total tasks (including assigned/in-progress)
    fn total_count(&self) -> usize;

    /// Get all pending tasks
    fn pending_tasks(&self) -> Vec<&Task>;

    /// Get all tasks
    fn all_tasks(&self) -> Vec<&Task>;
}
