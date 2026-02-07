use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use crate::jobs::common::{Map, Reduce, TaskContext};
use crate::jobs::face_detection::dispatch_face_detection::dispatch_face_detection_task;
use crate::{AppEvent, ImportJobState};
use photos_core::JobId;
use photos_domain::ImageRecord;
use photos_services::{ImageMetadataRepository, ServiceRegistry};
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;

pub(crate) async fn import_item_task(
    service_registry: Arc<AppServiceRegistry>,
    path: PathBuf,
    job_state: Arc<Mutex<ImportJobState>>,
    tx: mpsc::Sender<AppEvent>,
    job_id: JobId,
    import_jobs: Arc<Mutex<HashMap<JobId, Arc<Mutex<ImportJobState>>>>>,
    task_queue: Arc<Mutex<TaskQueue>>,
    cancel: CancellationToken,
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
            let _ = tx.send(progress_event).await;

            if job_state_guard.completed == job_state_guard.total {
                let image_records = std::mem::take(&mut job_state_guard.image_records);
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
                        let _ = tx_final.send(finish_event).await;

                        let mut jobs = import_jobs_final.lock().await;
                        jobs.remove(&job_id_final);
                        let task_queue_clone = task_queue.clone();
                        let cancel_clone = cancel.clone();
                        let dispatch_face_detection_task: TaskFn = Box::new(move || {
                            Box::pin(dispatch_face_detection_task(
                                service_registry,
                                task_queue_clone,
                                tx,
                                cancel_clone,
                            ))
                        });
                        let _ = task_queue.lock().await.submit(
                            dispatch_face_detection_task,
                            TaskPriority::Lowest,
                            cancel,
                        );
                    }
                    Err(e) => {
                        let finish_event = AppEvent::ImportFinished {
                            job_id: job_id_final,
                            success: false,
                        };
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
            let _ = tx.send(progress_event).await;

            if job_state_guard.completed == job_state_guard.total {
                let image_records = std::mem::take(&mut job_state_guard.image_records);
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
                let _ = tx_final.send(finish_event).await;

                let mut jobs = import_jobs_final.lock().await;
                jobs.remove(&job_id_final);
            }
        }
    }
}

pub(crate) struct CopyItemTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<PathBuf, ImageRecord> for CopyItemTask {
    async fn map(&self, input: PathBuf) -> Result<ImageRecord, AppError> {
        self.ctx
            .service_registry
            .image_repo()
            .insert_image(&input)
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }
}

pub(crate) struct InsertRecordsTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Reduce<ImageRecord, ()> for InsertRecordsTask {
    async fn reduce(&self, inputs: Vec<ImageRecord>) -> Result<(), AppError> {
        self.ctx
            .service_registry
            .image_metadata_repository
            .add_image_record_bulk(&inputs)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }
}
