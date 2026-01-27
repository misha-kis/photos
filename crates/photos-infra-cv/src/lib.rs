mod errors;
mod face_clustering;
mod face_detection;
mod face_embedding;

use std::{path::PathBuf, sync::Mutex};

use face_clustering::{ClusteringConfig, cluster_embeddings};
use face_detection::FaceDetector;
use face_embedding::FaceEmbedder;
use image::DynamicImage;
use photos_domain::{ClusteredFaceDetection, FaceDetectionWithEmbedding};
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
        image: &DynamicImage,
        resize_service: &dyn ResizeService,
    ) -> Result<Vec<photos_domain::FaceDetection>, ImageAnalysisServiceError> {
        self.face_detector
            .lock()
            .map_err(|_| ImageAnalysisServiceError::CouldNotInfer)?
            .detect(image, resize_service)
            .map(|v| {
                v.into_iter()
                    .map(|d| photos_domain::FaceDetection {
                        uuid: photos_core::Uuid::now_v7(),
                        bounding_box: d.bounding_box,
                        confidence: d.confidence,
                    })
                    .collect()
            })
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
            .zip(clustered_embeddings.labels.into_iter())
            .map(|(detection, cluster_id)| ClusteredFaceDetection {
                detection,
                cluster_id,
            })
            .collect();
        Ok(result)
    }
}
