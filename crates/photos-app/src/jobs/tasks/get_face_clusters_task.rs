use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Map;
use async_trait::async_trait;
use photos_domain::Uuid;
use photos_services::ImageMetadataRepository;

pub(crate) struct GetFaceClustersTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(), Vec<(Uuid, Vec<Uuid>)>> for GetFaceClustersTask {
    async fn map(&self, (): ()) -> Result<Vec<(Uuid, Vec<Uuid>)>, AppError> {
        self.ctx
            .service_registry
            .image_metadata_repository
            .get_face_clusters()
            .await
            .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() })
    }
}
