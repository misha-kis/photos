use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use photos_core::Uuid;
use photos_domain::{DynamicImage, ImageId};
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_import_item_discovery::discover_import_items;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use photos_services::{ImageMetadataRepository, ServiceRegistry};
use photos_workflow::errors::JobError;
use photos_workflow::{ProgressReporter, Workflow, WorkflowEvent, run_workflow};
use photos_workflow::{Step, StepContext, errors::StepError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub mod config;
mod errors;
mod job_manager;
mod runtime;
mod service_registry;

struct RegisterImagesStep {
    pub image_paths: Vec<PathBuf>,
}

#[async_trait::async_trait]
impl Step for RegisterImagesStep {
    async fn execute(&self, ctx: &StepContext) -> Result<(), StepError> {
        let mut image_records = Vec::new();
        let total_images = self.image_paths.len() as u64;
        tracing::info!("importing {total_images} images");
        for (processed_images, path) in self.image_paths.iter().enumerate() {
            if ctx.cancel.is_cancelled() {
                tracing::warn!("import workflow cancelled");
                return Err(StepError::Cancelled);
            }
            let services = ctx.services.clone();
            let path = path.clone();
            let image_record = tokio::task::spawn_blocking(move || {
                services.image_repo().insert_image(&path)
            })
            .await
            .map_err(|e| StepError::Failed(format!("spawn_blocking failed: {}", e)))?
            .map_err(|e| StepError::Failed(e.to_string()))?;
            image_records.push(image_record);
            ctx.progress_reporter
                .send(WorkflowEvent::StepProgress {
                    job_id: Uuid::nil(),
                    step: self.name(),
                    current: processed_images as u64 + 1,
                    total: total_images,
                })
                .await;
        }
        if ctx.cancel.is_cancelled() {
            return Err(StepError::Cancelled);
        }
        ctx.services
            .image_meta_repo()
            .add_image_record_bulk(&image_records)
            .await
            .map_err(|e| StepError::Failed(e.to_string()))?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "RegisterImagesStep"
    }
}

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
            .map_err(|_| AppError::Unknown)
    }

    pub fn import_items(
        &self,
        paths: Vec<PathBuf>,
    ) -> (
        mpsc::Receiver<WorkflowEvent>,
        JoinHandle<Result<(), JobError>>,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::channel(16);
        let workflow = Workflow {
            steps: vec![Box::new(RegisterImagesStep { image_paths: paths })],
            done: 0,
        };
        let cancel = CancellationToken::new();
        let progress_reporter = ProgressReporter { sender };
        let services = self.service_registry.clone();
        let ctx = StepContext {
            job_id: Uuid::nil(),
            cancel: cancel.clone(),
            progress_reporter,
            services,
        };
        let join = tokio::spawn(async move { run_workflow(workflow, ctx, 1).await });
        (receiver, join)
    }

    pub async fn get_thumbnail(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, AppError> {
        let service_registry = self.service_registry.clone();
        let image_id = *image_id;
        tokio::task::spawn_blocking(move || {
            service_registry.image_repo().get_thumbnail(&image_id, thumbnail_size)
        })
        .await
        .map_err(|_| AppError::Unknown)?
        .map_err(|_| AppError::Unknown)
    }

    pub async fn get_thumbnail_from_file(
        &self,
        path: &Path,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, AppError> {
        let service_registry = self.service_registry.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            service_registry.image_repo().get_thumbnail_from_file(&path, thumbnail_size)
        })
        .await
        .map_err(|_| AppError::Unknown)?
        .map_err(|_| AppError::Unknown)
    }
}
