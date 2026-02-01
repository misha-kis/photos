use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use photos_core::{JobId, Uuid};
use photos_domain::{ImageId, ImageRecord};
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_import_item_discovery::discover_import_items;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use photos_services::{ImageMetadataRepository, ServiceRegistry};
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::sync::{Mutex, oneshot};

pub mod config;
mod errors;
pub mod events;
mod service_registry;
mod tasks;

use crate::tasks::import_item_task;
pub use events::AppEvent;
use photos_infra_cv::ImageAnalysis;

struct ImportJobState {
    total: u64,
    completed: u64,
    image_records: Vec<ImageRecord>,
}

pub struct App {
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<Mutex<TaskQueue>>,
    import_jobs: Arc<Mutex<HashMap<JobId, Arc<Mutex<ImportJobState>>>>>,
    runtime: Runtime,
}

impl App {
    pub fn new(path: PathBuf, app_options: config::Options) -> Result<Self, AppError> {
        if !path.exists() {
            std::fs::create_dir(&path)
                .map_err(|e| AppError::BadDirectory { err: e.to_string() })?;
        }

        let runtime =
            Runtime::new().map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;

        let image_repository = FSImageRepository::new(
            path.clone(),
            app_options.thumbnail_sizes.clone(),
            FastImageResizeResizer::default(),
        );

        let image_metadata_repository = runtime.block_on(async {
            SqliteImageMetadataRepository::new(path)
                .await
                .map_err(|e| AppError::BadDirectory { err: e.to_string() })
        })?;

        let analysis_service = ImageAnalysis::new(app_options.image_analysis_config)
            .map_err(|e| AppError::BadDirectory { err: e.to_string() })?;

        let resize_service = FastImageResizeResizer::default();
        let service_registry = Arc::new(AppServiceRegistry {
            image_repository: Arc::new(image_repository),
            image_metadata_repository: Arc::new(image_metadata_repository),
            resize_service: Arc::new(resize_service),
            analysis_service: Arc::new(analysis_service),
        });

        let task_queue = Arc::new(Mutex::new(TaskQueue::new(
            runtime.handle().clone(),
            app_options.max_blocking_tasks,
        )));

        let app = Self {
            service_registry,
            task_queue,
            import_jobs: Arc::new(Mutex::new(HashMap::new())),
            runtime,
        };

        app.dispatch_image_analysis();

        Ok(app)
    }

    pub fn get_image_ids(&self) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();

        let task: TaskFn = Box::new(move || {
            let service_registry = service_registry.clone();
            let tx = tx;

            Box::pin(async move {
                let result = service_registry
                    .image_metadata_repository
                    .get_image_ids()
                    .await
                    .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() });

                let event = AppEvent::ImageIdsReady { result };
                let _ = tx.send(event).await;
            })
        });
        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High)
        });
        rx
    }

    pub fn get_face_clusters(&self) -> oneshot::Receiver<AppEvent> {
        let (tx, rx) = oneshot::channel();
        let service_registry = self.service_registry.clone();

        let task: TaskFn = Box::new(move || {
            Box::pin(async move {
                let result = service_registry
                    .image_meta_repo()
                    .get_face_clusters()
                    .await
                    .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() });
                let event = AppEvent::FaceClustersReady { result };
                let _ = tx.send(event);
            })
        });

        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High)
        });
        rx
    }

    pub fn get_face_detection_thumbnail(
        &self,
        detection_id: Uuid,
        thumbnail_size: u32,
    ) -> oneshot::Receiver<AppEvent> {
        let (tx, rx) = oneshot::channel();
        let service_registry = self.service_registry.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let service_registry = service_registry.clone();
            let tx = tx;

            Box::pin(async move {
                let result = match tokio::task::spawn_blocking({
                    let service_registry = service_registry.clone();
                    let detection_id = detection_id;
                    let detection_info = service_registry
                        .image_meta_repo()
                        .get_bbox_and_image_for_detection_id(detection_id)
                        .await
                        .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() });
                    move || {
                        let (bounding_box, image_record) = detection_info?;
                        service_registry
                            .image_repo()
                            .get_face_thumbnail(&image_record, bounding_box, thumbnail_size)
                            .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })
                    }
                })
                .await
                {
                    Ok(Ok(image)) => Ok(image),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(AppError::TaskSpawnFailed { err: e.to_string() }),
                };

                let event = AppEvent::FaceDetectionThumbnailReady { detection_id, result };
                let _ = tx.send(event);
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

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let tx = tx;
            let path_clone = path.clone();

            Box::pin(async move {
                let result = tokio::task::spawn_blocking(move || discover_import_items(path_clone))
                    .await
                    .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() });

                let event = AppEvent::ImportItemsDiscovered { path, result };
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
        let import_jobs = self.import_jobs.clone();

        let job_state = Arc::new(Mutex::new(ImportJobState {
            total,
            completed: 0,
            image_records: Vec::new(),
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
        let _ = self
            .runtime
            .block_on(async { tx.send(initial_event).await });

        for path in paths.into_iter() {
            let job_state = job_state.clone();
            let service_registry = service_registry.clone();
            let tx = tx.clone();
            let import_jobs = import_jobs.clone();
            let task_queue = self.task_queue.clone();

            let task: Box<
                dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
            > = Box::new(move || {
                Box::pin(import_item_task(
                    service_registry,
                    path,
                    job_state,
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

    pub fn dispatch_image_analysis(&self) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(16);
        let job_id = JobId::new_v4();
        let service_registry = self.service_registry.clone();
        let import_jobs = self.import_jobs.clone();

        let job_state = Arc::new(Mutex::new(ImportJobState {
            total: 0,
            completed: 0,
            image_records: Vec::new(),
        }));

        {
            let mut jobs = self.runtime.block_on(async { import_jobs.lock().await });
            jobs.insert(job_id, job_state.clone());
        }

        let service_registry = service_registry.clone();
        let tx = tx.clone();
        let task_queue = self.task_queue.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            Box::pin(tasks::dispatch_face_detection_task(
                service_registry,
                task_queue,
                tx,
            )) as std::pin::Pin<Box<dyn Future<Output = ()> + Send>>
        });

        let _ = self
            .runtime
            .block_on(async { self.task_queue.lock().await.submit(task, TaskPriority::Low) });

        rx
    }

    pub fn get_thumbnail(
        &self,
        image_id: ImageId,
        thumbnail_size: u32,
    ) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let service_registry = service_registry.clone();
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

    pub fn get_thumbnail_from_file(
        &self,
        path: PathBuf,
        thumbnail_size: u32,
    ) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();

        let task: Box<
            dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
        > = Box::new(move || {
            let service_registry = service_registry.clone();
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
}
