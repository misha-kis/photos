use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Map;
use async_trait::async_trait;
use photos_domain::ImageId;
use photos_services::ImageMetadataRepository;

pub(crate) struct GetImageIdsTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<(), Vec<ImageId>> for GetImageIdsTask {
    async fn map(&self, _: ()) -> Result<Vec<ImageId>, AppError> {
        self.ctx
            .service_registry
            .image_metadata_repository
            .get_image_ids()
            .await
            .map_err(|e| AppError::InvalidDatabaseState { err: e.to_string() })
    }
}
