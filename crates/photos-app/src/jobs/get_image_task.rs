use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Map;
use async_trait::async_trait;
use photos_domain::{ImageId, RgbaImage};
use photos_services::{ImageMetadataRepository, ImageRepository};

pub(crate) struct GetImageTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(ImageId, (u32, u32)), RgbaImage> for GetImageTask {
    async fn map(&self, (id, size): (ImageId, (u32, u32))) -> Result<RgbaImage, AppError> {
        let record = self
            .ctx
            .service_registry
            .image_metadata_repository
            .get_image_record(id)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        let img = self
            .ctx
            .service_registry
            .image_repository
            .get_image(&record, Some(size))
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        Ok(tokio::task::spawn_blocking(move || img.to_rgba8())
            .await
            .expect("blocking panicked"))
    }
}
