use crate::task::{QueuedTask, TaskFn, TaskPriority};
use std::collections::BinaryHeap;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info};

pub struct TaskQueue {
    task_sender: mpsc::UnboundedSender<QueuedTask>,
    _worker_handle: JoinHandle<()>,
}

impl TaskQueue {
    pub fn new(handle: Handle) -> Self {
        let (task_sender, mut task_receiver) = mpsc::unbounded_channel::<QueuedTask>();
        
        let worker_handle = handle.spawn(async move {
            let mut task_heap = BinaryHeap::new();
            
            loop {
                tokio::select! {
                    task_opt = task_receiver.recv() => {
                        match task_opt {
                            Some(task) => {
                                task_heap.push(task);
                                debug!("Task queued, heap size: {}", task_heap.len());
                            }
                            None => {
                                info!("Task queue receiver closed, shutting down worker");
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)), if !task_heap.is_empty() => {
                        if let Some(queued_task) = task_heap.pop() {
                            debug!("Processing task with priority: {:?}", queued_task.priority);
                            (queued_task.task)();
                        }
                    }
                }
            }
            
            while let Some(queued_task) = task_heap.pop() {
                debug!("Processing remaining task with priority: {:?}", queued_task.priority);
                (queued_task.task)();
            }
        });
        
        Self {
            task_sender,
            _worker_handle: worker_handle,
        }
    }
    
    pub fn submit(&self, task: TaskFn, priority: TaskPriority) -> Result<(), String> {
        let queued_task = QueuedTask::new(task, priority);
        self.task_sender
            .send(queued_task)
            .map_err(|e| format!("Failed to submit task: {}", e))
    }
}

