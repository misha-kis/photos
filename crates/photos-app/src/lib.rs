pub use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use photos_domain::{ImageId, RgbaImage, Uuid};
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use photos_services::ImageRepository;
use photos_task_queue::{TaskPriority, TaskQueue};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub mod config;
mod errors;
mod jobs;
mod service_registry;

use crate::jobs::{
    DiscoverImportItemsTask, Dispatchable, GetFaceClustersTask, GetFaceDetectionThumbnailTask,
    GetImageIdsTask, GetImagePathTask, GetImageTask, GetThumbnailFromFileTask, GetThumbnailTask,
    OneshotDispatchable, TaskContext, get_embeddings_detection_job, get_face_detection_job,
    get_import_job,
};
pub use crate::jobs::{JobEvent, JobHandle};
use photos_infra_cv::ImageAnalysis;

pub struct App {
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<Mutex<TaskQueue>>,
}

impl App {
    pub async fn new(path: PathBuf, app_options: config::Options) -> Result<Self, AppError> {
        if !path.exists() {
            std::fs::create_dir(&path)
                .map_err(|e| AppError::BadDirectory { err: e.to_string() })?;
        }

        let image_repository = FSImageRepository::new(
            path.clone(),
            app_options.thumbnail_sizes.clone(),
            FastImageResizeResizer::default(),
        );

        let image_metadata_repository = SqliteImageMetadataRepository::new(path)
            .await
            .map_err(|e| AppError::Internal(Box::from(e)))?;

        let analysis_service = ImageAnalysis::new(app_options.image_analysis_config)
            .map_err(|e| AppError::Internal(Box::from(e)))?;

        let resize_service = FastImageResizeResizer::default();
        let service_registry = Arc::new(AppServiceRegistry {
            image_repository: Arc::new(image_repository),
            image_metadata_repository: Arc::new(image_metadata_repository),
            resize_service: Arc::new(resize_service),
            analysis_service: Arc::new(analysis_service),
        });

        let task_queue = Arc::new(Mutex::new(TaskQueue::new(
            tokio::runtime::Handle::current(),
            app_options.max_blocking_tasks,
        )));

        let app = Self {
            service_registry,
            task_queue,
        };

        let analysis_service_registry = app.service_registry.clone();
        let analysis_task_queue = app.task_queue.clone();
        tokio::spawn(async move {
            let cancel = CancellationToken::new();
            let ctx = TaskContext {
                service_registry: analysis_service_registry,
                task_queue: analysis_task_queue,
            };
            let face_detection_job = Arc::new(get_face_detection_job(ctx.clone()));
            let embedding_job = Arc::new(get_embeddings_detection_job(ctx.clone()));
            let jobs = (face_detection_job, embedding_job);
            let _ = jobs.dispatch(ctx, (), cancel).await;
        });

        Ok(app)
    }

    fn task_context(&self) -> TaskContext {
        TaskContext {
            service_registry: self.service_registry.clone(),
            task_queue: self.task_queue.clone(),
        }
    }

    #[allow(clippy::async_yields_async)]
    pub async fn get_image_ids_async(&self) -> Result<Vec<ImageId>, AppError> {
        let ctx = self.task_context();
        let task = Arc::new(GetImageIdsTask { ctx });
        let receiver = task
            .dispatch(
                self.task_context(),
                (),
                TaskPriority::High,
                CancellationToken::new(),
            )
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }

    #[allow(clippy::async_yields_async)]
    pub async fn get_face_clusters_async(&self) -> Result<Vec<(Uuid, Vec<Uuid>)>, AppError> {
        let ctx = self.task_context();
        let task = Arc::new(GetFaceClustersTask { ctx });
        let receiver = task
            .dispatch(
                self.task_context(),
                (),
                TaskPriority::High,
                CancellationToken::new(),
            )
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }

