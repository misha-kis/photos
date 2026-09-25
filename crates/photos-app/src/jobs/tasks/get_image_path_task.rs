use std::path::PathBuf;

use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Map;
use async_trait::async_trait;
use photos_domain::ImageId;
use photos_services::{ImageMetadataRepository, ImageRepository};

pub(crate) struct GetImagePathTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<ImageId, PathBuf> for GetImagePathTask {
    async fn map(&self, id: ImageId) -> Result<PathBuf, AppError> {
        let record = self
            .ctx
            .service_registry
            .image_metadata_repository
            .get_image_record(id)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        let path = self
            .ctx
            .service_registry
            .image_repository
            .get_original_path(&record);
        Ok(path)
    }
}
