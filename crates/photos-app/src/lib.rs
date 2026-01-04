use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use crate::steps::RegisterImagesStep;
use photos_core::JobId;
use photos_domain::ImageId;
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_import_item_discovery::discover_import_items;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use photos_services::{ImageMetadataRepository, ServiceRegistry};
use photos_task_queue::{TaskPriority, TaskQueue};
use photos_workflow::StepContext;
use photos_workflow::{ProgressReporter, Workflow, run_workflow};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub mod config;
mod errors;
pub mod events;
mod service_registry;
mod steps;

pub use events::AppEvent;

pub struct App {
    service_registry: Arc<AppServiceRegistry>,
    task_queue: Arc<TaskQueue>,
    event_sender: mpsc::UnboundedSender<AppEvent>,
    _runtime: Runtime,
}

impl App {
    pub fn new(path: PathBuf, config: config::Config) -> Result<Self, AppError> {
        if !path.exists() {
            std::fs::create_dir(&path).map_err(|_| AppError::BadDirectory)?;
        }
        
        let runtime = Runtime::new().map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        
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
        
        let task_queue = Arc::new(TaskQueue::new(runtime.handle().clone()));
        
        Ok(Self {
            service_registry,
            task_queue,
            event_sender,
            _runtime: runtime,
        })
    }

    pub fn get_image_ids(&self) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();
        let handle = self._runtime.handle().clone();
        
        let task = Box::new(move || {
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx;
            let handle = handle.clone();
            
            // Spawn async work
            handle.spawn(async move {
                let result = service_registry
                    .image_metadata_repository
                    .get_image_ids()
                    .await
                    .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() });
                
                let event = events::AppEvent::ImageIdsReady { result };
                let _ = event_sender.send(event.clone());
                let _ = tx.send(event).await;
            });
        });
        
        let _ = self.task_queue.submit(task, TaskPriority::High);
        rx
    }

    pub fn discover_import_items(&self, path: PathBuf) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let event_sender = self.event_sender.clone();
        let handle = self._runtime.handle().clone();
        
        let task = Box::new(move || {
            let event_sender = event_sender.clone();
            let tx = tx;
            let path_clone = path.clone();
            let handle = handle.clone();
            
            handle.spawn(async move {
                let result = tokio::task::spawn_blocking(move || discover_import_items(path_clone))
                    .await
                    .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() });
                
                let event = AppEvent::ImportItemsDiscovered {
                    path,
                    result,
                };
                let _ = event_sender.send(event.clone());
                let _ = tx.send(event).await;
            });
        });
        
        let _ = self.task_queue.submit(task, TaskPriority::High);
        rx
    }

    pub fn import_items(&self, paths: Vec<PathBuf>) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(16);
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();
        let handle = self._runtime.handle().clone();
        
        let task = Box::new(move || {
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx.clone();
            let paths = paths.clone();
            let handle = handle.clone();
            
            handle.spawn(async move {
                let job_id = JobId::new_v4();
                let (workflow_sender, mut workflow_receiver) = tokio::sync::mpsc::channel(16);
                let workflow = Workflow {
                    steps: vec![Box::new(RegisterImagesStep { image_paths: paths })],
                };
                let cancel = CancellationToken::new();
                let progress_reporter = ProgressReporter { sender: workflow_sender };
                let ctx = StepContext {
                    job_id,
                    cancel: cancel.clone(),
                    progress_reporter,
                    services: service_registry,
                };
                
                let event_sender_clone = event_sender.clone();
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    while let Some(workflow_event) = workflow_receiver.recv().await {
                        let app_event = AppEvent::WorkflowEvent { event: workflow_event };
                        let _ = event_sender_clone.send(app_event.clone());
                        let _ = tx_clone.send(app_event).await;
                    }
                });
                
                let _ = run_workflow(workflow, ctx, 1).await;
            });
        });
        
        let _ = self.task_queue.submit(task, TaskPriority::Low);
        rx
    }

    pub fn get_thumbnail(&self, image_id: ImageId, thumbnail_size: u32) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();
        let handle = self._runtime.handle().clone();
        
        let task = Box::new(move || {
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx;
            let image_id = image_id;
            let handle = handle.clone();
            
            handle.spawn(async move {
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
                
                let event = AppEvent::ThumbnailReady {
                    image_id,
                    result,
                };
                let _ = event_sender.send(event.clone());
                let _ = tx.send(event).await;
            });
        });
        
        let _ = self.task_queue.submit(task, TaskPriority::High);
        rx
    }

    pub fn get_thumbnail_from_file(&self, path: PathBuf, thumbnail_size: u32) -> mpsc::Receiver<AppEvent> {
        let (tx, rx) = mpsc::channel(1);
        let service_registry = self.service_registry.clone();
        let event_sender = self.event_sender.clone();
        let handle = self._runtime.handle().clone();
        
        let task = Box::new(move || {
            let service_registry = service_registry.clone();
            let event_sender = event_sender.clone();
            let tx = tx;
            let path_clone = path.clone();
            let handle = handle.clone();
            
            handle.spawn(async move {
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
            });
        });
        
        let _ = self.task_queue.submit(task, TaskPriority::High);
        rx
    }
}
