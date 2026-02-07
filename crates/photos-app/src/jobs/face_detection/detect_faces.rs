use crate::errors::AppError;
use crate::jobs::common::{Map, TaskContext};
use async_trait::async_trait;
use photos_domain::ImageRecord;
use photos_services::{
    ImageAnalysisService, ImageMetadataRepository, ImageRepository, ServiceRegistry,
};

pub(crate) struct DetectFacesTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<ImageRecord, ()> for DetectFacesTask {
    async fn map(&self, input: ImageRecord) -> Result<(), AppError> {
        let image = self
            .ctx
            .service_registry
            .image_repository
            .get_image(&input)
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        let detections = self
            .ctx
            .service_registry
            .analysis_service
            .get_face_detections(&image, self.ctx.service_registry.resize_service())
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        self.ctx
            .service_registry
            .image_metadata_repository
            .add_detections_to_image(&input.id, detections)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        Ok(())
    }
}
