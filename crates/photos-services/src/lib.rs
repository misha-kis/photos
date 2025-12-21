use photos_domain::{
    DomainError, DynamicImage, FaceDetection, FaceDetectionWithEmbedding, ImageId, ImageMeta,
    ImageRecord,
};
use std::path::PathBuf;

#[async_trait::async_trait]
pub trait ThumbnailService {
    async fn generate_thumbnail(
        &self,
        photo_id: ImageId,
        size: u32,
    ) -> Result<DynamicImage, DomainError>;
}

#[async_trait::async_trait]
pub trait ImageMetadataRepository {
    async fn register_image(&mut self, image_meta: ImageMeta) -> Result<ImageRecord, DomainError>;
    async fn get_image(&self, image_id: ImageId) -> Result<ImageRecord, DomainError>;
    async fn delete_image(&mut self, image_id: ImageId) -> Result<(), DomainError>;
}

#[async_trait::async_trait]
pub trait ImageRepository {
    fn insert_image(&mut self, path: PathBuf) -> Result<(), DomainError>;
    fn delete_image(&mut self, path: PathBuf) -> Result<(), DomainError>;
    fn get_image(&self, path: PathBuf) -> Result<DynamicImage, DomainError>;
}

pub trait ImageAnalysisService {
    fn detect_faces(&self, image: &DynamicImage) -> Result<Vec<FaceDetection>, DomainError>;
    fn get_embedding(
        &self,
        image: &DynamicImage,
        face_detection: FaceDetection,
    ) -> Result<FaceDetectionWithEmbedding, DomainError>;
}

pub trait ServiceRegistry: Send + Sync {
    fn image_repo(&self) -> &dyn ImageRepository;
    fn image_meta_repo(&self) -> &dyn ImageMetadataRepository;
    fn thumbnail_service(&self) -> &dyn ThumbnailService;
    fn image_analysis_service(&self) -> &dyn ImageAnalysisService;
}
