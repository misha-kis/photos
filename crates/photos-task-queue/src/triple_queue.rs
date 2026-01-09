use crate::task::{QueuedTask, TaskPriority};
use std::collections::VecDeque;

#[derive(Default)]
pub(crate) struct TripleQueue {
    high: VecDeque<QueuedTask>,
    low: VecDeque<QueuedTask>,
    lowest: VecDeque<QueuedTask>,
}

impl TripleQueue {
    pub(crate) fn is_empty(&self) -> bool {
        self.high.is_empty() && self.low.is_empty() && self.lowest.is_empty()
    }

    pub(crate) fn push(&mut self, task: QueuedTask) {
        match task.priority {
            TaskPriority::High => self.high.push_back(task),
            TaskPriority::Low => self.low.push_back(task),
            TaskPriority::Lowest => self.lowest.push_back(task),
        }
    }

    pub(crate) fn peek_next_priority(&self) -> Option<TaskPriority> {
        if !self.high.is_empty() {
            Some(TaskPriority::High)
        } else if !self.low.is_empty() {
            Some(TaskPriority::Low)
        } else if !self.lowest.is_empty() {
            Some(TaskPriority::Lowest)
        } else {
            None
        }
    }

    pub(crate) fn pop(&mut self, priority: TaskPriority) -> Option<QueuedTask> {
        match priority {
            TaskPriority::High => self.high.pop_front(),
            TaskPriority::Low => self.low.pop_front(),
            TaskPriority::Lowest => self.lowest.pop_front(),
        }
    }

    pub(crate) fn len_h(&self) -> usize {
        self.high.len()
    }
    pub(crate) fn len_l(&self) -> usize {
        self.low.len()
    }
    pub(crate) fn len_ll(&self) -> usize {
        self.lowest.len()
    }
}
