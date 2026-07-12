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
    NextJob(Box<mpsc::Receiver<JobEvent>>),
}

pub struct JobHandle {
    pub res_rx: oneshot::Receiver<Result<(), AppError>>,
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
        let tx = Arc::new(Mutex::new(Some(tx)));
        let map = self.clone();
        let tx_for_task = tx.clone();
        let task: TaskFn = Box::new(move || {
            Box::pin(async move {
                let output = map.map(input).await;
                if let Some(tx) = tx_for_task.lock().await.take() {
                    let _ = tx.send(output);
                }
            })
        });
        if let Err(err) = ctx
            .task_queue
            .lock()
            .await
            .submit(task, task_priority, cancel)
            && let Some(tx) = tx.lock().await.take()
        {
            let _ = tx.send(Err(err.into()));
        }
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
        let res_tx = Arc::new(Mutex::new(Some(res_tx)));
        let (evt_tx, evt_rx) = mpsc::channel(16);

        let res_tx_outer = res_tx.clone();
        let evt_tx_outer = evt_tx.clone();
        let total = Arc::new(AtomicUsize::default());
        let completed = Arc::new(AtomicUsize::default());

        let res_tx_for_task = res_tx.clone();
        let evt_tx_for_task = evt_tx.clone();
        let task: TaskFn = Box::new(move || {
            Box::pin(async move {
                let vec_m1 = match expand.expand(input).await {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::error!("expand failed: {err}");
                        if let Some(res_tx) = res_tx_for_task.lock().await.take() {
                            let _ = res_tx.send(Err(err));
                        }
                        let _ = evt_tx_for_task.send(JobEvent::Done).await;
                        return;
                    }
                };
                total.store(vec_m1.len(), Ordering::Relaxed);
                let mut rxs = Vec::new();
                for m1 in vec_m1 {
                    let (map_tx, map_rx) = oneshot::channel();
                    let map_tx = Arc::new(Mutex::new(Some(map_tx)));
                    let map = map.clone();
                    let completed = completed.clone();
                    let total = total.clone();
                    let evt_tx = evt_tx.clone();
                    let map_tx_for_task = map_tx.clone();
                    let map_task: TaskFn = Box::new(move || {
                        Box::pin(async move {
                            let result = map.map(m1).await;
                            if result.is_ok() {
                                completed.fetch_add(1, Ordering::Relaxed);
                                let _ = evt_tx
                                    .send(JobEvent::Progress(
                                        completed.load(Ordering::Relaxed),
                                        total.load(Ordering::Relaxed),
                                    ))
                                    .await;
                            }
                            if let Some(map_tx) = map_tx_for_task.lock().await.take() {
                                let _ = map_tx.send(result);
                            }
                        })
                    });
                    if let Err(err) =
                        queue
                            .lock()
                            .await
                            .submit(map_task, TaskPriority::Low, cancel_clone.clone())
                        && let Some(map_tx) = map_tx.lock().await.take()
                    {
                        let _ = map_tx.send(Err(err.into()));
                    }

                    rxs.push(map_rx);
                }

                let res_tx_for_reduce = res_tx.clone();
                let evt_tx_for_reduce = evt_tx.clone();
                let reduce_task: TaskFn = Box::new(move || {
                    Box::pin(async move {
                        let mut vec_m2: Vec<M2> = Vec::new();
                        for res in join_all(rxs).await {
                            match res {
                                Ok(Ok(value)) => vec_m2.push(value),
                                Ok(Err(err)) => {
                                    tracing::error!("map failed: {err}");
                                    if let Some(res_tx) = res_tx_for_reduce.lock().await.take() {
                                        let _ = res_tx.send(Err(err));
                                    }
                                    let _ = evt_tx_for_reduce.send(JobEvent::Done).await;
                                    return;
                                }
                                Err(err) => {
                                    if let Some(res_tx) = res_tx_for_reduce.lock().await.take() {
                                        let _ = res_tx.send(Err(err.into()));
                                    }
                                    let _ = evt_tx_for_reduce.send(JobEvent::Done).await;
                                    return;
                                }
                            }
                        }

                        let result = reduce.reduce(vec_m2).await;
                        if let Err(err) = &result {
                            tracing::error!("reduce failed: {err}");
                        }
                        if let Some(res_tx) = res_tx_for_reduce.lock().await.take() {
                            let _ = res_tx.send(result);
                        }
                        let _ = evt_tx_for_reduce.send(JobEvent::Done).await;
                    })
                });
                if let Err(err) = queue.lock().await.submit(
                    reduce_task,
                    TaskPriority::Lowest,
                    cancel_clone.clone(),
                ) {
                    if let Some(res_tx) = res_tx.lock().await.take() {
                        let _ = res_tx.send(Err(err.into()));
                    }
                    let _ = evt_tx.send(JobEvent::Done).await;
                }
            })
        });

        if let Err(err) = ctx
            .task_queue
            .lock()
            .await
            .submit(task, TaskPriority::Low, cancel)
        {
            if let Some(res_tx) = res_tx_outer.lock().await.take() {
                let _ = res_tx.send(Err(err.into()));
            }
            let _ = evt_tx_outer.send(JobEvent::Done).await;
        }

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
        let res_tx = Arc::new(Mutex::new(Some(res_tx)));
        let (evt_tx, evt_rx) = mpsc::channel(32);

        let res_tx_outer = res_tx.clone();
        let evt_tx_outer = evt_tx.clone();

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
                let res_tx_for_job2 = res_tx.clone();
                let evt_tx_for_job2 = evt_tx.clone();
                let trigger_job2: TaskFn = Box::new(move || {
                    Box::pin(async move {
                        let job1_result = res_rx_1.await.unwrap_or_else(|err| Err(err.into()));

                        if let Err(err) = job1_result {
                            if let Some(res_tx) = res_tx_for_job2.lock().await.take() {
                                let _ = res_tx.send(Err(err));
                            }
                            let _ = evt_tx_for_job2.send(JobEvent::Done).await;
                            return;
                        }

                        let jh_2 = job2.dispatch(ctx2.clone(), (), cancel2.clone()).await;
                        let JobHandle { res_rx, evt_rx } = jh_2;
                        let _ = evt_tx_for_job2
                            .send(JobEvent::NextJob(Box::new(evt_rx)))
                            .await;

                        let job2_result = res_rx.await.unwrap_or_else(|err| Err(err.into()));
                        if let Some(res_tx) = res_tx_for_job2.lock().await.take() {
                            let _ = res_tx.send(job2_result);
                        }
                    })
                });

                if let Err(err) =
                    ctx1.task_queue
                        .lock()
                        .await
                        .submit(trigger_job2, TaskPriority::Lowest, cancel1)
                {
                    if let Some(res_tx) = res_tx.lock().await.take() {
                        let _ = res_tx.send(Err(err.into()));
                    }
                    let _ = evt_tx.send(JobEvent::Done).await;
                }
            })
        });

        if let Err(err) =
            ctx.task_queue
                .lock()
                .await
                .submit(start_job1, TaskPriority::Lowest, cancel)
        {
            if let Some(res_tx) = res_tx_outer.lock().await.take() {
                let _ = res_tx.send(Err(err.into()));
            }
            let _ = evt_tx_outer.send(JobEvent::Done).await;
        }

        JobHandle { res_rx, evt_rx }
    }
}
