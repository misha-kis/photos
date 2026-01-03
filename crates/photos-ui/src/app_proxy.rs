use anyhow::Context;
use dashmap::DashMap;
use image::DynamicImage;
use parking_lot::RwLock;
use photos_domain::ImageId;
use photos_workflow::WorkflowEvent;
use photos_workflow::errors::JobError;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;

type SharedImage = Arc<RwLock<Option<anyhow::Result<DynamicImage>>>>;

pub struct AppProxy {
    runtime: tokio::runtime::Runtime,
    app: Arc<Mutex<photos_app::App>>,
    thumbnail_load_requests: DashMap<ImageId, SharedImage>,
    thumbnail_size: u32,
    pub image_ids: Vec<ImageId>,
    render_import_thumbnail_requests: DashMap<PathBuf, SharedImage>,
    import_item_discovery_request: Option<Arc<RwLock<Option<anyhow::Result<Vec<PathBuf>>>>>>,
    import_job: Option<(Receiver<WorkflowEvent>, JoinHandle<Result<(), JobError>>)>,
}

async fn example_import(app: &mut photos_app::App) {
    let to_import = dirs::picture_dir()
        .unwrap()
        .join("picslib3")
        .join("originals");
    let items = app.discover_import_items(to_import).await.unwrap();
    let (mut rx, handle) = app.import_items(items);
    while let Some(evt) = rx.recv().await {
        println!("{:?}", evt);
    }
    handle.await.unwrap().unwrap();
}

impl AppProxy {
    pub fn new(gallery_dir: PathBuf, config: photos_app::config::Config) -> anyhow::Result<Self> {
        let thumbnail_size = config.thumbnail_sizes[0];
        let runtime = tokio::runtime::Runtime::new()?;
        let app = runtime.block_on(async {
            photos_app::App::new(gallery_dir, config).await
        })?;
        // runtime.block_on(example_import(&mut app));
        let image_ids = runtime.block_on(async {
            app.get_image_ids().await
        })?;
        Ok(Self {
            runtime,
            app: Arc::new(Mutex::new(app)),
            thumbnail_load_requests: DashMap::new(),
            thumbnail_size,
            image_ids,
            render_import_thumbnail_requests: DashMap::new(),
            import_item_discovery_request: None,
            import_job: None,
        })
    }

    pub fn try_get_thumbnail(&self, id: ImageId) -> anyhow::Result<Option<DynamicImage>> {
        if let Some((_, shared)) = self
            .thumbnail_load_requests
            .remove_if(&id, |_, shared| shared.read().is_some())
        {
            if let Some(result) = shared.write().take() {
                return result.map(Some);
            }
            return Ok(None);
        }

        if self.thumbnail_load_requests.contains_key(&id) {
            return Ok(None);
        }

        let shared = Arc::new(RwLock::new(None));
        self.thumbnail_load_requests.insert(id, shared.clone());

        let thumbnail_size = self.thumbnail_size;
        let app = self.app.clone();

        self.runtime.handle().spawn(async move {
            let result = async {
                let app = app.lock().await;
                app.get_thumbnail(&id, thumbnail_size).await
            }
            .await
            .context("getting thumbnail");

            shared.write().replace(result);
        });

        Ok(None)
    }

    pub fn number_of_images(&self) -> usize {
        self.image_ids.len()
    }

    pub fn try_render_import_thumbnail(
        &self,
        path: &PathBuf,
    ) -> anyhow::Result<Option<DynamicImage>> {
        if let Some((_, shared)) = self
            .render_import_thumbnail_requests
            .remove_if(path, |_, shared| shared.read().is_some())
        {
            if let Some(result) = shared.write().take() {
                return result.map(Some);
            }
            return Ok(None);
        }

        if self.render_import_thumbnail_requests.contains_key(path) {
            return Ok(None);
        }

        let shared = Arc::new(RwLock::new(None));
        self.render_import_thumbnail_requests
            .insert(path.clone(), shared.clone());

        let thumbnail_size = self.thumbnail_size;
        let app = self.app.clone();
        let path = path.clone();

        self.runtime.handle().spawn(async move {
            let result = async {
                let app = app.lock().await;
                app.get_thumbnail_from_file(&path, thumbnail_size).await
            }
            .await
            .context("getting thumbnail");

            shared.write().replace(result);
        });

        Ok(None)
    }

    pub fn try_discover_import_items(
        &mut self,
        path: &Path,
    ) -> anyhow::Result<Option<Vec<PathBuf>>> {
        if let Some(shared) = self
            .import_item_discovery_request
            .take_if(|shared| shared.read().is_some())
        {
            if let Some(shared) = shared.write().take() {
                shared.map(Some)
            } else {
                Ok(None)
            }
        } else {
            let shared = Arc::new(RwLock::new(None));
            self.import_item_discovery_request = Some(shared.clone());
            let path = path.to_path_buf();
            let app = self.app.clone();
            self.runtime.handle().spawn(async move {
                let result = app
                    .lock()
                    .await
                    .discover_import_items(path)
                    .await
                    .context("discovering import items");
                shared.write().replace(result);
            });
            Ok(None)
        }
    }

    pub fn start_import(&mut self, paths: Vec<PathBuf>) {
        let app = self.app.clone();
        let (evt_rx, handle) = self.runtime.block_on(async move {
            app.lock().await.import_items(paths)
        });
        self.import_job = Some((evt_rx, handle));
    }

    pub fn check_import_progress(&mut self) -> ImportProgress {
        let mut ret_val = ImportProgress::None;
        self.import_job = if let Some((mut evt_rx, handle)) = self.import_job.take() {
            while let Ok(evt) = evt_rx.try_recv() {
                ret_val = match evt {
                    WorkflowEvent::StepProgress { current, total, .. } => {
                        ImportProgress::Progress(current, total)
                    }
                    WorkflowEvent::JobFinished { .. } => ImportProgress::Done,
                    _ => ret_val,
                };
            }
            Some((evt_rx, handle))
        } else {
            None
        };
        ret_val
    }

    pub fn refresh_images(&mut self) {
        let app = self.app.clone();
        self.image_ids = self.runtime.block_on(async move {
            app.lock().await.get_image_ids().await.unwrap()
        })
    }
}

pub enum ImportProgress {
    None,
    Progress(u64, u64),
    Done,
}
