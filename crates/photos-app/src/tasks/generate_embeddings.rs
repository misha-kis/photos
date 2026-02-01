use crate::AppEvent;
use crate::service_registry::AppServiceRegistry;
use photos_domain::{FaceDetection, ImageRecord};
use photos_services::{ImageRepository, ServiceRegistry};
use std::sync::Arc;
use tokio::sync::mpsc;

async fn generate_embeddings_task_inner(
    service_registry: Arc<AppServiceRegistry>,
    image_record: ImageRecord,
    detection: FaceDetection,
) -> Result<(), String> {
    let image = service_registry
        .image_repository
        .get_image(&image_record)
        .map_err(|e| e.to_string())?;
    let detection_with_embedding = service_registry
        .analysis_service()
        .get_face_embedding(&image, detection, service_registry.resize_service())
        .map_err(|e| e.to_string())?;
    service_registry
        .image_meta_repo()
        .update_face_detection_with_embedding(detection_with_embedding)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn generate_embeddings_task(
    service_registry: Arc<AppServiceRegistry>,
    image_record: ImageRecord,
    detection: FaceDetection,
    _tx: mpsc::Sender<AppEvent>,
) {
    if let Err(e) = generate_embeddings_task_inner(service_registry, image_record, detection).await
    {
        tracing::error!("{e}");
    }
}
