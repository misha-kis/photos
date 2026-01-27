use photos_services::ImageAnalysisServiceError;

pub trait IntoInternal<T> {
    fn internal(self) -> Result<T, ImageAnalysisServiceError>;
}

impl<T, E> IntoInternal<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn internal(self) -> Result<T, ImageAnalysisServiceError> {
        self.map_err(|e| ImageAnalysisServiceError::Internal(Box::new(e)))
    }
}
