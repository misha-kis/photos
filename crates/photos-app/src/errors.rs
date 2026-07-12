use photos_services::{
    ImageAnalysisServiceError, ImageMetadataRepositoryError, ImageRepositoryError,
};
use photos_task_queue::TaskQueueError;
use tokio::sync::oneshot;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("bad directory: {source}")]
    BadDirectory {
        #[source]
        source: std::io::Error,
    },
    #[error("task queue error: {message}")]
    TaskQueue { message: String },
    #[error("task join failed: {source}")]
    TaskJoin {
        #[source]
        source: tokio::task::JoinError,
    },
    #[error("image repository error: {source}")]
    ImageRepository {
        #[from]
        source: ImageRepositoryError,
    },
    #[error("image metadata repository error: {source}")]
    ImageMetadataRepository {
        #[from]
        source: ImageMetadataRepositoryError,
    },
    #[error("image analysis service error: {source}")]
    ImageAnalysisService {
        #[from]
        source: ImageAnalysisServiceError,
    },
    #[error("internal error: {source}")]
    Internal {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl From<std::io::Error> for AppError {
    fn from(source: std::io::Error) -> Self {
        Self::BadDirectory { source }
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::TaskQueue { message }
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(source: tokio::task::JoinError) -> Self {
        Self::TaskJoin { source }
    }
}

impl From<TaskQueueError> for AppError {
    fn from(source: TaskQueueError) -> Self {
        Self::TaskQueue {
            message: source.message,
        }
    }
}

impl From<oneshot::error::RecvError> for AppError {
    fn from(source: oneshot::error::RecvError) -> Self {
        Self::Internal {
            source: Box::new(source),
        }
    }
}
