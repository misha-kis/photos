use crate::errors::AppError;
use photos_core::JobId;
use photos_domain::{DynamicImage, ImageId};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum AppEvent {
    ImageIdsReady {
        result: Result<Vec<ImageId>, AppError>,
    },
    ThumbnailReady {
        image_id: ImageId,
        result: Result<DynamicImage, AppError>,
    },
    ThumbnailFromFileReady {
        path: PathBuf,
        result: Result<DynamicImage, AppError>,
    },
    ImportItemsDiscovered {
        path: PathBuf,
        result: Result<Vec<PathBuf>, AppError>,
    },
    ImportProgress {
        job_id: JobId,
        current: u64,
        total: u64,
    },
    ImportFinished {
        job_id: JobId,
        success: bool,
    },
}

