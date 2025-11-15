use crate::workers::db_worker::DbWorker;
use anyhow::{Context, Result};
use image::DynamicImage;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::Mutex;

pub struct ImageLoader {
    db_worker: Arc<Mutex<DbWorker>>,
    thumbnails_path: PathBuf,
    full_images_path: PathBuf,
    image_name_map: HashMap<u32, String>,
    thumbnail_cache: LruCache<u32, DynamicImage>,
    face_thumbnail_cache: LruCache<u32, DynamicImage>,
    full_image_cache: LruCache<u32, DynamicImage>,
}

impl ImageLoader {
    pub async fn new(
        db_worker: Arc<Mutex<DbWorker>>,
        thumbnails_path: PathBuf,
        originals_path: PathBuf,
    ) -> Self {

        let thumbnail_cache = LruCache::new(NonZeroUsize::new(64).unwrap());
        let face_thumbnail_cache = LruCache::new(NonZeroUsize::new(64).unwrap());
        let full_image_cache = LruCache::new(NonZeroUsize::new(16).unwrap());

        let image_name_map = db_worker.lock().await.get_image_names().await.expect("Failed to get image names");

        Self {
            db_worker,
            thumbnails_path,
            full_images_path: originals_path,
            image_name_map,
            thumbnail_cache,
            face_thumbnail_cache,
            full_image_cache,
        }
    }

    pub(crate) async fn get_thumbnail(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self.image_name_map.get(&photo_id).context("Image name not found")?;

        let result = self.thumbnail_cache.get_or_insert(photo_id, || {
            tracing::debug!("getting thumbnail from disk for photo id {}", photo_id);
            let path = self.thumbnails_path.join(format!("{}", 32)).join(name); // todo(other sizes)
            let result = image::open(path).expect("Failed to open image");
            result
        }).clone();

        Ok(result)
    }
    pub(crate) async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self.image_name_map.get(&photo_id).context("Image name not found")?;
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
        let name = self.image_name_map.get(&photo_id).context("Image name not found")?;
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
}


#[derive(Debug)]
pub(crate) struct UpdateImageNameMapCommand {
    pub(crate) new_image_name_map: HashMap<u32, String>,
}

impl UpdateImageNameMapCommand {
    pub(crate) async fn execute(self, image_loader: Arc<Mutex<ImageLoader>>) -> Result<()> {
        let mut image_loader = image_loader.lock().await;
        image_loader.image_name_map.extend(self.new_image_name_map);
        Ok(())
    }
}