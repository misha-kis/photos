use crate::service_registry::AppServiceRegistry;
use photos_domain::ImageRecord;
use photos_services::{ImageMetadataRepository, ImageRepository, ServiceRegistry};
use std::sync::Arc;

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
