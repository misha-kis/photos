use crate::AppEvent;
use crate::service_registry::AppServiceRegistry;
use photos_domain::{FaceDetection, ImageRecord};
use photos_services::{ImageRepository, ServiceRegistry};
use std::sync::Arc;
use tokio::sync::mpsc;

pub(crate) async fn generate_embeddings_task(
    service_registry: Arc<AppServiceRegistry>,
    image_record: ImageRecord,
    detection: FaceDetection,
    _tx: mpsc::Sender<AppEvent>,
) {
    if let Ok(image) = service_registry.image_repository.get_image(&image_record)
        && let Ok(detection_with_embedding) = service_registry
            .analysis_service()
            .get_face_embedding(&image, detection, service_registry.resize_service())
        && let Err(e) = service_registry
            .image_meta_repo()
            .update_face_detection_with_embedding(detection_with_embedding)
            .await
    {
        tracing::error!("could not insert embeddings: {e}");
    }
}
