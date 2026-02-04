use crate::task::{QueuedTask, TaskFn, TaskPriority};
use crate::triple_queue::TripleQueue;
use std::ops::Div;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::{runtime::Handle, sync::Semaphore};
use tokio_util::sync::CancellationToken;
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
            let mut queues = TripleQueue::default();
            let mut running_tasks: Vec<(_, JoinHandle<()>)> = Vec::new();

            loop {
                tokio::select! {
                    task_opt = task_receiver.recv() => {
                        match task_opt {
                            Some(task) => {
                                queues.push(task);
                                debug!("Task queued, queue lens: {}/{}/{}", queues.len_h(), queues.len_l(), queues.len_ll());
                            }
                            None => {
                                info!("Task queue receiver closed, shutting down worker");
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)), if !queues.is_empty() || !running_tasks.is_empty() => {
                        running_tasks.retain(|(_, h)| !h.is_finished());

                        let allowed_priority = if let Some(p) = running_tasks
                            .iter()
                            .map(|(p, _)| *p)
                            .max()
                        {
                            Some(p)
                        } else {
                            queues.peek_next_priority()
                        };

                        let priorities_all = [TaskPriority::High, TaskPriority::Low, TaskPriority::Lowest];
                        let priorities_high_low = [TaskPriority::High, TaskPriority::Low];
                        let priorities_high = [TaskPriority::High];

                        let priorities_to_run: &[TaskPriority] = match allowed_priority {
                            None | Some(TaskPriority::Lowest) => &priorities_all,
                            Some(TaskPriority::Low) => &priorities_high_low,
                            Some(TaskPriority::High) => &priorities_high,
                        };


                        tracing::trace!("max allowed priority: {allowed_priority:?}");

                        for priority in priorities_to_run {
                            while !queues.is_empty() && can_start(*priority, &running_tasks, max_blocking_tasks) {
                                let task = match queues.pop(*priority) {
                                    Some(t) if t.cancel.is_cancelled() => continue,
                                    Some(t) => t,
                                    None => break,
                                };

                                let permit = match semaphore_for_worker.clone().try_acquire_owned() {
                                    Ok(p) => p,
                                    Err(_) => break,
                                };

                                let task_fn = task.task;

                                debug!("running task with priority {:?}, queue lens: {}/{}/{}", priority, queues.len_h(), queues.len_l(), queues.len_ll());

                                let handle = handle_for_worker.clone().spawn(async move {
                                    let _permit = permit;
                                    task_fn().await;
                                });

                                running_tasks.push((*priority, handle));
                            }
                        }
                    }
                }
            }

            for (_, task_handle) in running_tasks {
                let _ = task_handle.await;
            }

            for priority in [TaskPriority::High, TaskPriority::Low, TaskPriority::Lowest] {
                while let Some(queued_task) = queues.pop(priority) {
                    debug!("Processing remaining task with priority: {:?}", queued_task.priority);
                    if let Ok(permit) = semaphore_for_worker.clone().try_acquire_owned() {
                        let task_fn = queued_task.task;
                        let _ = handle_for_worker.spawn(async move {
                            let _permit = permit;
                            let future = task_fn();
                            future.await;
                        }).await;
                    } else {
                        let future = (queued_task.task)();
                        let _ = handle_for_worker.spawn(future).await;
                    }
                }
            }
        });

        Self {
            task_sender,
            _semaphore: semaphore,
            _worker_handle: worker_handle,
        }
    }

    pub fn submit(
        &self,
        task: TaskFn,
        priority: TaskPriority,
        cancel: CancellationToken,
    ) -> Result<(), String> {
        let queued_task = QueuedTask::new(task, priority, cancel);
        self.task_sender
            .send(queued_task)
            .map_err(|e| format!("Failed to submit task: {}", e))
    }
}

fn count_running(running: &[(TaskPriority, JoinHandle<()>)], priority: TaskPriority) -> usize {
    running.iter().filter(|(p, _)| *p == priority).count()
}

fn can_start(
    priority: TaskPriority,
    running: &[(TaskPriority, JoinHandle<()>)],
    max: usize,
) -> bool {
    let total_running = running.len();
    if total_running >= max {
        return false;
    }

    let running_of_priority = count_running(running, priority);
    running_of_priority < priority_limit(priority, max)
}

fn priority_limit(priority: TaskPriority, max: usize) -> usize {
    match priority {
        TaskPriority::High => max,
        TaskPriority::Low => max.div(2).max(1),
        TaskPriority::Lowest => max.div(4).max(1),
    }
}
