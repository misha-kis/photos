use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use crate::tasks::dispatch_face_detection::dispatch_face_detection_task;
use crate::{AppEvent, ImportJobState};
use photos_core::JobId;
use photos_services::{ImageMetadataRepository, ServiceRegistry};
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

pub(crate) async fn import_item_task(
    service_registry: Arc<AppServiceRegistry>,
    path: PathBuf,
    job_state: Arc<Mutex<ImportJobState>>,
    event_sender: mpsc::UnboundedSender<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
    job_id: JobId,
    import_jobs: Arc<Mutex<HashMap<JobId, Arc<Mutex<ImportJobState>>>>>,
    task_queue: Arc<Mutex<TaskQueue>>,
) {
    let image_record_result = service_registry
        .image_repo()
        .insert_image(&path)
        .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() });

    let mut job_state_guard = job_state.lock().await;

    match image_record_result {
        Ok(image_record) => {
            job_state_guard.image_records.push(image_record);
            job_state_guard.completed += 1;

            let progress_event = AppEvent::ImportProgress {
                job_id,
                current: job_state_guard.completed,
                total: job_state_guard.total,
            };
            let _ = event_sender.send(progress_event.clone());
            let _ = tx.send(progress_event).await;

            if job_state_guard.completed == job_state_guard.total {
                let image_records = std::mem::take(&mut job_state_guard.image_records);
                let event_sender_final = event_sender.clone();
                let tx_final = tx.clone();
                let job_id_final = job_id;
                let import_jobs_final = import_jobs.clone();

                drop(job_state_guard);

                match service_registry
                    .image_metadata_repository
                    .add_image_record_bulk(&image_records)
                    .await
                {
                    Ok(_) => {
                        let finish_event = AppEvent::ImportFinished {
                            job_id: job_id_final,
                            success: true,
                        };
                        let _ = event_sender_final.send(finish_event.clone());
                        let _ = tx_final.send(finish_event).await;

                        let mut jobs = import_jobs_final.lock().await;
                        jobs.remove(&job_id_final);
                        let task_queue_clone = task_queue.clone();
                        let dispatch_face_detection_task: TaskFn = Box::new(move || {
                            Box::pin(dispatch_face_detection_task(
                                service_registry,
                                task_queue_clone,
                                tx,
                            ))
                        });
                        let _ = task_queue
                            .lock()
                            .await
                            .submit(dispatch_face_detection_task, TaskPriority::Lowest);
                    }
                    Err(e) => {
                        let finish_event = AppEvent::ImportFinished {
                            job_id: job_id_final,
                            success: false,
                        };
                        let _ = event_sender_final.send(finish_event.clone());
                        let _ = tx_final.send(finish_event).await;

                        let mut jobs = import_jobs_final.lock().await;
                        jobs.remove(&job_id_final);

                        tracing::error!("Failed to save image records: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            job_state_guard.completed += 1;
            tracing::error!("Failed to import image {}: {}", path.display(), e);

            let progress_event = AppEvent::ImportProgress {
                job_id,
                current: job_state_guard.completed,
                total: job_state_guard.total,
            };
            let _ = event_sender.send(progress_event.clone());
            let _ = tx.send(progress_event).await;

            if job_state_guard.completed == job_state_guard.total {
                let image_records = std::mem::take(&mut job_state_guard.image_records);
                let event_sender_final = event_sender.clone();
                let tx_final = tx.clone();
                let job_id_final = job_id;
                let import_jobs_final = import_jobs.clone();

                drop(job_state_guard);

                if !image_records.is_empty() {
                    let _ = service_registry
                        .image_metadata_repository
                        .add_image_record_bulk(&image_records)
                        .await;
                }

                let finish_event = AppEvent::ImportFinished {
                    job_id: job_id_final,
                    success: !image_records.is_empty(),
                };
                let _ = event_sender_final.send(finish_event.clone());
                let _ = tx_final.send(finish_event).await;

                let mut jobs = import_jobs_final.lock().await;
                jobs.remove(&job_id_final);
            }
        }
    }
}
