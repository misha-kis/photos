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

// impl ImageFormat {
//     pub fn as_u8(&self) -> u8 {
//         match self {
//             ImageFormat::Jpeg => 0,
//             ImageFormat::Png => 1,
//             ImageFormat::Webp => 2,
//         }
//     }
// }
//
// impl TryFrom<u8> for ImageFormat {
//     type Error = DomainError;
//
//     fn try_from(value: u8) -> Result<Self, Self::Error> {
//         match value {
//             0 => Ok(Self::Jpeg),
//             1 => Ok(Self::Png),
//             2 => Ok(Self::Webp),
//             _ => Err(DomainError::UnsupportedFormat),
//         }
//     }
// }
//
// impl AsRef<str> for ImageFormat {
//     fn as_ref(&self) -> &str {
//         match self {
//             ImageFormat::Jpeg => "jpeg",
//             ImageFormat::Png => "png",
//             ImageFormat::Webp => "webp",
//         }
//     }
// }
//
// impl TryFrom<&str> for ImageFormat {
//     type Error = DomainError;
//
//     fn try_from(value: &str) -> Result<Self, Self::Error> {
//         match value.to_lowercase().as_ref() {
//             "jpeg" | "jpg" => Ok(Self::Jpeg),
//             "png" => Ok(Self::Png),
//             "webp" => Ok(Self::Webp),
//             _ => Err(DomainError::UnsupportedFormat),
//         }
//     }
// }

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
    pub format: ImageFormat,
}

pub struct ImageRecord {
    pub id: ImageId,
    pub meta: ImageMeta,
}
