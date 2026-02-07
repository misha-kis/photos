use crate::AppEvent;
use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Reduce;
use crate::service_registry::AppServiceRegistry;
use photos_services::ServiceRegistry;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;

async fn do_cluster_embeddings_task(
    app_service_registry: Arc<AppServiceRegistry>,
) -> Result<(), String> {
    let detections_with_embeddings = app_service_registry
        .image_meta_repo()
        .get_detections_with_embeddings()
        .await
        .map_err(|e| e.to_string())?;
    let clustered_face_detections = app_service_registry
        .analysis_service()
        .cluster_embeddings(detections_with_embeddings)
        .map_err(|e| e.to_string())?;
    app_service_registry
        .image_meta_repo()
        .update_detections_with_clusters(&clustered_face_detections)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn cluster_embeddings_task(
    app_service_registry: Arc<AppServiceRegistry>,
    _tx: mpsc::Sender<AppEvent>,
) {
    tracing::debug!("creating clusters for faces");
    match do_cluster_embeddings_task(app_service_registry).await {
        Ok(()) => tracing::info!("done clustering"),
        Err(e) => tracing::info!("error while clustering: {}", e),
    }
}

pub(crate) struct ClusterEmbeddings {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Reduce<(), ()> for ClusterEmbeddings {
    async fn reduce(&self, _inputs: Vec<()>) -> Result<(), AppError> {
        let detections_with_embeddings = self
            .ctx
            .service_registry
            .image_meta_repo()
            .get_detections_with_embeddings()
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        let clustered_face_detections = self
            .ctx
            .service_registry
            .analysis_service()
            .cluster_embeddings(detections_with_embeddings)
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        self.ctx
            .service_registry
            .image_meta_repo()
            .update_detections_with_clusters(&clustered_face_detections)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        Ok(())
    }
}
