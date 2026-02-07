use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use photos_domain::{ImageId, RgbaImage, Uuid};
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_import_item_discovery::discover_import_items;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use photos_services::{ImageMetadataRepository, ImageRepository};
use photos_task_queue::{TaskFn, TaskPriority, TaskQueue};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, oneshot};
use tokio_util::sync::CancellationToken;

pub mod config;
mod errors;
pub mod events;
mod jobs;
mod service_registry;

use crate::jobs::{
    Dispatchable, GetImageTask, OneshotDispatchable, TaskContext, get_embeddings_detection_job,
    get_face_detection_job, get_import_job,
};
pub use crate::jobs::{JobEvent, JobHandle};
pub use events::AppEvent;
use photos_infra_cv::ImageAnalysis;

pub struct OneshotJobHandle<T> {
    pub cancel: CancellationToken,
    pub rx: oneshot::Receiver<Result<T, AppError>>,
}

pub struct App {
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<Mutex<TaskQueue>>,
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
            runtime,
        };

        app.dispatch_image_analysis();

        Ok(app)
    }

    pub fn get_image_ids(&self) -> OneshotJobHandle<Vec<ImageId>> {
        let (tx, rx) = oneshot::channel();
        let service_registry = self.service_registry.clone();
        let cancel = CancellationToken::new();

        let task: TaskFn = Box::new(move || {
            let service_registry = service_registry.clone();
            let tx = tx;

            Box::pin(async move {
                let result = service_registry
                    .image_metadata_repository
                    .get_image_ids()
                    .await
                    .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() });

                let _ = tx.send(result);
            })
        });
        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High, cancel.clone())
        });
        OneshotJobHandle { cancel, rx }
    }

    pub fn get_face_clusters(&self) -> oneshot::Receiver<AppEvent> {
        let (tx, rx) = oneshot::channel();
        let service_registry = self.service_registry.clone();
        let cancel = CancellationToken::new();

        let task: TaskFn = Box::new(move || {
            Box::pin(async move {
                let result = service_registry
                    .image_metadata_repository
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
                .submit(task, TaskPriority::High, cancel)
        });
        rx
    }

    pub fn get_image(
        &self,
        image_id: ImageId,
        size: (u32, u32),
        cancel: CancellationToken,
    ) -> OneshotJobHandle<RgbaImage> {
        let ctx = TaskContext {
            service_registry: self.service_registry.clone(),
            task_queue: self.task_queue.clone(),
            cancel: cancel.clone(),
        };
        let task = Arc::new(GetImageTask { ctx: ctx.clone() });
        let rx = self.runtime.block_on(async {
            task.dispatch(ctx, image_id, TaskPriority::High, cancel.clone())
                .await
        });
        OneshotJobHandle { cancel, rx }
    }

    pub fn get_face_detection_thumbnail(
        &self,
        detection_id: &Uuid,
        thumbnail_size: u32,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<RgbaImage> {
        let (tx, rx) = oneshot::channel();
        let service_registry = self.service_registry.clone();
        let detection_id = *detection_id;

        let task: TaskFn = Box::new(move || {
            let service_registry = service_registry.clone();
            let tx = tx;

            Box::pin(async move {
                let result = match tokio::task::spawn_blocking({
                    let service_registry = service_registry.clone();
                    let detection_info = service_registry
                        .image_metadata_repository
                        .get_bbox_and_image_for_detection_id(detection_id)
                        .await
                        .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() });
                    move || {
                        let (bounding_box, image_record) = detection_info?;
                        service_registry
                            .image_repository
                            .get_face_thumbnail(&image_record, bounding_box, thumbnail_size)
                            .map(|image| image.to_rgba8())
                            .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })
                    }
                })
                .await
                {
                    Ok(Ok(image)) => Ok(image),
                    Ok(Err(e)) => Err(e),
                    Err(e) => Err(AppError::TaskSpawnFailed { err: e.to_string() }),
                };

                let _ = tx.send(result);
            })
        });

        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High, cancel.clone())
        });
        OneshotJobHandle { cancel, rx }
    }

    pub fn discover_import_items(
        &self,
        path: PathBuf,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<Vec<PathBuf>> {
        let (tx, rx) = oneshot::channel();

        let task: TaskFn = Box::new(move || {
            let tx = tx;
            let path_clone = path.clone();

            Box::pin(async move {
                let result = tokio::task::spawn_blocking(move || discover_import_items(path_clone))
                    .await
                    .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() });

                let _ = tx.send(result);
            })
        });

        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High, cancel.clone())
        });
        OneshotJobHandle { cancel, rx }
    }

    pub fn import_items(&self, paths: Vec<PathBuf>) -> JobHandle {
        let cancel = CancellationToken::new();
        let ctx = TaskContext {
            service_registry: self.service_registry.clone(),
            task_queue: self.task_queue.clone(),
            cancel: CancellationToken::new(),
        };
        let import_job = Arc::new(get_import_job(ctx.clone()));
        let face_detection_job = Arc::new(get_face_detection_job(ctx.clone()));
        let embedding_job = Arc::new(get_embeddings_detection_job(ctx.clone()));
        let processing_job = Arc::new((face_detection_job, embedding_job));
        let jobs = (import_job, processing_job);
        self.runtime
            .block_on(async { jobs.dispatch(ctx, paths, cancel).await })
    }

    pub fn dispatch_image_analysis(&self) -> JobHandle {
        let cancel = CancellationToken::new();
        let ctx = TaskContext {
            service_registry: self.service_registry.clone(),
            task_queue: self.task_queue.clone(),
            cancel: CancellationToken::new(),
        };
        let face_detection_job = Arc::new(get_face_detection_job(ctx.clone()));
        let embedding_job = Arc::new(get_embeddings_detection_job(ctx.clone()));
        let jobs = (face_detection_job, embedding_job);
        self.runtime
            .block_on(async { jobs.dispatch(ctx, (), cancel).await })
    }

    pub fn get_thumbnail(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<RgbaImage> {
        let (tx, rx) = oneshot::channel();
        let service_registry = self.service_registry.clone();
        let image_id = *image_id;

        let task: TaskFn = Box::new(move || {
            let service_registry = service_registry.clone();
            let tx = tx;

            Box::pin(async move {
                let result = match tokio::task::spawn_blocking({
                    let service_registry = service_registry.clone();
                    let image_id = image_id;
                    move || {
                        service_registry
                            .image_repository
                            .get_thumbnail(&image_id, thumbnail_size)
                            .map(|image| image.into_rgba8())
                    }
                })
                .await
                {
                    Ok(Ok(image)) => Ok(image),
                    Ok(Err(e)) => Err(AppError::ImageRepositoryError { err: e.to_string() }),
                    Err(e) => Err(AppError::TaskSpawnFailed { err: e.to_string() }),
                };

                let _ = tx.send(result);
            })
        });

        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High, cancel.clone())
        });
        OneshotJobHandle { cancel, rx }
    }

    pub fn get_thumbnail_from_file(
        &self,
        path: PathBuf,
        thumbnail_size: u32,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<RgbaImage> {
        let (tx, rx) = oneshot::channel();
        let service_registry = self.service_registry.clone();

        let task: TaskFn = Box::new(move || {
            let service_registry = service_registry.clone();
            let tx = tx;
            let path_clone = path.clone();

            Box::pin(async move {
                let result = match tokio::task::spawn_blocking({
                    let service_registry = service_registry.clone();
                    let path = path_clone.clone();
                    move || {
                        service_registry
                            .image_repository
                            .get_thumbnail_from_file(&path, thumbnail_size)
                            .map(|image| image.into_rgba8())
                    }
                })
                .await
                {
                    Ok(Ok(image)) => Ok(image),
                    Ok(Err(e)) => Err(AppError::ImageRepositoryError { err: e.to_string() }),
                    Err(e) => Err(AppError::TaskSpawnFailed { err: e.to_string() }),
                };

                let _ = tx.send(result);
            })
        });

        let _ = self.runtime.block_on(async {
            self.task_queue
                .lock()
                .await
                .submit(task, TaskPriority::High, cancel.clone())
        });
        OneshotJobHandle { cancel, rx }
    }
}
