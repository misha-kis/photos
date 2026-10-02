use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Reduce;
use async_trait::async_trait;
use photos_services::{ImageAnalysisService, ImageMetadataRepository, ImageRepository};

pub(crate) struct ClusterEmbeddings {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Reduce<(), ()> for ClusterEmbeddings {
    async fn reduce(&self, _inputs: Vec<()>) -> Result<(), AppError> {
        let detections_with_embeddings = self
            .ctx
            .service_registry
            .image_metadata_repository
            .get_detections_with_embeddings()
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        let clustered_face_detections = self
            .ctx
            .service_registry
            .analysis_service
            .cluster_embeddings(detections_with_embeddings)
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;
        self.ctx
            .service_registry
            .image_metadata_repository
            .update_detections_with_clusters(&clustered_face_detections)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;

        let clusters = self
            .ctx
            .service_registry
            .image_metadata_repository
            .get_face_clusters()
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })?;

        for (cluster_id, detection_ids) in clusters {
            let representative =
                detection_ids
                    .first()
                    .ok_or_else(|| AppError::InvalidDatabaseState {
                        err: "face cluster has no detections".into(),
                    })?;
            let (bounding_box, image_record) = self
                .ctx
                .service_registry
                .image_metadata_repository
                .get_bbox_and_image_for_detection_id(*representative)
                .await
                .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() })?;
            self.ctx
                .service_registry
                .image_repository
                .save_face_thumbnail(cluster_id, &image_record, bounding_box, 128)
                .map_err(|e| AppError::ImageRepositoryError { err: e.to_string() })?;
        }
        Ok(())
    }
}
