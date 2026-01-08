use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use photos_core::JobId;
use photos_domain::{ImageId, ImageRecord};
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_import_item_discovery::discover_import_items;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use photos_services::{ImageMetadataRepository, ServiceRegistry};
use photos_task_queue::{TaskPriority, TaskQueue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

pub mod config;
mod errors;
pub mod events;
mod service_registry;

pub use events::AppEvent;

struct ImportJobState {
    job_id: JobId,
    total: u64,
    completed: u64,
    image_records: Vec<ImageRecord>,
    event_sender: mpsc::UnboundedSender<AppEvent>,
    result_sender: mpsc::Sender<AppEvent>,
}

pub struct App {
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<Mutex<TaskQueue>>,
    event_sender: mpsc::UnboundedSender<AppEvent>,
    import_jobs: Arc<Mutex<HashMap<JobId, Arc<Mutex<ImportJobState>>>>>,
    runtime: Runtime,
}

impl App {
    pub fn new(path: PathBuf, config: config::Config) -> Result<Self, AppError> {
        if !path.exists() {
            std::fs::create_dir(&path).map_err(|_| AppError::BadDirectory)?;
        }

        let runtime =
            Runtime::new().map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;

        let image_repository = FSImageRepository::new(
            path.clone(),
            config.thumbnail_sizes.clone(),
            FastImageResizeResizer::default(),
        );

        let image_metadata_repository = runtime.block_on(async {
            SqliteImageMetadataRepository::new(path)
                .await
                .map_err(|_| AppError::BadDirectory)
        })?;

        let resize_service = FastImageResizeResizer::default();
        let service_registry = Arc::new(AppServiceRegistry {
            image_repository: Arc::new(image_repository),
            image_metadata_repository: Arc::new(image_metadata_repository),
            resize_service: Arc::new(resize_service),
        });

        let (event_sender, _event_receiver) = mpsc::unbounded_channel();

        let task_queue = Arc::new(Mutex::new(TaskQueue::new(
            runtime.handle().clone(),
            config.max_blocking_tasks,
        )));

        Ok(Self {
            service_registry,
            task_queue,
            event_sender,
            import_jobs: Arc::new(Mutex::new(HashMap::new())),
            runtime,
        })
    }

    pub fn get_image_ids(&self) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx;

            Box::pin(async move {
                let result = service_registry
                    .image_metadata_repository
                    .get_image_ids()
                    .await
                    .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() });

                let event = events::AppEvent::ImageIdsReady { result };
                let _ = event_sender.send(event.clone());
                let _ = tx.send(event).await;
            }) as std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        });
        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High)
        });
        rx
    }

    pub fn discover_import_items(&self, path: PathBuf) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let event_sender = self.event_sender.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let event_sender = event_sender.clone();
            let tx = tx;
            let path_clone = path.clone();

            Box::pin(async move {
                let result = tokio::task::spawn_blocking(move || discover_import_items(path_clone))
                    .await
                    .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() });

                let event = AppEvent::ImportItemsDiscovered { path, result };
                let _ = event_sender.send(event.clone());
                let _ = tx.send(event).await;
            }) as std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        });

        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High)
        });
        rx
    }

    pub fn import_items(&self, paths: Vec<PathBuf>) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(16);
        let job_id = JobId::new_v4();
        let total = paths.len() as u64;
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();
        let import_jobs = self.import_jobs.clone();

        let job_state = Arc::new(Mutex::new(ImportJobState {
            job_id,
            total,
            completed: 0,
            image_records: Vec::new(),
            event_sender: event_sender.clone(),
            result_sender: tx.clone(),
        }));

        {
            let mut jobs = self.runtime.block_on(async { import_jobs.lock().await });
            jobs.insert(job_id, job_state.clone());
        }

        let initial_event = AppEvent::ImportProgress {
            job_id,
            current: 0,
            total,
        };
        let _ = event_sender.send(initial_event.clone());
        let _ = self
            .runtime
            .block_on(async { tx.send(initial_event).await });

        for path in paths.into_iter() {
            let job_state = job_state.clone();
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx.clone();
            let import_jobs = import_jobs.clone();
            let task_queue = self.task_queue.clone();

            let task: Box<
                dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
            > = Box::new(move || {
                Box::pin(import_item_task_function(
                    service_registry,
                    path,
                    job_state,
                    event_sender,
                    tx,
                    job_id,
                    import_jobs,
                    task_queue,
                )) as std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
            });

            let _ = self
                .runtime
                .block_on(async { self.task_queue.lock().await.submit(task, TaskPriority::Low) });
        }

        rx
    }

    pub fn get_thumbnail(
        &self,
        image_id: ImageId,
        thumbnail_size: u32,
    ) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx;
            let image_id = image_id;

            Box::pin(async move {
                let result = match tokio::task::spawn_blocking({
                    let service_registry = service_registry.clone();
                    let image_id = image_id;
                    move || {
                        service_registry
                            .image_repo()
                            .get_thumbnail(&image_id, thumbnail_size)
                    }
                })
                .await
                {
                    Ok(Ok(image)) => Ok(image),
                    Ok(Err(e)) => Err(AppError::ImageRepositoryError { err: e.to_string() }),
                    Err(e) => Err(AppError::TaskSpawnFailed { err: e.to_string() }),
                };

                let event = AppEvent::ThumbnailReady { image_id, result };
                let _ = event_sender.send(event.clone());
                let _ = tx.send(event).await;
            }) as std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        });

        let _ = self
            .runtime
            .block_on(async { self.task_queue.lock().await.submit(task, TaskPriority::Low) });
        rx
    }

    pub fn get_thumbnail_from_file(
        &self,
        path: PathBuf,
        thumbnail_size: u32,
    ) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx;
            let path_clone = path.clone();

            Box::pin(async move {
                let result = match tokio::task::spawn_blocking({
                    let service_registry = service_registry.clone();
                    let path = path_clone.clone();
                    move || {
                        service_registry
                            .image_repo()
                            .get_thumbnail_from_file(&path, thumbnail_size)
                    }
                })
                .await
                {
                    Ok(Ok(image)) => Ok(image),
                    Ok(Err(e)) => Err(AppError::ImageRepositoryError { err: e.to_string() }),
                    Err(e) => Err(AppError::TaskSpawnFailed { err: e.to_string() }),
                };

                let event = AppEvent::ThumbnailFromFileReady {
                    path: path_clone,
                    result,
                };
                let _ = event_sender.send(event.clone());
                let _ = tx.send(event).await;
            }) as std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        });

        let _ = self
            .runtime
            .block_on(async { self.task_queue.lock().await.submit(task, TaskPriority::Low) });
        rx
    }
}

async fn import_item_task_function(
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
