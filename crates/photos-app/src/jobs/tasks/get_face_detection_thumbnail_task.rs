use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Map;
use async_trait::async_trait;
use photos_domain::{ImageId, RgbaImage};
use photos_services::{ImageMetadataRepository, ImageRepository};

pub(crate) struct GetFaceDetectionThumbnailTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(ImageId, u32), RgbaImage> for GetFaceDetectionThumbnailTask {
    async fn map(&self, (id, size): (ImageId, u32)) -> Result<RgbaImage, AppError> {
        let (bounding_box, image_record) = self
            .ctx
            .service_registry
            .image_metadata_repository
            .get_bbox_and_image_for_detection_id(id)
            .await
            .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() })?;
        let img = self
            .ctx
            .service_registry
            .image_repository
            .get_face_thumbnail(&image_record, bounding_box, size)
            .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })?;
        Ok(tokio::task::spawn_blocking(move || img.to_rgba8())
            .await
            .expect("blocking panicked"))
    }
}
