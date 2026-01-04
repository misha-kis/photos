use crate::errors::AppError;
use photos_domain::{DynamicImage, ImageId};
use photos_workflow::WorkflowEvent;
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
    WorkflowEvent {
        event: WorkflowEvent,
    },
}

