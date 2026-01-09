use crate::AppEvent;
use crate::service_registry::AppServiceRegistry;
use photos_services::ServiceRegistry;
use std::sync::Arc;
use tokio::sync::mpsc;

pub(crate) async fn cluster_embeddings_task(
    app_service_registry: Arc<AppServiceRegistry>,
    _tx: mpsc::Sender<AppEvent>,
) {
    tracing::debug!("creating clusters for faces");
    if let Ok(detections_with_embeddings) = app_service_registry
        .image_meta_repo()
        .get_detections_with_embeddings()
        .await
        && let Ok(clustered_face_detections) = app_service_registry
            .analysis_service()
            .cluster_embeddings(detections_with_embeddings)
        && let Ok(()) = app_service_registry
            .image_meta_repo()
            .update_detections_with_clusters(&clustered_face_detections)
            .await
    {
        tracing::debug!("creating clusters for faces done");
    }
}
