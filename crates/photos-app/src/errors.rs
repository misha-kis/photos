#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("bad directory")]
    BadDirectory,
    #[error("invalid database state: {err} ")]
    InvalidDatabaseState{ err: String },
    #[error("something went wrong")]
    Unknown,
}
