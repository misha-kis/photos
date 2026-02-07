use crate::errors::AppError;
use crate::service_registry::AppServiceRegistry;
use crate::tasks::common::{Expand, Job, Map, Task, TaskContext};
use photos_domain::{FaceDetection, ImageRecord};
use photos_services::{ImageAnalysisService, ImageMetadataRepository, ImageRepository, ServiceRegistry};
use std::sync::Arc;
use async_trait::async_trait;

pub(crate) async fn detect_faces_task(
    service_registry: Arc<AppServiceRegistry>,
    image_record: ImageRecord,
) {
    tracing::debug!("detecting faces for image: {}", image_record.id);
    if let Ok(image) = service_registry.image_repository.get_image(&image_record)
        && let Ok(face_detections) = service_registry
            .analysis_service()
            .get_face_detections(&image, service_registry.resize_service())
    {
        let _ = service_registry
            .image_metadata_repository
            .add_detections_to_image(&image_record.id, face_detections)
            .await;
    }
}


struct DetectFacesTask {
    ctx: TaskContext,
}

#[async_trait]
impl Map<ImageRecord, (ImageRecord, Vec<FaceDetection>)> for DetectFacesTask {
    async fn map(&self, input: ImageRecord) -> Result<(ImageRecord, Vec<FaceDetection>), AppError> {
        let image = self.ctx
            .service_registry
            .image_repository
            .get_image(&input)
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        let detections = self.ctx
            .service_registry
            .analysis_service
            .get_face_detections(&image, self.ctx.service_registry.resize_service())
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        self.ctx.service_registry
            .image_metadata_repository
            .add_detections_to_image(&input.id, detections.clone())
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        Ok((input, detections))
    }
}
