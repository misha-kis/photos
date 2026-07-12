#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("bad directory: {err}")]
    BadDirectory { err: String },
    #[error("invalid database state: {err}")]
    InvalidDatabaseState { err: String },
    #[error("task spawn failed: {err}")]
    TaskSpawnFailed { err: String },
    #[error("image repository error: {err}")]
    ImageRepositoryError { err: String },
    #[error("AppError {0}")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}
