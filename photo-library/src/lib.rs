mod config;
mod workers;

use crate::workers::cv_worker::{
    CreateEmbeddingCommand, CreateEmbeddingCommandResult, CvWorker, DetectFacesCommand,
    DetectFacesCommandResult,
};
use crate::workers::db_worker::DbWorker;
use crate::workers::image_loader_worker::{
    ImageLoader, LoadImageCommand, LoadImageCommandResult, LoadThumbnailCommand,
    LoadThumbnailCommandResult,
};
use crate::workers::import_worker::{ImportCommand, ImportCommandResult, ImportWorker};
use anyhow::{Context, Result, anyhow};
use image::DynamicImage;
use rayon::ThreadPoolBuilder;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs::create_dir;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

pub use crate::config::Config;
pub use crate::workers::cv_worker::CvConfig;

const THUMBNAILS_SUBDIRECTORY: &str = "thumbnails";
const ORIGINALS_SUBDIRECTORY: &str = "originals";
const DB_PATH: &str = "db.db";

#[derive(Debug)]
enum Command {
    LoadImage(LoadImageCommand),
    LoadThumbnail(LoadThumbnailCommand),
    Import(ImportCommand),
    DetectFaces(DetectFacesCommand),
    EmbedFace(CreateEmbeddingCommand),
}

enum CommandResult {
    LoadImage(LoadImageCommandResult),
    LoadThumbnail(LoadThumbnailCommandResult),
    Import(ImportCommandResult),
    DetectFaces(DetectFacesCommandResult),
    EmbedFace(CreateEmbeddingCommandResult),
}

#[derive(Clone)]
pub struct SchedulerHandle {
    ui_tx: mpsc::Sender<Command>,
    bg_tx: mpsc::Sender<Command>,
    // cancel: CancellationToken,
}

pub struct Scheduler {
    ui_rx: mpsc::Receiver<Command>,
    bg_rx: mpsc::Receiver<Command>,
    image_loader: Arc<Mutex<ImageLoader>>,
    import_worker: ImportWorker,
    cv_worker: CvWorker,
    // rayon_pool: Arc<rayon::ThreadPool>,
    // progress_tx: broadcast::Sender<ProgressEvent>,
    cancel: CancellationToken,
}

impl Scheduler {
    fn new(
        image_loader: Arc<Mutex<ImageLoader>>,
        import_worker: ImportWorker,
        cv_worker: CvWorker,
    ) -> SchedulerHandle {
        let (ui_tx, ui_rx) = mpsc::channel(256); // priority queue
        let (bg_tx, bg_rx) = mpsc::channel(1024); // background queue
        let cancel = CancellationToken::new();

        // let rayon_pool = ThreadPoolBuilder::new()
        //     .num_threads(num_cpus::get_physical().max(1))
        //     .build()
        //     .unwrap();

        // let (progress_tx, _rx) = broadcast::channel(64);

        let mut sched = Scheduler {
            ui_rx,
            bg_rx,
            image_loader,
            import_worker,
            cv_worker,
            // rayon_pool: Arc::new(rayon_pool),
            // progress_tx,
            cancel: cancel.clone(),
        };
        tracing::debug!("Scheduler initialized");

        let handle = SchedulerHandle {
            ui_tx,
            bg_tx,
            // cancel,
        };
        let handle_clone = handle.clone();

        tokio::spawn(async move {
            tracing::debug!("Starting scheduler thread");
            sched
                .run(handle_clone)
                .await
                .expect("error running scheduler");
            tracing::info!("Scheduler thread stopped");
        });
        tracing::info!("Scheduler started");

        handle
    }

    async fn run(&mut self, handle: SchedulerHandle) -> Result<()> {
        tracing::debug!("Starting scheduling loop");
        loop {
            tokio::select! {
                biased;
                Some(task) = self.ui_rx.recv() => self.handle_task(task, &handle).await.context("handling ui task")?,
                Some(task) = self.bg_rx.recv() => self.handle_task(task, &handle).await.context("handling bg task")?,
                _ = self.cancel.cancelled() => break,
            }
        }
        tracing::info!("Scheduler loop stopped");
        Ok(())
    }

    async fn handle_task(&mut self, cmd: Command, handle: &SchedulerHandle) -> Result<()> {
        tracing::debug!("Handling task: {:?}", cmd);
        match cmd {
            Command::LoadThumbnail(cmd) => cmd
                .execute(&mut self.image_loader)
                .await
                .context("loading thumbnail")?,
            Command::LoadImage(cmd) => cmd
                .execute(&mut self.image_loader)
                .await
                .context("loading image")?,
            Command::Import(cmd) => cmd.execute(&mut self.import_worker, &handle.bg_tx).await,
            Command::DetectFaces(cmd) => cmd
                .execute(&mut self.cv_worker, &handle.bg_tx)
                .await
                .context("detecting faces")?,
            Command::EmbedFace(cmd) => cmd
                .execute(&mut self.cv_worker)
                .await
                .context("embedding face")?,
        }
        Ok(())
    }
}

pub struct PhotoLibrary {
    db_worker: Arc<Mutex<DbWorker>>,
    scheduler_handle: SchedulerHandle,
}

