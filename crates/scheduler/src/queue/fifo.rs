//! Priority-aware FIFO queue implementation
//!
//! Despite the name "FifoQueue", this is actually a **priority queue with FIFO tiebreaking**.
//! Tasks are ordered by:
//! 1. **Priority** - Critical > High > Normal > Low
//! 2. **Insertion order** - FIFO within the same priority level
//!
//! The name is kept for API compatibility. For a pure heap-based priority queue,
//! implement a new `HeapPriorityQueue` struct.

use super::TaskQueue;
use protocol::{Priority, Task, TaskId, TaskStatus};
use std::collections::VecDeque;

/// Priority-aware FIFO queue
///
/// Tasks are stored in insertion order. When dequeuing:
/// 1. Highest priority pending task is returned first
/// 2. Among same priority, FIFO order is preserved
///
/// **Note:** This is NOT a pure FIFO queue - it prioritizes by `Priority` enum first.
pub struct FifoQueue {
    tasks: VecDeque<Task>,
    next_id: TaskId,
}

impl FifoQueue {
    pub fn new() -> Self {
        FifoQueue {
            tasks: VecDeque::new(),
            next_id: 1,
        }
    }

    /// Generate a new unique task ID
    pub fn next_task_id(&mut self) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Find index of highest priority pending task (preserves FIFO for equal priority)
    /// Called internally by dequeue() and peek() - appears unused but is actually used
    #[allow(dead_code)]
    fn find_next_pending_index(&self) -> Option<usize> {
        let mut best_idx: Option<usize> = None;
        let mut best_priority = Priority::Low;

        for (i, task) in self.tasks.iter().enumerate() {
            if task.status != TaskStatus::Pending {
                continue;
            }
            // Only update if strictly higher priority, or first pending task found
            if best_idx.is_none() || task.priority > best_priority {
                best_priority = task.priority;
                best_idx = Some(i);
            }
        }

        best_idx
    }
}

impl Default for FifoQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskQueue for FifoQueue {
    fn next_task_id(&mut self) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn enqueue(&mut self, task: Task) {
        self.tasks.push_back(task);
    }

    fn dequeue(&mut self) -> Option<Task> {
        self.find_next_pending_index()
            .and_then(|idx| self.tasks.remove(idx))
    }

    fn peek(&self) -> Option<&Task> {
        self.find_next_pending_index()
            .and_then(|idx| self.tasks.get(idx))
    }

    fn get(&self, id: TaskId) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    fn get_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    fn remove(&mut self, id: TaskId) -> Option<Task> {
        self.tasks.iter().position(|t| t.id == id)
            .and_then(|idx| self.tasks.remove(idx))
    }

