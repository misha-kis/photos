#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("bad directory")]
    BadDirectory,
    #[error("invalid database state")]
    InvalidDatabaseState,
    #[error("something went wrong")]
    Unknown,
}
