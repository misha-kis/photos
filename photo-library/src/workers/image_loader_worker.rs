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
    face_thumbnail_cache: LruCache<u32, DynamicImage>,
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
            face_thumbnail_cache: LruCache::new(NonZeroUsize::new(64).unwrap()),
            full_image_cache: LruCache::new(NonZeroUsize::new(16).unwrap()),
        }
    }

    pub(crate) async fn get_thumbnail(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;

        let result = self.thumbnail_cache.get_or_insert(photo_id, || {
            tracing::debug!("getting thumbnail from disk for photo id {}", photo_id);
            let path = self.thumbnails_path.join(format!("{}", 32)).join(name); // todo(other sizes)
            let result = image::open(path).expect("Failed to open image");
            result
        }).clone();

        Ok(result)
    }
    pub(crate) async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;
        tracing::debug!("cache size: {}", self.full_image_cache.len());

        let result = self.full_image_cache.get_or_insert(photo_id, || {
            tracing::debug!("getting image from disk for photo id {}", photo_id);
            let path = self.full_images_path.join(name);
            let result = image::open(path).expect("Failed to open image");
            result
        }).clone();

        Ok(result)
    }

    pub(crate) async fn get_image_no_cache(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;
        let path = self.full_images_path.join(name);
        tracing::debug!("getting image without cache at path {}", &path.display());
        let result = image::open(path)?;
        Ok(result)
    }

    pub(crate) async fn get_face_thumbnail(&mut self, face_detection_id: u32) -> Result<DynamicImage> {
        if let Some(thumbnail) = self.face_thumbnail_cache.get(&face_detection_id) {
            Ok(thumbnail.clone())
        } else {
            tracing::debug!("getting face thumbnail from disk for face detection id {}", face_detection_id);
            let face_detection = self.db_worker.lock().await.get_face_detection(face_detection_id).await?;
            let mut full_image = self.get_full_image(face_detection.0).await?;
            let bounding_box = face_detection.1;
            let x = bounding_box.x1 as u32;
            let y = bounding_box.y1 as u32;
            let w = bounding_box.x2 as u32 - x;
            let h = bounding_box.y2 as u32 - y;
            let thumbnail = full_image.crop(x, y, w, h);
            self.face_thumbnail_cache.put(face_detection_id, thumbnail.clone());
            Ok(thumbnail)
        }
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