    fn pending_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Pending).count()
    }

    fn total_count(&self) -> usize {
        self.tasks.len()
    }

    fn pending_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == TaskStatus::Pending).collect()
    }

    fn pending_task_ids_limited(&self, limit: usize) -> Vec<TaskId> {
        let mut ids = Vec::with_capacity(limit);
        if limit == 0 {
            return ids;
        }

        for task in &self.tasks {
            if task.status == TaskStatus::Pending {
                ids.push(task.id);
                if ids.len() == limit {
                    break;
                }
            }
        }

        ids
    }

    fn all_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().collect()
    }

    fn cleanup_completed(&mut self) -> usize {
        let initial_len = self.tasks.len();
        // Retain only tasks that are not completed/failed
        self.tasks.retain(|task| {
            match task.status {
                TaskStatus::Completed | TaskStatus::Failed { .. } | TaskStatus::Cancelled => false,
                _ => true,
            }
        });
        initial_len - self.tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::TaskType;

    fn make_task(id: u64, priority: Priority) -> Task {
        Task::new(id, TaskType::PickAndDeliver {
            pickup: (1, 1), dropoff: (5, 5), cargo_id: None,
        }, priority)
    }

    #[test]
    fn test_enqueue_increments_count() {
        let mut queue = FifoQueue::new();
        assert_eq!(queue.total_count(), 0);
        
        queue.enqueue(make_task(1, Priority::Normal));
        assert_eq!(queue.total_count(), 1);
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_next_task_id_increments() {
        let mut queue = FifoQueue::new();
        assert_eq!(queue.next_task_id(), 1);
        assert_eq!(queue.next_task_id(), 2);
        assert_eq!(queue.next_task_id(), 3);
    }

    #[test]
    fn test_fifo_order_same_priority() {
        let mut queue = FifoQueue::new();
        queue.enqueue(make_task(1, Priority::Normal));
        queue.enqueue(make_task(2, Priority::Normal));
        queue.enqueue(make_task(3, Priority::Normal));
        
        // FIFO: first in, first out
        assert_eq!(queue.dequeue().unwrap().id, 1);
        assert_eq!(queue.dequeue().unwrap().id, 2);
        assert_eq!(queue.dequeue().unwrap().id, 3);
        assert!(queue.dequeue().is_none());
    }

    #[test]
    fn test_priority_over_fifo() {
        let mut queue = FifoQueue::new();
        queue.enqueue(make_task(1, Priority::Low));
        queue.enqueue(make_task(2, Priority::Critical));
        queue.enqueue(make_task(3, Priority::Normal));
        
        // Critical first, then Normal, then Low
        assert_eq!(queue.dequeue().unwrap().id, 2);
        assert_eq!(queue.dequeue().unwrap().id, 3);
        assert_eq!(queue.dequeue().unwrap().id, 1);
    }

    #[test]
    fn test_get_and_get_mut() {
        let mut queue = FifoQueue::new();
        queue.enqueue(make_task(1, Priority::Normal));
        queue.enqueue(make_task(2, Priority::Normal));
        
        assert!(queue.get(1).is_some());
        assert!(queue.get(99).is_none());
        
        // Modify via get_mut
        if let Some(task) = queue.get_mut(1) {
            task.status = TaskStatus::Assigned { robot_id: 42 };
        }
        
        // Verify assigned task is no longer pending
        assert_eq!(queue.pending_count(), 1);
        assert_eq!(queue.total_count(), 2);
    }

    #[test]
    fn test_remove_task() {
        let mut queue = FifoQueue::new();
        queue.enqueue(make_task(1, Priority::Normal));
        queue.enqueue(make_task(2, Priority::Normal));
        
        let removed = queue.remove(1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, 1);
        assert_eq!(queue.total_count(), 1);
        assert!(queue.remove(1).is_none()); // Already removed
    }

    #[test]
    fn test_peek_does_not_remove() {
        let mut queue = FifoQueue::new();
        queue.enqueue(make_task(1, Priority::Normal));
        
        assert_eq!(queue.peek().unwrap().id, 1);
        assert_eq!(queue.peek().unwrap().id, 1); // Still there
        assert_eq!(queue.total_count(), 1);
    }

    #[test]
    fn test_cleanup_completed() {
        let mut queue = FifoQueue::new();
        let task1 = make_task(1, Priority::Normal);
        let task2 = make_task(2, Priority::Normal);
        let task3 = make_task(3, Priority::High);
        
        queue.enqueue(task1);
        queue.enqueue(task2);
        queue.enqueue(task3);
        
        assert_eq!(queue.total_count(), 3);
        
        // Mark some as completed
        if let Some(t) = queue.get_mut(1) {
            t.status = TaskStatus::Completed;
        }
        if let Some(t) = queue.get_mut(2) {
            t.status = TaskStatus::Failed { reason: "test".to_string() };
        }
        
        let removed = queue.cleanup_completed();
        assert_eq!(removed, 2);
        assert_eq!(queue.total_count(), 1); // Only task3 remains
        assert!(queue.get(3).is_some()); // task3 should still be there
    }

    #[test]
    fn test_pending_task_ids_limited_respects_limit() {
        let mut queue = FifoQueue::new();
        queue.enqueue(make_task(1, Priority::Normal));
        queue.enqueue(make_task(2, Priority::High));
        queue.enqueue(make_task(3, Priority::Low));

        if let Some(task) = queue.get_mut(2) {
            task.status = TaskStatus::Assigned { robot_id: 7 };
        }

        assert_eq!(queue.pending_task_ids_limited(0), Vec::<TaskId>::new());
        assert_eq!(queue.pending_task_ids_limited(1), vec![1]);
        assert_eq!(queue.pending_task_ids_limited(5), vec![1, 3]);
    }
}
