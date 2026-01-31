extern crate image;

mod errors;
mod image_features;

use chrono::{DateTime, Utc};
pub use errors::DomainError;
use std::cmp::Ordering;

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

#[derive(Debug, Clone)]
pub struct Timestamps {
    pub exif_timestamp: Option<DateTime<Utc>>,
    pub os_timestamp: DateTime<Utc>,
    pub import_timestamp: DateTime<Utc>,
}

impl Timestamps {
    pub fn best_creation_timestamp(&self) -> DateTime<Utc> {
        if let Some(ts) = self.exif_timestamp {
            ts
        } else {
            self.os_timestamp
        }
    }
}

impl Eq for Timestamps {}

impl PartialEq<Self> for Timestamps {
    fn eq(&self, other: &Self) -> bool {
        self.best_creation_timestamp()
            .eq(&other.best_creation_timestamp())
    }
}

impl PartialOrd<Self> for Timestamps {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamps {
    fn cmp(&self, other: &Self) -> Ordering {
        self.best_creation_timestamp()
            .cmp(&other.best_creation_timestamp())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRecord {
    pub id: ImageId,
    pub format: ImageFormat,
    pub timestamps: Timestamps,
}

impl Ord for ImageRecord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.timestamps.cmp(&other.timestamps) {
            core::cmp::Ordering::Equal => self.id.cmp(&other.id),
            ord => ord,
        }
    }
}

impl PartialOrd for ImageRecord {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
