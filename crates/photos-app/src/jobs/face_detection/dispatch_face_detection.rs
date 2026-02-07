use crate::errors::AppError;
use crate::jobs::common::{Expand, TaskContext};
use async_trait::async_trait;
use photos_domain::ImageRecord;
use photos_services::ImageMetadataRepository;

pub(crate) struct DiscoverImagesToDetect {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Expand<(), ImageRecord> for DiscoverImagesToDetect {
    async fn expand(&self, _input: ()) -> Result<Vec<ImageRecord>, AppError> {
        self.ctx
            .service_registry
            .image_metadata_repository
            .get_image_records_without_detections()
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }
}
