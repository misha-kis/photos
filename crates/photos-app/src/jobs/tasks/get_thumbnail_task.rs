use crate::{
    AppError,
    jobs::{TaskContext, common::Map},
};
use async_trait::async_trait;
use photos_domain::{ImageId, RgbaImage};
use photos_services::ImageRepository;

pub(crate) struct GetThumbnailTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(ImageId, u32), RgbaImage> for GetThumbnailTask {
    async fn map(&self, (id, size): (ImageId, u32)) -> Result<RgbaImage, AppError> {
        let img = self
            .ctx
            .service_registry
            .image_repository
            .get_thumbnail(&id, size)
            .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })?;
        Ok(tokio::task::spawn_blocking(move || img.to_rgba8())
            .await
            .expect("blocking panicked"))
    }
}
