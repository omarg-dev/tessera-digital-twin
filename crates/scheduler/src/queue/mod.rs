//! Task queue abstraction
//!
//! This module defines the `TaskQueue` trait and provides implementations.
//! To add a new queue strategy, create a new file and implement the trait.

mod fifo;
mod dispatcher;

pub use fifo::FifoQueue;
pub use dispatcher::QueueInstance;

use protocol::{Task, TaskId};

/// Trait for task queue implementations
///
/// Implement this trait to create custom queueing strategies:
/// - `FifoQueue` - Priority-first queue with FIFO tiebreaking (default)
/// - `PriorityQueue` - Heap-based priority queue (future)
/// - `MLOptimizedQueue` - ML-driven task ordering (future)
///
/// Note: Some trait methods may appear unused in the binary but are part of the public API
/// used by tests and future extensions (pathfinding-aware allocation, ML optimization, etc.)
pub trait TaskQueue: Send + Sync {
    /// Generate a new unique task ID
    fn next_task_id(&mut self) -> TaskId;
    
    /// Add a new task to the queue
    fn enqueue(&mut self, task: Task);

    /// Remove and return the next task to process
    #[allow(dead_code)]
    fn dequeue(&mut self) -> Option<Task>;

    /// Peek at the next task without removing it
    #[allow(dead_code)]
    fn peek(&self) -> Option<&Task>;

    /// Get a task by ID
    fn get(&self, id: TaskId) -> Option<&Task>;

    /// Get a mutable reference to a task by ID
    fn get_mut(&mut self, id: TaskId) -> Option<&mut Task>;

    /// Remove a task by ID (for cancellation)
    #[allow(dead_code)]
    fn remove(&mut self, id: TaskId) -> Option<Task>;

    /// Number of pending tasks
    fn pending_count(&self) -> usize;

    /// Total tasks (including assigned/in-progress)
    fn total_count(&self) -> usize;

    /// Get all pending tasks
    fn pending_tasks(&self) -> Vec<&Task>;

    /// Get all tasks
    fn all_tasks(&self) -> Vec<&Task>;

    /// Remove all completed and failed tasks from the queue
    /// Returns the number of tasks removed (for logging)
    #[allow(dead_code)]
    fn cleanup_completed(&mut self) -> usize;
}
