use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use async_trait::async_trait;
use photos_task_queue::task::TaskFn2;
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::fmt::Debug;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use futures::future::join_all;
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct TaskContext {
    pub(crate) service_registry: Arc<AppServiceRegistry>,
    pub(crate) task_queue: Arc<Mutex<TaskQueue>>,
    pub(crate) cancel: CancellationToken,
}

pub(crate) trait Task: Send + Sync {
    type Output;
    async fn run(self, ctx: TaskContext) -> Result<Self::Output, AppError>;
}

pub(crate) trait Job {
    type NextJob: Job;
    type TaskType: Task;
    fn tasks(&self) -> Vec<&Self::TaskType>;
}

impl Job for () {
    type NextJob = ();
    type TaskType = ();
    fn tasks(&self) -> Vec<&Self::TaskType> {
        Default::default()
    }
}

impl Task for () {
    type Output = ();
    async fn run(self, _ctx: TaskContext) -> Result<Self::Output, AppError> {
        Ok(())
    }
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

struct ExpandMapReduce<I, M1, M2, O> {
    expand: Arc<dyn Expand<I, M1>>,
    map: Arc<dyn Map<M1, M2>>,
    reduce: Arc<dyn Reduce<M2, O>>,
}

impl<I: Send + Sync, M1: Send + Sync, M2: Send + Sync + Debug, O: Send + Sync> ExpandMapReduce<I, M1, M2, O> {
    fn run(
        &self,
        ctx: TaskContext,
        task_priority: TaskPriority,
        input: I,
        cancel: CancellationToken,
    ) -> oneshot::Receiver<O> {
        let queue = ctx.task_queue;
        let expand = self.expand.clone();
        let map = self.map.clone();
        let reduce = self.reduce.clone();

        let (tx, rx) = oneshot::channel();

        let task: TaskFn = Box::new(move || {
            Box::pin(async move {
                let vec_m1 = expand.expand(input).await.unwrap();
                let mut rxs = Vec::new();
                for m1 in vec_m1 {
                    let (tx, rx) = oneshot::channel();
                    let map_task: TaskFn = Box::new(move || {
                        Box::pin(async move {
                            let m2 = map.map(m1).await.unwrap();
                            tx.send(m2).expect("cannot send m2");
                        })
                    });
                    queue
                        .lock()
                        .await
                        .submit(map_task, TaskPriority::Low, cancel.clone());
                    rxs.push(rx);
                }

                let vec_m2 = join_all(rxs).await.into_iter().filter(|m2| m2.is_ok()).collect();
                let reduce_task: TaskFn = Box::new(move || {
                    Box::pin(async move {
                        let o = reduce.reduce(vec_m2);
                        tx.send(o).unwrap();
                    })
                });
                queue
                    .lock()
                    .await
                    .submit(reduce_task, TaskPriority::Lowest, cancel.clone());
            })
        });

        ctx.runtime.block_on(async {
            ctx.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::Low, cancel.clone())
        });

        rx
    }
}
