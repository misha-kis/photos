use photos_core::Uuid;
use photos_domain::{
    BoundingBox, ClusteredFaceDetection, DynamicImage, FaceDetection, FaceDetectionWithEmbedding,
    ImageId, ImageRecord,
};
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum ResizeServiceError {
    #[error("could not resize")]
    ResizeServiceError,
    #[error("failed to build image with format {format}")]
    ImageFromRaw { format: &'static str },
    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait ResizeService {
    fn resize(
        &self,
        image: &DynamicImage,
        width: u32,
        height: u32,
    ) -> Result<DynamicImage, ResizeServiceError>;
}

#[derive(thiserror::Error, Debug)]
pub enum ImageMetadataRepositoryError {
    #[error("query failed: {err}")]
    QueryFailed { err: String },
    #[error("invalid image format")]
    InvalidImageFormat,
    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[async_trait::async_trait]
pub trait ImageMetadataRepository {
    async fn add_image_record(
        &self,
        image_record: &ImageRecord,
    ) -> Result<(), ImageMetadataRepositoryError>;
    async fn add_image_record_bulk(
        &self,
        image_records: &[ImageRecord],
    ) -> Result<(), ImageMetadataRepositoryError>;
    async fn get_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<ImageRecord, ImageMetadataRepositoryError>;
    async fn delete_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<(), ImageMetadataRepositoryError>;

    async fn get_image_ids(&self) -> Result<Vec<ImageId>, ImageMetadataRepositoryError>;
    async fn get_face_ids(&self) -> Result<Vec<Uuid>, ImageMetadataRepositoryError>;
    /// Returns clusters: each item is (cluster_face_uuid, list of detection uuids in that cluster).
    async fn get_face_clusters(
        &self,
    ) -> Result<Vec<(Uuid, Vec<Uuid>)>, ImageMetadataRepositoryError>;
    async fn get_number_of_images(&self) -> Result<u64, ImageMetadataRepositoryError>;
    async fn get_image_records_without_detections(
        &self,
    ) -> Result<Vec<ImageRecord>, ImageMetadataRepositoryError>;
    async fn add_detections_to_image(
        &self,
        image_record: &ImageId,
        face_detections: Vec<FaceDetection>,
    ) -> Result<(), ImageMetadataRepositoryError>;
    async fn get_detections_without_embeddings(
        &self,
    ) -> Result<Vec<(ImageRecord, FaceDetection)>, ImageMetadataRepositoryError>;
    async fn update_face_detection_with_embedding(
        &self,
        face_detection_with_embedding: FaceDetectionWithEmbedding,
    ) -> Result<(), ImageMetadataRepositoryError>;
    async fn get_detections_with_embeddings(
        &self,
    ) -> Result<Vec<FaceDetectionWithEmbedding>, ImageMetadataRepositoryError>;
    async fn update_detections_with_clusters(
        &self,
        clustered_face_detections: &[ClusteredFaceDetection],
    ) -> Result<(), ImageMetadataRepositoryError>;
    async fn get_min_detection_bbox_and_image_for_face_id(
        &self,
        face_id: Uuid,
    ) -> Result<(BoundingBox, ImageRecord), ImageMetadataRepositoryError>;
    async fn get_detections_for_face_id(
        &self,
        face_id: Uuid,
    ) -> Result<Vec<(Uuid, BoundingBox, ImageRecord)>, ImageMetadataRepositoryError>;
    async fn get_bbox_and_image_for_detection_id(
        &self,
        detection_id: Uuid,
    ) -> Result<(BoundingBox, ImageRecord), ImageMetadataRepositoryError>;
}

#[derive(thiserror::Error, Debug)]
pub enum ImageRepositoryError {
    #[error("the requested thumbnail size is invalid")]
    InvalidThumbnailSize,
    #[error("the requested image does not exist")]
    ImageDoesNotExist,
    #[error("failed to read timestamps")]
    FailedToReadTimestamps,
    #[error("image error: {err}")]
    ImageError { err: String },
    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait ImageRepository {
    fn insert_image(&self, path: &Path) -> Result<ImageRecord, ImageRepositoryError>;
    fn delete_image(&self, image_record: &ImageRecord) -> Result<(), ImageRepositoryError>;
    fn get_image(&self, image_record: &ImageRecord) -> Result<DynamicImage, ImageRepositoryError>;
    fn get_thumbnail(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, ImageRepositoryError>;
    fn get_thumbnail_from_file(
        &self,
        path: &Path,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, ImageRepositoryError>;
    fn get_face_thumbnail(
        &self,
        image_record: &ImageRecord,
        bounding_box: BoundingBox,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, ImageRepositoryError>;
}

#[derive(thiserror::Error, Debug)]
pub enum ImageAnalysisServiceError {
    #[error("could not infer")]
    CouldNotInfer,
    #[error("internal error")]
    Internal(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub trait ImageAnalysisService {
    fn get_face_detections(
        &self,
        image: &DynamicImage,
        resize_service: &dyn ResizeService,
    ) -> Result<Vec<FaceDetection>, ImageAnalysisServiceError>;
    fn get_face_embedding(
        &self,
        image: &DynamicImage,
        face_detection: FaceDetection,
        resize_service: &dyn ResizeService,
    ) -> Result<FaceDetectionWithEmbedding, ImageAnalysisServiceError>;

    fn cluster_embeddings(
        &self,
        detections_with_embeddings: Vec<FaceDetectionWithEmbedding>,
    ) -> Result<Vec<ClusteredFaceDetection>, ImageAnalysisServiceError>;
}

pub trait ServiceRegistry: Send + Sync {
    fn image_repo(&self) -> &dyn ImageRepository;
    fn image_meta_repo(&self) -> &dyn ImageMetadataRepository;
    fn resize_service(&self) -> &dyn ResizeService;
    fn analysis_service(&self) -> &dyn ImageAnalysisService;
}
