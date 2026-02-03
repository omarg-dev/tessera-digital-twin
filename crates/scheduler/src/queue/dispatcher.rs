//! Task queue strategy dispatcher
//!
//! Allows runtime selection of queueing algorithm via config.
//! Provides enum-based dispatch while maintaining trait flexibility.

use super::{TaskQueue, FifoQueue};
use protocol::{Task, TaskId};

/// Runtime-selectable task queue strategy
pub enum QueueInstance {
    /// FIFO queue with priority support (default)
    Fifo(FifoQueue),
}

impl QueueInstance {
    /// Create queue based on config strategy
    pub fn from_config() -> Self {
        match protocol::config::scheduler::QUEUE_STRATEGY {
            "fifo" | _ => {
                println!("✓ Queue: FIFO with Priority (default)");
                QueueInstance::Fifo(FifoQueue::new())
            }
        }
    }
}

impl TaskQueue for QueueInstance {
    fn next_task_id(&mut self) -> TaskId {
        match self {
            QueueInstance::Fifo(queue) => queue.next_task_id(),
        }
    }

    fn enqueue(&mut self, task: Task) {
        match self {
            QueueInstance::Fifo(queue) => queue.enqueue(task),
        }
    }

    fn dequeue(&mut self) -> Option<Task> {
        match self {
            QueueInstance::Fifo(queue) => queue.dequeue(),
        }
    }

    fn peek(&self) -> Option<&Task> {
        match self {
            QueueInstance::Fifo(queue) => queue.peek(),
        }
    }

    fn get(&self, id: TaskId) -> Option<&Task> {
        match self {
            QueueInstance::Fifo(queue) => queue.get(id),
        }
    }

    fn get_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        match self {
            QueueInstance::Fifo(queue) => queue.get_mut(id),
        }
    }

    fn remove(&mut self, id: TaskId) -> Option<Task> {
        match self {
            QueueInstance::Fifo(queue) => queue.remove(id),
        }
    }

    fn pending_count(&self) -> usize {
        match self {
            QueueInstance::Fifo(queue) => queue.pending_count(),
        }
    }

    fn total_count(&self) -> usize {
        match self {
            QueueInstance::Fifo(queue) => queue.total_count(),
        }
    }

    fn pending_tasks(&self) -> Vec<&Task> {
        match self {
            QueueInstance::Fifo(queue) => queue.pending_tasks(),
        }
    }

    fn all_tasks(&self) -> Vec<&Task> {
        match self {
            QueueInstance::Fifo(queue) => queue.all_tasks(),
        }
    }

    fn cleanup_completed(&mut self) -> usize {
        match self {
            QueueInstance::Fifo(queue) => queue.cleanup_completed(),
        }
    }
}
