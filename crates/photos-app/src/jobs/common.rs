use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use async_trait::async_trait;
use futures::future::join_all;
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct TaskContext {
    pub(crate) service_registry: Arc<AppServiceRegistry>,
    pub(crate) task_queue: Arc<Mutex<TaskQueue>>,
    pub(crate) cancel: CancellationToken,
}

#[async_trait]
pub(crate) trait Map<I: Send + Sync, O: Send + Sync>: Send + Sync {
    async fn map(&self, input: I) -> Result<O, AppError>;
}

#[async_trait]
pub(crate) trait Reduce<I: Send + Sync, O: Send + Sync>: Send + Sync {
    async fn reduce(&self, inputs: Vec<I>) -> Result<O, AppError>;
}

#[async_trait]
pub(crate) trait Expand<I: Send + Sync, O: Send + Sync>: Send + Sync {
    async fn expand(&self, input: I) -> Result<Vec<O>, AppError>;
}

pub enum JobEvent {
    Progress(usize, usize),
    Done,
}

pub struct JobHandle<Output> {
    pub result_rx: Option<oneshot::Receiver<Output>>,
    pub event_rx: mpsc::Receiver<JobEvent>,
}

pub(crate) struct ExpandMapReduce<I, M1, M2, O> {
    pub(crate) expand: Arc<dyn Expand<I, M1>>,
    pub(crate) map: Arc<dyn Map<M1, M2>>,
    pub(crate) reduce: Arc<dyn Reduce<M2, O>>,
}

#[async_trait]
pub(crate) trait Dispatchable<I: Send + Sync + 'static, O: Send + Sync + 'static> {
    async fn dispatch(&self, ctx: TaskContext, input: I, cancel: CancellationToken) -> JobHandle<O>;
}

#[async_trait]
impl<
    I: Send + Sync + 'static,
    M1: Send + Sync + 'static,
    M2: Send + Sync + 'static,
    O: Send + Sync + 'static,
> Dispatchable<I, O> for ExpandMapReduce<I, M1, M2, O>
{
    async fn dispatch(
        &self,
        ctx: TaskContext,
        input: I,
        cancel: CancellationToken,
    ) -> JobHandle<O> {
        let queue = ctx.task_queue.clone();
        let expand = self.expand.clone();
        let map = self.map.clone();
        let reduce = self.reduce.clone();
        let cancel_clone = cancel.clone();

        let (tx, rx) = oneshot::channel();
        let (evt_tx, evt_rx) = mpsc::channel(16);
        let mut total = Arc::new(AtomicUsize::default());
        let mut completed = Arc::new(AtomicUsize::default());

        let task: TaskFn = Box::new(move || {
            Box::pin(async move {
                let vec_m1 = expand.expand(input).await.unwrap();
                total.store(vec_m1.len(), Ordering::Relaxed);
                let mut rxs = Vec::new();
                for m1 in vec_m1 {
                    let (map_tx, map_rx) = oneshot::channel();
                    let map = map.clone();
                    let completed = completed.clone();
                    let total = total.clone();
                    let evt_tx = evt_tx.clone();
                    let map_task: TaskFn = Box::new(move || { Box::pin(async move {
                            let m2 = map.map(m1).await.unwrap();
                            let _ = map_tx.send(m2);
                            completed.fetch_add(1, Ordering::Relaxed);
                            let _ = evt_tx.send(JobEvent::Progress(
                                completed.load(Ordering::Relaxed),
                                total.load(Ordering::Relaxed),
                            )).await;
                        })
                    });
                    let _ = queue.lock().await.submit(
                        map_task,
                        TaskPriority::Low,
                        cancel_clone.clone(),
                    );
                    rxs.push(map_rx);
                }

                let reduce_task: TaskFn = Box::new(move || {
                    Box::pin(async move {
                        let vec_m2: Vec<M2> = join_all(rxs)
                            .await
                            .into_iter()
                            .filter_map(|r| r.ok())
                            .collect();
                        let o = reduce.reduce(vec_m2).await.unwrap();
                        let _ = tx.send(o);
                        let _ = evt_tx.send(JobEvent::Done).await;
                    })
                });
                let _ = queue.lock().await.submit(
                    reduce_task,
                    TaskPriority::Lowest,
                    cancel_clone.clone(),
                );
            })
        });

        let _ = ctx
            .task_queue
            .lock()
            .await
            .submit(task, TaskPriority::Low, cancel);

        JobHandle {
            result_rx: Some(rx),
            event_rx: evt_rx,
        }
    }
}

#[async_trait]
impl Reduce<(),()> for () {
    async fn reduce(&self, _inputs: Vec<()>) -> Result<(), AppError> {
        Ok(())
    }
}

