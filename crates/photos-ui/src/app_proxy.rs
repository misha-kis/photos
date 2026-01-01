use anyhow::Context;
use dashmap::DashMap;
use image::DynamicImage;
use parking_lot::RwLock;
use photos_domain::ImageId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

type SharedImage = Arc<RwLock<Option<anyhow::Result<DynamicImage>>>>;

pub struct AppProxy {
    runtime: tokio::runtime::Runtime,
    app: Arc<Mutex<photos_app::App>>,
    thumbnail_load_requests: DashMap<ImageId, SharedImage>,
    thumbnail_size: u32,
    pub image_ids: Vec<ImageId>,
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
        let app = runtime.block_on(photos_app::App::new(gallery_dir, config))?;
        // runtime.block_on(example_import(&mut app));
        let image_ids = runtime.block_on(app.get_image_ids())?;
        let app = Arc::new(Mutex::new(app));
        let thumbnail_load_requests = DashMap::new();
        Ok(Self {
            runtime,
            app,
            thumbnail_load_requests,
            thumbnail_size,
            image_ids,
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
}
