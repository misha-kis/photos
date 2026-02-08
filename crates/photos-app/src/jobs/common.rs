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
    NextJob(Box<JobHandle>),
}

pub struct JobHandle {
    pub res_rx: oneshot::Receiver<()>,
    pub evt_rx: mpsc::Receiver<JobEvent>,
}

pub(crate) struct ExpandMapReduce<I, M1, M2, O> {
    pub(crate) expand: Arc<dyn Expand<I, M1>>,
    pub(crate) map: Arc<dyn Map<M1, M2>>,
    pub(crate) reduce: Arc<dyn Reduce<M2, O>>,
}

#[async_trait]
pub(crate) trait Dispatchable<I: Send + Sync + 'static, O: Send + Sync + 'static>:
    Send + Sync
{
    async fn dispatch(&self, ctx: TaskContext, input: I, cancel: CancellationToken) -> JobHandle;
}

#[async_trait]
pub(crate) trait OneshotDispatchable<I: Send + Sync + 'static, O: Send + Sync + 'static> {
    async fn dispatch(
        &self,
        ctx: TaskContext,
        input: I,
        task_priority: TaskPriority,
        cancel: CancellationToken,
    ) -> oneshot::Receiver<Result<O, AppError>>;
}

#[async_trait]
impl<T, I, O> OneshotDispatchable<I, O> for Arc<T>
where
    T: Map<I, O> + Send + Sync + 'static,
    I: Send + Sync + 'static,
    O: Send + Sync + 'static,
{
    async fn dispatch(
        &self,
        ctx: TaskContext,
        input: I,
        task_priority: TaskPriority,
        cancel: CancellationToken,
    ) -> oneshot::Receiver<Result<O, AppError>> {
        let (tx, rx) = oneshot::channel();
        let map = self.clone();
        let task: TaskFn = Box::new(move || {
            Box::pin(async move {
                let output = map.map(input).await;
                let _ = tx.send(output);
            })
        });
        ctx.task_queue
            .lock()
            .await
            .submit(task, task_priority, cancel)
            .expect("couldn't dispatch");
        rx
    }
}

#[async_trait]
impl<
    I: Send + Sync + 'static,
    M1: Send + Sync + 'static,
    M2: Send + Sync + 'static,
    // O: Send + Sync + 'static,
> Dispatchable<I, ()> for ExpandMapReduce<I, M1, M2, ()>
{
    async fn dispatch(&self, ctx: TaskContext, input: I, cancel: CancellationToken) -> JobHandle {
        let queue = ctx.task_queue.clone();
        let expand = self.expand.clone();
        let map = self.map.clone();
        let reduce = self.reduce.clone();
        let cancel_clone = cancel.clone();

        let (res_tx, res_rx) = oneshot::channel();
        let (evt_tx, evt_rx) = mpsc::channel(16);
        let total = Arc::new(AtomicUsize::default());
        let completed = Arc::new(AtomicUsize::default());

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
                    let map_task: TaskFn = Box::new(move || {
                        Box::pin(async move {
                            let m2 = map.map(m1).await.unwrap();
                            let _ = map_tx.send(m2);
                            completed.fetch_add(1, Ordering::Relaxed);
                            let _ = evt_tx
                                .send(JobEvent::Progress(
                                    completed.load(Ordering::Relaxed),
                                    total.load(Ordering::Relaxed),
                                ))
                                .await;
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
                        let _ = res_tx.send(o);
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

        JobHandle { res_rx, evt_rx }
    }
}

#[async_trait]
impl Reduce<(), ()> for () {
    async fn reduce(&self, _inputs: Vec<()>) -> Result<(), AppError> {
        Ok(())
    }
}

#[async_trait]
impl<I, J1, J2> Dispatchable<I, ()> for (Arc<J1>, Arc<J2>)
where
    I: Send + Sync + 'static,
    J1: Dispatchable<I, ()> + ?Sized + 'static,
    J2: Dispatchable<(), ()> + ?Sized + 'static,
{
    async fn dispatch(&self, ctx: TaskContext, input: I, cancel: CancellationToken) -> JobHandle {
        let (res_tx, res_rx) = oneshot::channel();
        let (evt_tx, evt_rx) = mpsc::channel(32);

        let (job1, job2) = self.clone();

        let ctx1 = ctx.clone();
        let ctx2 = ctx.clone();
        let cancel1 = cancel.clone();
        let cancel2 = cancel.clone();

        // ---- Task 1: dispatch job1 and wire listeners
        let start_job1: TaskFn = Box::new(move || {
            Box::pin(async move {
                let JobHandle {
                    evt_rx: mut evt_rx_1,
                    res_rx: res_rx_1,
                } = job1.dispatch(ctx1.clone(), input, cancel1.clone()).await;

                // Forward job1 events (non-blocking, separate task)
                let evt_tx_clone = evt_tx.clone();
                tokio::spawn(async move {
                    while let Some(evt) = evt_rx_1.recv().await {
                        let _ = evt_tx_clone.send(evt).await;
                    }
                });

                // ---- Task 2: triggered when job1 finishes
                let trigger_job2: TaskFn = Box::new(move || {
                    Box::pin(async move {
                        let _ = res_rx_1.await;

                        let jh_2 = job2.dispatch(ctx2.clone(), (), cancel2.clone()).await;

                        let _ = evt_tx.send(JobEvent::NextJob(Box::new(jh_2))).await;

                        // Final completion is driven by job2
                        // tokio::spawn(async move {
                        //     let _ = jh_2.res_rx.await;
                        //     let _ = res_tx.send(());
                        // });
                    })
                });

                let _ = ctx1.task_queue.lock().await.submit(
                    trigger_job2,
                    TaskPriority::Lowest,
                    cancel1,
                );
            })
        });

        let _ = ctx
            .task_queue
            .lock()
            .await
            .submit(start_job1, TaskPriority::Lowest, cancel);

        JobHandle { res_rx, evt_rx }
    }
}
