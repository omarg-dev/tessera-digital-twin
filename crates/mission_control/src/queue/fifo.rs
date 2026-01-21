//! Simple FIFO queue with priority support

use super::TaskQueue;
use protocol::{Priority, Task, TaskId, TaskStatus};
use std::collections::VecDeque;

/// FIFO queue with priority awareness
///
/// Tasks are stored in insertion order. When dequeuing:
/// 1. Critical tasks are returned first
/// 2. Among same priority, FIFO order is preserved
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
    fn enqueue(&mut self, task: Task) {
        println!("+ Task {} queued: {:?} (priority: {:?})", 
            task.id, task.task_type, task.priority);
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

    fn all_tasks(&self) -> Vec<&Task> {
        self.tasks.iter().collect()
    }
}
