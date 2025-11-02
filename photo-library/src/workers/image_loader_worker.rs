use crate::workers::db_worker::DbWorker;
use anyhow::Result;
use image::DynamicImage;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use tokio::sync::{Mutex, oneshot};

pub(crate) enum ImageLoadCmd {
    LoadThumbnail(u32),
    LoadFullImage(u32),
}

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
            let result = image::open(path)?;
            self.thumbnail_cache.put(photo_id, result.clone());
            Ok(result)
        }
    }
    pub(crate) async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
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

    pub(crate) async fn get_image_no_cache(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self._get_name(photo_id).await?;
        let path = self.full_images_path.join(name);
        tracing::debug!("getting image without cache at path {}", &path.display());
        let result = image::open(path)?;
        Ok(result)
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
        let name = self
            .db_worker
            .lock()
            .await
            .get_photo_name_by_photo_id(photo_id)
            .await?;
        self.image_name_cache.put(photo_id, name.clone());
        Ok(name)
    }
}

pub struct LoadImageCommand {
    pub id: u32,
    pub tx: oneshot::Sender<Result<DynamicImage>>,
}

impl LoadImageCommand {
    pub async fn execute(self, loader: &mut Arc<Mutex<ImageLoader>>) -> Result<()> {
        tracing::debug!("Loading full image for photo ID: {}", self.id);
        let image = loader.lock().await.get_full_image(self.id).await?;
        tracing::debug!("Full image loaded for photo ID: {}", self.id);
        self.tx.send(Ok(image)).unwrap();
        tracing::debug!("Full image sent for photo ID: {}", self.id);
        Ok(())
    }
}

impl std::fmt::Debug for LoadImageCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadImageCommand")
            .field("id", &self.id)
            .finish()
    }
}

pub struct LoadThumbnailCommand {
    pub id: u32,
    pub tx: oneshot::Sender<Result<DynamicImage>>,
}

impl LoadThumbnailCommand {
    pub async fn execute(self, loader: &mut Arc<Mutex<ImageLoader>>) -> Result<()> {
        tracing::debug!("Loading thumbnail for photo ID: {}", self.id);
        let image = loader.lock().await.get_thumbnail(self.id).await?;
        tracing::debug!("Thumbnail loaded for photo ID: {}", self.id);
        self.tx.send(Ok(image)).unwrap();
        tracing::debug!("Thumbnail sent for photo ID: {}", self.id);
        Ok(())
    }
}

impl std::fmt::Debug for LoadThumbnailCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadThumbnailCommand")
            .field("id", &self.id)
            .finish()
    }
}

#[derive(Debug)]
pub struct LoadImageCommandResult {}

#[derive(Debug)]
pub struct LoadThumbnailCommandResult {}
