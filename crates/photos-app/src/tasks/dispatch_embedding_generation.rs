use crate::AppEvent;
use crate::service_registry::AppServiceRegistry;
use crate::tasks::generate_embeddings::generate_embeddings_task;
use photos_services::ServiceRegistry;
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

pub(crate) async fn dispatch_embedding_generation_task(
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<Mutex<TaskQueue>>,
    tx: mpsc::Sender<AppEvent>,
) {
    match service_registry
        .image_meta_repo()
        .get_detections_without_embeddings()
        .await
    {
        Ok(detections_without_embeddings) => {
            tracing::debug!("dispatching tasks for embedding generation");
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
            let task_queue = task_queue.lock().await;
            for (task, priority) in new_tasks {
                let _ = task_queue.submit(task, priority);
            }
        }
        Err(e) => {
            tracing::error!("Failed to get detections without embeddings: {e:?}")
        }
    }
}
