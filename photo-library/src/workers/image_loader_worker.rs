use crate::workers::db_worker::DbWorker;
use anyhow::{Context, Result};
use image::DynamicImage;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::Mutex;

pub struct ImageLoader {
    db_worker: Arc<Mutex<DbWorker>>,
    thumbnails_path: PathBuf,
    full_images_path: PathBuf,
    image_name_cache: LruCache<u32, String>,
    thumbnail_cache: LruCache<u32, DynamicImage>,
    full_image_cache: LruCache<u32, DynamicImage>,
}

impl ImageLoader {
    pub fn new(
        db_worker: Arc<Mutex<DbWorker>>,
        thumbnails_path: PathBuf,
        originals_path: PathBuf,
    ) -> Self {
        Self {
            db_worker,
            thumbnails_path,
            full_images_path: originals_path,
            image_name_cache: LruCache::new(NonZeroUsize::new(128).unwrap()),
            thumbnail_cache: LruCache::new(NonZeroUsize::new(64).unwrap()),
            full_image_cache: LruCache::new(NonZeroUsize::new(5).unwrap()),
        }
    }

    pub(crate) async fn get_thumbnail(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;

        if let Some(result) = self.thumbnail_cache.get(&photo_id) {
            Ok(result.clone())
        } else {
            let path = self.thumbnails_path.join(format!("{}", 32)).join(name); // todo(other sizes)
            let result =
                image::open(&path).context(format!("Failed to open image {}", path.display()))?;
            self.thumbnail_cache.put(photo_id, result.clone());
            Ok(result)
        }
    }
    pub(crate) async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;

        if let Some(result) = self.full_image_cache.get(&photo_id) {
            tracing::debug!("getting image from cache for photo id {}", photo_id);
            Ok(result.clone())
        } else {
            tracing::debug!("getting image from disk for photo id {}", photo_id);
            let path = self.full_images_path.join(name);
            let result = image::open(path)?;
            self.full_image_cache.put(photo_id, result.clone());
            Ok(result)
        }
    }

    pub(crate) async fn get_image_no_cache(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;
        let path = self.full_images_path.join(name);
        tracing::debug!("getting image without cache at path {}", &path.display());
        let result = image::open(path)?;
        Ok(result)
    }

    async fn _get_name(&mut self, photo_id: u32) -> Result<String> {
        if let Some(name) = self.image_name_cache.get(&photo_id) {
            tracing::debug!("getting image name from cache for photo id {}", photo_id);
            Ok(name.clone())
        } else {
            tracing::debug!("getting image name from database for photo id {}", photo_id);
            let name = self._get_name_from_db(photo_id).await?;
            self.image_name_cache.put(photo_id, name.clone());
            Ok(name)
        }
    }

    async fn _get_name_from_db(&mut self, photo_id: u32) -> Result<String> {
        let name = self
            .db_worker
            .lock()
            .await
            .get_photo_name_by_photo_id(photo_id)
            .await
            .context("Failed to get photo name")?;
        self.image_name_cache.put(photo_id, name.clone());
        Ok(name)
    }
}
