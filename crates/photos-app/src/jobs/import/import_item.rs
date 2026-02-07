use crate::errors::AppError;
use crate::jobs::common::{Map, Reduce, TaskContext};
use async_trait::async_trait;
use photos_domain::ImageRecord;
use photos_services::{ImageMetadataRepository, ImageRepository};
use std::path::PathBuf;

pub(crate) struct CopyItemTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Map<PathBuf, ImageRecord> for CopyItemTask {
    async fn map(&self, input: PathBuf) -> Result<ImageRecord, AppError> {
        self.ctx
            .service_registry
            .image_repository
            .insert_image(&input)
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }
}

pub(crate) struct InsertRecordsTask {
    pub(crate) ctx: TaskContext,
}

#[async_trait]
impl Reduce<ImageRecord, ()> for InsertRecordsTask {
    async fn reduce(&self, inputs: Vec<ImageRecord>) -> Result<(), AppError> {
        self.ctx
            .service_registry
            .image_metadata_repository
            .add_image_record_bulk(&inputs)
            .await
            .map_err(|e| AppError::TaskSpawnFailed { err: e.to_string() })
    }
}
