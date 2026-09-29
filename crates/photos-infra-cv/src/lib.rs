mod apple_face_detection;
mod errors;
mod face_alignment;
mod face_clustering;
mod face_detection;
mod face_embedding;
mod geometry;

use std::{path::PathBuf, sync::Mutex};

use face_clustering::{ClusteringConfig, cluster_embeddings};
use face_detection::FaceDetector;
use face_embedding::FaceEmbedder;
use image::DynamicImage;
use photos_domain::{
    Affine2D, ClusteredFaceDetection, FaceDetection, FaceDetectionWithEmbedding, ImageRecord,
};
use photos_services::{
    ImageAnalysisService, ImageAnalysisServiceError, ImageMetadataRepository, ImageRepository,
    ResizeService,
};

pub struct ImageAnalysisConfig {
    pub detector_model_path: PathBuf,
    pub embedder_model_path: PathBuf,
    pub detector_image_size: u32,
    pub embedder_image_size: u32,
}

pub struct ImageAnalysis {
    face_detector: Mutex<FaceDetector>,
    face_embedder: Mutex<FaceEmbedder>,
}

impl ImageAnalysis {
    pub fn new(config: ImageAnalysisConfig) -> Result<Self, ImageAnalysisServiceError> {
        tracing::debug!("initializing models");
        let face_detector =
            FaceDetector::new(config.detector_model_path, config.detector_image_size)?;
        let face_embedder =
            FaceEmbedder::new(config.embedder_model_path, config.embedder_image_size)?;
        tracing::debug!("initializing models done");
        Ok(Self {
            face_detector: Mutex::new(face_detector),
            face_embedder: Mutex::new(face_embedder),
        })
    }
}

impl ImageAnalysisService for ImageAnalysis {
    fn get_face_detections(
        &self,
        image_record: &ImageRecord,
        image_repository: &dyn ImageRepository,
    ) -> Result<Vec<FaceDetection>, ImageAnalysisServiceError> {
        let image_path = image_repository.get_original_path(image_record);
        apple_face_detection::detect(&image_path.to_string_lossy())
            .map_err(|e| ImageAnalysisServiceError::Internal(Box::new(e)))
    }

    fn get_face_embedding(
        &self,
        image: &DynamicImage,
        face_detection: photos_domain::FaceDetection,
        resize_service: &dyn ResizeService,
    ) -> Result<FaceDetectionWithEmbedding, ImageAnalysisServiceError> {
        self.face_embedder
            .lock()
            .map_err(|_| ImageAnalysisServiceError::CouldNotInfer)?
            .generate_embedding(image, face_detection, resize_service)
    }

    fn cluster_embeddings(
        &self,
        detections_with_embeddings: Vec<FaceDetectionWithEmbedding>,
    ) -> Result<Vec<ClusteredFaceDetection>, ImageAnalysisServiceError> {
        let embeddings: Vec<_> = detections_with_embeddings
            .iter()
            .map(|d| d.embedding)
            .collect();
        let clustered_embeddings = cluster_embeddings(&embeddings, ClusteringConfig::default())?;
        let result = detections_with_embeddings
            .into_iter()
            .zip(clustered_embeddings.labels)
            .map(|(detection, cluster_id)| ClusteredFaceDetection {
                detection,
                cluster_id,
            })
            .collect();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use image::ImageFormat;
    use photos_domain::{ImageId, Timestamps};
    use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;

    use super::*;

    #[test]
    fn test_detection_embedding_pipeline() {
        let image_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("test_data")
            .join("example.jpeg");
        let image_record = ImageRecord {
            id: ImageId::default(),
            format: ImageFormat::Jpeg,
            timestamps: Timestamps {
                exif_timestamp: None,
                os_timestamp: chrono::Utc::now(),
                import_timestamp: chrono::Utc::now(),
            },
        };
        let detections = apple_face_detection::detect(image_path.to_str().unwrap()).unwrap();
        let image = image::open(image_path).unwrap();

        let face_detector_model_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("assets")
            .join("models")
            .join("facenet_240.onnx");
        let mut face_embedder = FaceEmbedder::new(face_detector_model_path, 160).unwrap();
        let resize_service = FastImageResizeResizer::default();
        for detection in detections {
            let embedding = face_embedder
                .generate_embedding(&image, detection, &resize_service)
                .unwrap();
        }
    }
}
