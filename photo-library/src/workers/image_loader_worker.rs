use crate::workers::db_worker::{DbWorkerCmd, get_photo_name_by_id};
use anyhow::Result;
use image::DynamicImage;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub(crate) enum ImageLoadCmd {
    LoadThumbnail(u32),
    LoadFullImage(u32),
}

struct ImageLoader {
    db_worker_tx: mpsc::Sender<DbWorkerCmd>,
    thumbnails_path: PathBuf,
    full_images_path: PathBuf,
    image_name_cache: LruCache<u32, String>,
    thumbnail_cache: LruCache<u32, DynamicImage>,
    full_image_cache: LruCache<u32, DynamicImage>,
}

impl ImageLoader {
    fn new(
        db_worker_tx: mpsc::Sender<DbWorkerCmd>,
        thumbnails_path: PathBuf,
        originals_path: PathBuf,
    ) -> Self {
        Self {
            db_worker_tx,
            thumbnails_path,
            full_images_path: originals_path,
            image_name_cache: LruCache::new(NonZeroUsize::new(128).unwrap()),
            thumbnail_cache: LruCache::new(NonZeroUsize::new(64).unwrap()),
            full_image_cache: LruCache::new(NonZeroUsize::new(5).unwrap()),
        }
    }

    async fn get_thumbnail(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;

        if let Some(result) = self.thumbnail_cache.get(&photo_id) {
            Ok(result.clone())
        } else {
            let path = self.thumbnails_path.join(format!("{}", 32)).join(name); // todo(other sizes)
            let result = image::open(path)?;
            self.thumbnail_cache.put(photo_id, result.clone());
            Ok(result)
        }
    }
    async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;

        if let Some(result) = self.full_image_cache.get(&photo_id) {
            Ok(result.clone())
        } else {
            let path = self.full_images_path.join(name);
            let result = image::open(path)?;
            self.full_image_cache.put(photo_id, result.clone());
            Ok(result)
        }
    }

    async fn _get_name(&mut self, photo_id: u32) -> Result<String> {
        if let Some(name) = self.image_name_cache.get(&photo_id) {
            Ok(name.clone())
        } else {
            let name = self._get_name_from_db(photo_id).await?;
            self.image_name_cache.put(photo_id, name.clone());
            Ok(name)
        }
    }

    async fn _get_name_from_db(&mut self, photo_id: u32) -> Result<String> {
        let name = get_photo_name_by_id(&self.db_worker_tx, photo_id).await?;
        self.image_name_cache.put(photo_id, name.clone());
        Ok(name)
    }
}

pub(crate) fn spawn_image_loader(
    db_worker_tx: mpsc::Sender<DbWorkerCmd>,
    thumbnails_path: PathBuf,
    originals_path: PathBuf,
) -> (
    JoinHandle<()>,
    mpsc::Sender<ImageLoadCmd>,
    mpsc::Receiver<Result<DynamicImage>>,
) {
    let (thumbnail_cmd_tx, mut thumbnail_cmd_rx) = mpsc::channel(32);
    let (thumbnail_res_tx, thumbnail_res_rx) = mpsc::channel(32);
    let mut image_loader = ImageLoader::new(db_worker_tx, thumbnails_path, originals_path);

    let worker = tokio::spawn(async move {
        while let Some(cmd) = thumbnail_cmd_rx.recv().await {
            let res = match cmd {
                ImageLoadCmd::LoadThumbnail(id) => image_loader.get_thumbnail(id).await,
                ImageLoadCmd::LoadFullImage(id) => image_loader.get_full_image(id).await,
            };
            thumbnail_res_tx.send(res).await.expect("Can send result");
        }
    });

    (worker, thumbnail_cmd_tx, thumbnail_res_rx)
}
