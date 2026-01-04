use crate::task::{QueuedTask, TaskFn, TaskPriority};
use std::collections::BinaryHeap;
use std::sync::Arc;
use tokio::{runtime::Handle, sync::Semaphore};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, info};

pub struct TaskQueue {
    task_sender: mpsc::UnboundedSender<QueuedTask>,
    _semaphore: Arc<Semaphore>,
    _worker_handle: JoinHandle<()>,
}

impl TaskQueue {
    pub fn new(handle: Handle, max_blocking_tasks: usize) -> Self {
        let (task_sender, mut task_receiver) = mpsc::unbounded_channel::<QueuedTask>();
        let semaphore = Arc::new(Semaphore::new(max_blocking_tasks));
        let semaphore_for_worker = semaphore.clone();
        let handle_for_worker = handle.clone();
        
        let worker_handle = handle.spawn(async move {
            let mut task_heap = BinaryHeap::new();
            let mut running_tasks = Vec::new();
            
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
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)), if !task_heap.is_empty() || !running_tasks.is_empty() => {
                        running_tasks.retain(|handle: &JoinHandle<()>| {
                            if handle.is_finished() {
                                false
                            } else {
                                true
                            }
                        });
                        
                        while running_tasks.len() < max_blocking_tasks && !task_heap.is_empty() {
                            if let Ok(permit) = semaphore_for_worker.clone().try_acquire_owned() {
                                if let Some(queued_task) = task_heap.pop() {
                                    debug!("Processing task with priority: {:?}, running: {}", queued_task.priority, running_tasks.len());
                                    let task_fn = queued_task.task;
                                    let spawn_handle = handle_for_worker.clone();
                                    
                                    let task_handle = spawn_handle.spawn(async move {
                                        let _permit = permit;
                                        let future = (task_fn)();
                                        future.await;
                                    });
                                    
                                    running_tasks.push(task_handle);
                                } else {
                                    drop(permit);
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
            
            for task_handle in running_tasks {
                let _ = task_handle.await;
            }
            
            while let Some(queued_task) = task_heap.pop() {
                debug!("Processing remaining task with priority: {:?}", queued_task.priority);
                if let Ok(permit) = semaphore_for_worker.clone().try_acquire_owned() {
                    let task_fn = queued_task.task;
                    let _ = handle_for_worker.spawn(async move {
                        let _permit = permit;
                        let future = (task_fn)();
                        future.await;
                    }).await;
                } else {
                    let future = (queued_task.task)();
                    let _ = handle_for_worker.spawn(future).await;
                }
            }
        });
        
        Self {
            task_sender,
            _semaphore: semaphore,
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

