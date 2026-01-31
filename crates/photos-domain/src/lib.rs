extern crate image;

mod errors;
mod image_features;
pub use errors::DomainError;

pub use image::DynamicImage;
use image::ImageFormat;
pub use image_features::{
    BoundingBox, ClusteredFaceDetection, FaceDetection, FaceDetectionWithEmbedding,
};

pub type ImageId = photos_core::Uuid;

pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    pub fn new(width: u32, height: u32) -> Result<Self, DomainError> {
        if width == 0 || height == 0 {
            return Err(DomainError::InvalidDimensions);
        }
        Ok(Self { width, height })
    }
}

pub struct ImageRecord {
    pub id: ImageId,
    pub format: ImageFormat,
}
