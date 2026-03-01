use async_trait::async_trait;
use photos_domain::RgbaImage;
use photos_services::ImageRepository;
use std::path::PathBuf;

use crate::{
    AppError,
    jobs::{TaskContext, common::Map},
};

pub(crate) struct GetThumbnailFromFileTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(PathBuf, u32), RgbaImage> for GetThumbnailFromFileTask {
    async fn map(&self, (path, size): (PathBuf, u32)) -> Result<RgbaImage, AppError> {
        let img = self
            .ctx
            .service_registry
            .image_repository
            .get_thumbnail_from_file(&path, size)
            .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })?;
        Ok(tokio::task::spawn_blocking(move || img.to_rgba8())
            .await
            .expect("blocking panicked"))
    }
}
