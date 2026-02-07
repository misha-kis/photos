use crate::errors::AppError;
use crate::jobs::TaskContext;
use crate::jobs::common::Expand;
use async_trait::async_trait;
use photos_domain::{FaceDetection, ImageRecord};
use photos_services::ServiceRegistry;

pub(crate) struct DiscoverImagesWithoutEmbeddings {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Expand<(), (ImageRecord, FaceDetection)> for DiscoverImagesWithoutEmbeddings {
    async fn expand(&self, _input: ()) -> Result<Vec<(ImageRecord, FaceDetection)>, AppError> {
        self.ctx
            .service_registry
            .image_meta_repo()
            .get_detections_without_embeddings()
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }
}
