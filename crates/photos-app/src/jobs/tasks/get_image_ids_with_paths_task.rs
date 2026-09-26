use std::path::PathBuf;

use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Map;
use async_trait::async_trait;
use photos_domain::ImageId;
use photos_services::{ImageMetadataRepository, ImageRepository};

pub(crate) struct GetImageIdsWithPathsTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(), Vec<(ImageId, PathBuf, PathBuf)>> for GetImageIdsWithPathsTask {
    async fn map(&self, _: ()) -> Result<Vec<(ImageId, PathBuf, PathBuf)>, AppError> {
        let records = self
            .ctx
            .service_registry
            .image_metadata_repository
            .get_image_records()
            .await
            .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() })?;
        records
            .into_iter()
            .map(|r| {
                let repo = &self.ctx.service_registry.image_repository;
                let thumbnail_path = repo
                    .get_thumbnail_path(&r.id, 128)
                    .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })?;
                let original_path = repo.get_original_path(&r);
                Ok((r.id, thumbnail_path, original_path))
            })
            .collect()
    }
}
