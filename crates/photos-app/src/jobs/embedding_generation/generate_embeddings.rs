use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Map;
use async_trait::async_trait;
use photos_domain::{FaceDetection, ImageRecord};
use photos_services::{ImageAnalysisService, ImageMetadataRepository, ImageRepository};

pub(crate) struct GenerateEmbeddings {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(ImageRecord, FaceDetection), ()> for GenerateEmbeddings {
    async fn map(&self, (image, detection): (ImageRecord, FaceDetection)) -> Result<(), AppError> {
        let image = self
            .ctx
            .service_registry
            .image_repository
            .get_image(&image)
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        let detection_with_embedding = self
            .ctx
            .service_registry
            .analysis_service
            .get_face_embedding(
                &image,
                detection,
                self.ctx.service_registry.resize_service.as_ref(),
            )
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        self.ctx
            .service_registry
            .image_metadata_repository
            .update_face_detection_with_embedding(detection_with_embedding)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        Ok(())
    }
}
