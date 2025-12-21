#[derive(thiserror::Error, Debug)]
pub enum DomainError {
    #[error("unsupported image format")]
    UnsupportedFormat,

    #[error("image dimensions invalid")]
    InvalidDimensions,

    #[error("image already exists")]
    DuplicateImage,
}
