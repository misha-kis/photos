use crate::AppEvent;
use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Expand;
use crate::jobs::embedding_generation::cluster_embeddings::cluster_embeddings_task;
use crate::jobs::embedding_generation::generate_embeddings::generate_embeddings_task;
use crate::service_registry::AppServiceRegistry;
use photos_domain::{FaceDetection, ImageRecord};
use photos_services::ServiceRegistry;
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

pub(crate) async fn dispatch_embedding_generation_task(
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<Mutex<TaskQueue>>,
    tx: mpsc::Sender<AppEvent>,
    cancel: CancellationToken,
) {
    match service_registry
        .image_meta_repo()
        .get_detections_without_embeddings()
        .await
    {
        Ok(detections_without_embeddings) => {
            tracing::debug!("dispatching jobs for embedding generation");
            let mut new_tasks = Vec::new();
            for (image_record, detection) in detections_without_embeddings {
                let service_registry = service_registry.clone();
                let tx = tx.clone();
                let task: TaskFn = Box::new(move || {
                    Box::pin(generate_embeddings_task(
                        service_registry,
                        image_record,
                        detection,
                        tx,
                    ))
                });
                new_tasks.push((task, TaskPriority::Low));
            }
            let cluster_embeddings_task: TaskFn =
                Box::new(move || Box::pin(cluster_embeddings_task(service_registry, tx)));
            new_tasks.push((cluster_embeddings_task, TaskPriority::Lowest));
            let task_queue = task_queue.lock().await;
            for (task, priority) in new_tasks {
                let _ = task_queue.submit(task, priority, cancel.clone());
            }
        }
        Err(e) => {
            tracing::error!("Failed to get detections without embeddings: {e:?}")
        }
    }
}

pub(crate) struct DiscoverImagesWithoutEmbeddings {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Expand<(), (ImageRecord, FaceDetection)> for DiscoverImagesWithoutEmbeddings {
    async fn expand(&self, _input: ()) -> Result<Vec<(ImageRecord, FaceDetection)>, AppError> {
        self.ctx
            .service_registry
            .image_meta_repo()
            .get_detections_without_embeddings()
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }
}
