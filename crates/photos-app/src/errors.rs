#[derive(thiserror::Error, Debug, Clone)]
pub enum AppError {
    #[error("bad directory")]
    BadDirectory,
    #[error("invalid database state: {err}")]
    InvalidDatabaseState { err: String },
    #[error("task spawn failed: {err}")]
    TaskSpawnFailed { err: String },
    #[error("image repository error: {err}")]
    ImageRepositoryError { err: String },
}
