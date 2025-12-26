use photos_domain::{
    DynamicImage, FaceDetection, FaceDetectionWithEmbedding, ImageId, ImageMeta, ImageRecord,
};
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ResizeServiceError {
    #[error("could not resize")]
    ResizeServiceError,
}

pub trait ResizeService {
    fn resize(&mut self, image: &DynamicImage, width: u32, height: u32) -> Result<DynamicImage, ResizeServiceError>;
}

#[derive(thiserror::Error, Debug)]
pub enum ImageMetadataRepositoryError {
    #[error("image metadata repository failure")]
    ImageMetadataRepositoryError,
}

#[async_trait::async_trait]
pub trait ImageMetadataRepository {
    async fn register_image(
        &mut self,
        image_meta: ImageMeta,
    ) -> Result<ImageRecord, ImageMetadataRepositoryError>;
    async fn get_image(
        &self,
        image_id: ImageId,
    ) -> Result<ImageRecord, ImageMetadataRepositoryError>;
    async fn delete_image(&mut self, image_id: ImageId)
    -> Result<(), ImageMetadataRepositoryError>;
}

#[derive(thiserror::Error, Debug)]
pub enum ImageRepositoryError {
    #[error("image metadata repository failure")]
    ImageRepositoryError,
    #[error("the requested thumbnail size is invalid")]
    InvalidThumbnailSize,
}

pub trait ImageRepository {
    fn insert_image(&mut self, path: &PathBuf) -> Result<(), ImageRepositoryError>;
    fn delete_image(&mut self, image_record: &ImageRecord) -> Result<(), ImageRepositoryError>;
    fn get_image(&self, image_record: &ImageRecord) -> Result<DynamicImage, ImageRepositoryError>;
    fn get_thumbnail(
        &mut self,
        image_record: &ImageRecord,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, ImageRepositoryError>;
}

#[derive(thiserror::Error, Debug)]
pub enum ImageAnalysisServiceError {
    #[error("image metadata repository failure")]
    ImageAnalysisServiceError,
}

pub trait ImageAnalysisService {
    fn detect_faces(
        &self,
        image: &DynamicImage,
    ) -> Result<Vec<FaceDetection>, ImageAnalysisServiceError>;
    fn get_embedding(
        &self,
        image: &DynamicImage,
        face_detection: FaceDetection,
    ) -> Result<FaceDetectionWithEmbedding, ImageAnalysisServiceError>;
}

pub trait ServiceRegistry: Send + Sync {
    fn image_repo(&self) -> &dyn ImageRepository;
    fn image_meta_repo(&self) -> &dyn ImageMetadataRepository;
    fn thumbnail_service(&self) -> &dyn ResizeService;
    fn image_analysis_service(&self) -> &dyn ImageAnalysisService;
}
