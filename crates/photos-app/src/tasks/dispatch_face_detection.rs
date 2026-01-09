use crate::AppEvent;
use crate::service_registry::AppServiceRegistry;
use crate::tasks::detect_faces::detect_faces_task;
use crate::tasks::dispatch_embedding_generation::dispatch_embedding_generation_task;
use photos_services::ImageMetadataRepository;
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

pub(crate) async fn dispatch_face_detection_task(
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<Mutex<TaskQueue>>,
    tx: mpsc::Sender<AppEvent>,
) {
    tracing::info!("getting images without detections");
    if let Ok(image_records_without_detections) = service_registry
        .image_metadata_repository
        .get_image_records_without_detections()
        .await
    {
        tracing::debug!("dispatching tasks for face detection");

        let mut new_tasks = Vec::new();
        for image_record in image_records_without_detections {
            let service_registry = service_registry.clone();
            let tx = tx.clone();
            let task: TaskFn = Box::new(move || {
                Box::pin(async move { detect_faces_task(service_registry, image_record, tx).await })
                    as std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
            });
            new_tasks.push((task, TaskPriority::Low));
        }
        let task_queue_clone = task_queue.clone();
        let dispatch_embedding_generation_task: TaskFn = Box::new(move || {
            Box::pin(dispatch_embedding_generation_task(
                service_registry,
                task_queue_clone,
                tx,
            ))
        });
        new_tasks.push((dispatch_embedding_generation_task, TaskPriority::Lowest));

        let task_queue = task_queue.lock().await;
        for (task, priority) in new_tasks {
            let _ = task_queue.submit(task, priority);
        }
    }
}