    #[allow(clippy::async_yields_async)]
    pub async fn get_image_async(
        &self,
        image_id: ImageId,
        size: Option<(u32, u32)>,
        cancel: CancellationToken,
    ) -> Result<RgbaImage, AppError> {
        let ctx = self.task_context();
        let task = Arc::new(GetImageTask { ctx: ctx.clone() });
        let receiver = task
            .dispatch(ctx, (image_id, size), TaskPriority::High, cancel)
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }

    #[allow(clippy::async_yields_async)]
    pub async fn get_face_detection_thumbnail_async(
        &self,
        detection_id: Uuid,
        thumbnail_size: u32,
        cancel: CancellationToken,
    ) -> Result<RgbaImage, AppError> {
        let ctx = self.task_context();
        let task = Arc::new(GetFaceDetectionThumbnailTask { ctx: ctx.clone() });
        let receiver = task
            .dispatch(
                ctx,
                (detection_id, thumbnail_size),
                TaskPriority::High,
                cancel,
            )
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }

    #[allow(clippy::async_yields_async)]
    pub async fn discover_import_items_async(
        &self,
        path: PathBuf,
        cancel: CancellationToken,
    ) -> Result<Vec<PathBuf>, AppError> {
        let task = Arc::new(DiscoverImportItemsTask {});
        let receiver = task
            .dispatch(self.task_context(), path, TaskPriority::High, cancel)
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }

    pub async fn import_items(&self, paths: Vec<PathBuf>) -> JobHandle {
        let cancel = CancellationToken::new();
        let ctx = TaskContext {
            service_registry: self.service_registry.clone(),
            task_queue: self.task_queue.clone(),
        };
        let import_job = Arc::new(get_import_job(ctx.clone()));
        let face_detection_job = Arc::new(get_face_detection_job(ctx.clone()));
        let embedding_job = Arc::new(get_embeddings_detection_job(ctx.clone()));
        let processing_job = Arc::new((face_detection_job, embedding_job));
        let jobs = (import_job, processing_job);
        jobs.dispatch(ctx, paths, cancel).await
    }

    pub async fn dispatch_image_analysis(&self) -> JobHandle {
        let cancel = CancellationToken::new();
        let ctx = TaskContext {
            service_registry: self.service_registry.clone(),
            task_queue: self.task_queue.clone(),
        };
        let face_detection_job = Arc::new(get_face_detection_job(ctx.clone()));
        let embedding_job = Arc::new(get_embeddings_detection_job(ctx.clone()));
        let jobs = (face_detection_job, embedding_job);
        jobs.dispatch(ctx, (), cancel).await
    }

    #[allow(clippy::async_yields_async)]
    pub async fn get_thumbnail_async(
        &self,
        image_id: ImageId,
        thumbnail_size: u32,
        cancel: CancellationToken,
    ) -> Result<RgbaImage, AppError> {
        let ctx = self.task_context();
        let task = Arc::new(GetThumbnailTask { ctx });
        let receiver = task
            .dispatch(
                self.task_context(),
                (image_id, thumbnail_size),
                TaskPriority::High,
                cancel,
            )
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }

    #[allow(clippy::async_yields_async)]
    pub async fn get_thumbnail_from_file_async(
        &self,
        path: PathBuf,
        thumbnail_size: u32,
        cancel: CancellationToken,
    ) -> Result<RgbaImage, AppError> {
        let ctx = self.task_context();
        let task = Arc::new(GetThumbnailFromFileTask { ctx });
        let receiver = task
            .dispatch(
                self.task_context(),
                (path, thumbnail_size),
                TaskPriority::High,
                cancel,
            )
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }

    pub fn get_thumbnail_path(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
    ) -> Result<PathBuf, AppError> {
        self.service_registry
            .image_repository
            .get_thumbnail_path(&image_id, thumbnail_size)
            .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })
    }

    pub async fn get_original_path(&self, image_id: ImageId) -> Result<PathBuf, AppError> {
        let ctx = self.task_context();
        let task = Arc::new(GetImagePathTask { ctx: ctx.clone() });
        let cancel = CancellationToken::default();
        let receiver = task
            .dispatch(ctx, image_id, TaskPriority::High, cancel)
            .await;
        receiver
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
    }
}
