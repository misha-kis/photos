mod face_clustering;
mod face_detection;
mod face_embedding;

use std::{path::PathBuf, sync::Mutex};

pub use face_clustering::{ClusteringConfig, ClusteringResult, cluster_embeddings};
pub use face_detection::FaceDetector;
pub use face_embedding::FaceEmbedder;
use image::DynamicImage;
use photos_services::{ImageAnalysisService, ImageAnalysisServiceError, ResizeService};

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
        let face_detector =
            FaceDetector::new(config.detector_model_path, config.detector_image_size)
                .map_err(|_| ImageAnalysisServiceError::CouldNotInitialize)?;
        let face_embedder =
            FaceEmbedder::new(config.embedder_model_path, config.embedder_image_size)
                .map_err(|_| ImageAnalysisServiceError::CouldNotInitialize)?;
        Ok(Self {
            face_detector: Mutex::new(face_detector),
            face_embedder: Mutex::new(face_embedder),
        })
    }
}

impl ImageAnalysisService for ImageAnalysis {
    fn get_face_detections(
        &self,
        image: &DynamicImage,
        resize_service: &dyn ResizeService,
    ) -> Result<Vec<photos_domain::FaceDetection>, ImageAnalysisServiceError> {
        self.face_detector
            .lock()
            .map_err(|_| ImageAnalysisServiceError::CouldNotInfer)?
            .detect(image, resize_service)
    }

    fn get_face_embedding(
        &self,
        image: &DynamicImage,
        face_detection: photos_domain::FaceDetection,
        resize_service: &dyn ResizeService,
    ) -> Result<photos_domain::FaceDetectionWithEmbedding, ImageAnalysisServiceError> {
        self.face_embedder
            .lock()
            .map_err(|_| ImageAnalysisServiceError::CouldNotInfer)?
            .generate_embedding(image, face_detection, resize_service)
    }

    fn cluster_embeddings(
        &self,
        detections_with_embeddings: Vec<(u32, [f32; 512])>,
    ) -> Result<Vec<(u32, Option<u32>)>, ImageAnalysisServiceError> {
        let embeddings: Vec<_> = detections_with_embeddings.iter().map(|(_, e)| *e).collect();
        let clustered_embeddings = cluster_embeddings(&embeddings, ClusteringConfig::default())
            .map_err(|_| ImageAnalysisServiceError::CouldNotInfer)?;
        let result = detections_with_embeddings
            .iter()
            .map(|(id, _)| *id)
            .zip(clustered_embeddings.labels.iter().copied())
            .collect();
        Ok(result)
    }
}
