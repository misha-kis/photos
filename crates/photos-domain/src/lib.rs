extern crate image;

mod errors;
mod image_features;
pub use errors::DomainError;

pub use image::DynamicImage;
pub use image_features::{BoundingBox, FaceDetection, FaceDetectionWithEmbedding};

pub type ImageId = photos_core::Uuid;

pub enum ImageFormat {
    Jpeg,
    Png,
    Webp,
}

impl AsRef<str> for ImageFormat {
    fn as_ref(&self) -> &str {
        match self {
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Png => "png",
            ImageFormat::Webp => "webp",
        }
    }
}

impl TryFrom<&str> for ImageFormat {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "jpeg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            "webp" => Ok(Self::Webp),
            _ => Err(DomainError::UnsupportedFormat)
        }
    }
}

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

pub struct ImageMeta {
    pub dimensions: Dimensions,
    pub format: ImageFormat,
}

pub struct ImageRecord {
    pub id: ImageId,
    pub meta: ImageMeta,
}