impl PhotoLibrary {
    pub async fn new(config: Config) -> Result<Self> {
        let thumbnails_path = config.library_path.join(THUMBNAILS_SUBDIRECTORY);
        let originals_path = config.library_path.join(ORIGINALS_SUBDIRECTORY);
        try_ensure_dir(&config.library_path)?;
        try_ensure_dir(&thumbnails_path)?;
        try_ensure_dir(&originals_path)?;
        for thumbnail_size in &config.thumbnail_sizes {
            try_ensure_dir(&thumbnails_path.join(format!("{thumbnail_size}")))?;
        }
        let db_worker = Arc::new(Mutex::new(DbWorker::new(&config.library_path).await?));

        let image_loader = Arc::new(Mutex::new(ImageLoader::new(
            db_worker.clone(),
            thumbnails_path.clone(),
            originals_path.clone(),
        )));

        let cv_worker = CvWorker::new(&config.cv_config, db_worker.clone(), image_loader.clone())?;
        let import_worker = ImportWorker::new(
            db_worker.clone(),
            thumbnails_path.clone(),
            originals_path.clone(),
            config.thumbnail_sizes,
        );
        let scheduler_handle = Scheduler::new(image_loader.clone(), import_worker, cv_worker);

        Ok(Self {
            db_worker,
            scheduler_handle,
        })
    }

    pub async fn get_thumbnail(&mut self, id: u32) -> Result<DynamicImage> {
        tracing::debug!("Loading thumbnail for photo with ID {}", id);
        let (tx, rx) = oneshot::channel();
        self.scheduler_handle
            .ui_tx
            .send(Command::LoadThumbnail(LoadThumbnailCommand { id, tx }))
            .await?;
        tracing::debug!("Task sent for thumbnail {}", id);
        rx.await?
    }

    pub async fn get_full_image(&mut self, id: u32) -> Result<DynamicImage> {
        tracing::debug!("Loading full image for photo with ID {}", id);
        let (tx, rx) = oneshot::channel();
        self.scheduler_handle
            .ui_tx
            .send(Command::LoadImage(LoadImageCommand { id, tx }))
            .await?;
        tracing::debug!("Task sent for full image {}", id);
        rx.await?
    }

    pub async fn import_photo(&mut self, path: PathBuf) -> Result<ImportCommandResult> {
        tracing::debug!("Importing photo from {}", path.display());
        let (tx, rx) = oneshot::channel();
        self.scheduler_handle
            .bg_tx
            .send(Command::Import(ImportCommand { path, tx }))
            .await?;
        tracing::debug!("Task sent for import");
        rx.await?
    }

    pub async fn get_number_of_images(&self) -> Result<usize> {
        self.db_worker
            .lock()
            .await
            .get_number_of_images()
            .await
            .map(|x| x as usize)
    }
}

fn try_ensure_dir(path: &PathBuf) -> Result<()> {
    if !path.is_dir() {
        create_dir(path).context(format!(
            "expected successful creation of {}",
            path.to_str()
                .ok_or(anyhow!("the path is not a valid UTF-8 string"))?
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::workers::cv_worker::CvConfig;

    use super::*;
    use futures::future::join_all;
    use tempdir::TempDir;
    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn get_cv_config() -> CvConfig {
        CvConfig {
            face_detector_model_path: workspace_path().join("models").join("yolov12n-face.onnx"),
            face_embedder_model_path: workspace_path().join("models").join("facenet.onnx"),
            face_detector_image_size: 480,
            face_embedder_image_size: 160,
        }
    }

    #[tokio::test]
    async fn test_init_photo_library() {
        let temp_dir = TempDir::new("photo_library").unwrap();
        let config = Config::new(temp_dir.path().to_path_buf(), get_cv_config())
            .with_thumbnail_sizes(vec![32, 64]);
        let _ = PhotoLibrary::new(config).await.unwrap();
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_import_photo_and_get_image() {
        let temp_dir = TempDir::new("photo_library").unwrap();
        let config = Config::new(temp_dir.path().to_path_buf(), get_cv_config())
            .with_thumbnail_sizes(vec![32]);
        let mut library = PhotoLibrary::new(config).await.unwrap();
        let new_image_path = workspace_path().join("test_data");
        let result = library.import_photo(new_image_path).await;
        assert!(result.is_ok());

        let thumbnail = library.get_thumbnail(1).await.unwrap();
        assert_eq!(thumbnail.height(), 32);
        let full_image = library.get_full_image(1).await.unwrap();
        assert_eq!(full_image.height(), 1280);
    }

    #[tokio::test]
    async fn test_cv_worker() {
        let temp_dir = TempDir::new("photo_library").unwrap();
        let config = Config::new(temp_dir.path().to_path_buf(), get_cv_config())
            .with_thumbnail_sizes(vec![32]);
        let mut library = PhotoLibrary::new(config).await.unwrap();
        let new_image_path = workspace_path().join("test_data").join("example.jpeg");
        let import_cmd_result = library.import_photo(new_image_path).await.unwrap();

        let full_image = library.get_full_image(1).await.unwrap();
        assert_eq!(full_image.height(), 1280);

        let detect_faces_result = join_all(import_cmd_result.rxs)
            .await
            .into_iter()
            .map(|result| result.unwrap());

        // let face_boxes = library.get_faces(full_image).await.unwrap();
        // assert_eq!(face_boxes.len(), 1);
    }
}
