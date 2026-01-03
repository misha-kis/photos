use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use crate::steps::RegisterImagesStep;
use photos_core::JobId;
use photos_domain::{DynamicImage, ImageId};
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_import_item_discovery::discover_import_items;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use photos_services::{ImageMetadataRepository, ServiceRegistry};
use photos_workflow::StepContext;
use photos_workflow::errors::JobError;
use photos_workflow::{ProgressReporter, Workflow, run_workflow};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub mod config;
mod errors;
mod service_registry;
mod steps;

pub struct App {
    service_registry: Arc<AppServiceRegistry>,
}

impl App {
    pub async fn new(path: PathBuf, config: config::Config) -> Result<Self, AppError> {
        if !path.exists() {
            std::fs::create_dir(&path).map_err(|_| AppError::BadDirectory)?;
        }
        let image_repository = FSImageRepository::new(
            path.clone(),
            config.thumbnail_sizes.clone(),
            FastImageResizeResizer::default(),
        );
        let image_metadata_repository = SqliteImageMetadataRepository::new(path)
            .await
            .map_err(|_| AppError::BadDirectory)?;
        let resize_service = FastImageResizeResizer::default();
        let service_registry = Arc::new(AppServiceRegistry {
            image_repository: Arc::new(image_repository),
            image_metadata_repository: Arc::new(image_metadata_repository),
            resize_service: Arc::new(resize_service),
        });
        Ok(Self { service_registry })
    }

    pub async fn get_image_ids(&self) -> Result<Vec<ImageId>, AppError> {
        self.service_registry
            .image_metadata_repository
            .get_image_ids()
            .await
            .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() })
    }

    pub async fn discover_import_items(&self, path: PathBuf) -> Result<Vec<PathBuf>, AppError> {
        tokio::task::spawn_blocking(move || discover_import_items(path))
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }

    pub fn import_items(
        &self,
        paths: Vec<PathBuf>,
    ) -> (
        JobId,
        mpsc::Receiver<photos_workflow::WorkflowEvent>,
        JoinHandle<Result<(), JobError>>,
    ) {
        let job_id = JobId::new_v4();
        let (sender, receiver) = tokio::sync::mpsc::channel(16);
        let workflow = Workflow {
            steps: vec![Box::new(RegisterImagesStep { image_paths: paths })],
        };
        let cancel = CancellationToken::new();
        let progress_reporter = ProgressReporter { sender };
        let services = self.service_registry.clone();
        let ctx = StepContext {
            job_id,
            cancel: cancel.clone(),
            progress_reporter,
            services,
        };
        let join = tokio::spawn(async move { run_workflow(workflow, ctx, 1).await });
        (job_id, receiver, join)
    }

    pub async fn get_thumbnail(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, AppError> {
        let service_registry = self.service_registry.clone();
        let image_id = *image_id;
        tokio::task::spawn_blocking(move || {
            service_registry
                .image_repo()
                .get_thumbnail(&image_id, thumbnail_size)
        })
        .await
        .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
        .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })
    }

    pub async fn get_thumbnail_from_file(
        &self,
        path: &Path,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, AppError> {
        let service_registry = self.service_registry.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            service_registry
                .image_repo()
                .get_thumbnail_from_file(&path, thumbnail_size)
        })
        .await
        .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?
        .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })
    }
}
