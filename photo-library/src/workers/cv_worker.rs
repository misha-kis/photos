use crate::{
    Command,
    workers::{db_worker::DbWorker, image_loader_worker::ImageLoader},
};
use anyhow::{Context, Result};
use cv::{BoundingBox, ClusteringConfig, FaceDetector, FaceEmbedder, cluster_embeddings};
use image::DynamicImage;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::{Mutex, mpsc, oneshot};

pub struct CvConfig {
    pub face_detector_model_path: PathBuf,
    pub face_embedder_model_path: PathBuf,
    pub face_detector_image_size: u32,
    pub face_embedder_image_size: u32,
}

pub struct CvWorker {
    db_worker: Arc<Mutex<DbWorker>>,
    image_loader: Arc<Mutex<ImageLoader>>,
    face_detector: FaceDetector,
    face_embedder: FaceEmbedder,
}

impl CvWorker {
    pub(crate) fn new(
        config: &CvConfig,
        db_worker: Arc<Mutex<DbWorker>>,
        image_loader: Arc<Mutex<ImageLoader>>,
    ) -> Result<Self> {
        Ok(Self {
            db_worker,
            image_loader,
            face_detector: FaceDetector::new(
                config.face_detector_model_path.clone(),
                config.face_detector_image_size as usize,
            )?,
            face_embedder: FaceEmbedder::new(
                config.face_embedder_model_path.clone(),
                config.face_embedder_image_size as usize,
            )?,
        })
    }

    pub fn detect_faces(&mut self, image: DynamicImage) -> Result<Vec<BoundingBox>> {
        self.face_detector.detect(image)
    }

    pub fn embed_face(&mut self, image: DynamicImage) -> Result<[f32; 512]> {
        self.face_embedder.generate_embedding(image)
    }
}

pub(crate) struct DetectFacesCommand {
    pub(crate) image_id: u32,
    tx: oneshot::Sender<DetectFacesCommandResult>,
}

impl DetectFacesCommand {
    pub(crate) fn new(image_id: u32, tx: oneshot::Sender<DetectFacesCommandResult>) -> Self {
        Self { image_id, tx }
    }

    pub(crate) async fn execute(
        self,
        worker: &mut CvWorker,
        cmd_tx: &mpsc::Sender<Command>,
    ) -> Result<()> {
        tracing::debug!("Detecting faces");
        let image = worker
            .image_loader
            .lock()
            .await
            .get_image_no_cache(self.image_id)
            .await?;
        tracing::debug!("Image loaded");
        let faces = worker.detect_faces(image)?;
        tracing::debug!("Faces detected");
        let mut result_rxs = Vec::new();
        for face in faces {
            let detection_id = worker
                .db_worker
                .lock()
                .await
                .insert_face_detection(self.image_id, face)
                .await?;
            let (tx, rx) = oneshot::channel();
            cmd_tx
                .send(Command::EmbedFace(CreateEmbeddingCommand::new(
                    detection_id,
                    tx,
                )))
                .await?;
            result_rxs.push(rx);
        }
        if let Err(e) = self.tx.send(DetectFacesCommandResult { rxs: result_rxs }) {
            tracing::warn!("Failed to send result: {:?}", e);
        }
        tracing::debug!("Inserted faces, created embedding tasks");
        Ok(())
    }
}

impl std::fmt::Debug for DetectFacesCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectFacesCommand")
            .field("image_id", &self.image_id)
            .finish()
    }
}

pub struct DetectFacesCommandResult {
    pub rxs: Vec<oneshot::Receiver<CreateEmbeddingCommandResult>>,
}

impl std::fmt::Debug for DetectFacesCommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateEmbeddingCommand").finish()
    }
}

pub(crate) struct CreateEmbeddingCommand {
    pub(crate) detection_id: u32,
    pub(crate) tx: oneshot::Sender<CreateEmbeddingCommandResult>,
}

impl CreateEmbeddingCommand {
    pub(crate) fn new(
        detection_id: u32,
        tx: oneshot::Sender<CreateEmbeddingCommandResult>,
    ) -> Self {
        Self { detection_id, tx }
    }

    pub(crate) async fn execute(self, worker: &mut CvWorker) -> Result<()> {
        let result = self
            .do_execute(worker)
            .await
            .context("Failed to create embedding")?;
        if let Err(e) = self.tx.send(result) {
            tracing::warn!("Failed to send result: {:?}", e);
        }
        Ok(())
    }

    async fn do_execute(&self, worker: &mut CvWorker) -> Result<CreateEmbeddingCommandResult> {
        tracing::debug!("Creating embedding for detection ID: {}", self.detection_id);
        let (image_id, bounding_box) = worker
            .db_worker
            .lock()
            .await
            .get_face_detection(self.detection_id)
            .await
            .context("Failed to get face detection")?;
        tracing::debug!("Image ID: {}", image_id);
        let image = worker
            .image_loader
            .lock()
            .await
            .get_image_no_cache(image_id)
            .await?;
        let cropped_image = image.crop_imm(
            bounding_box.x1 as u32,
            bounding_box.y1 as u32,
            bounding_box.width() as u32,
            bounding_box.height() as u32,
        );
        tracing::debug!("Image loaded");
        let embedding = worker.embed_face(cropped_image)?;
        tracing::debug!("Embedding created");
        worker
            .db_worker
            .lock()
            .await
            .insert_face_embedding(self.detection_id, embedding)
            .await?;
        tracing::debug!("Embedding inserted");
        Ok(CreateEmbeddingCommandResult {})
    }
}

impl std::fmt::Debug for CreateEmbeddingCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateEmbeddingCommand")
            .field("detection_id", &self.detection_id)
            .finish()
    }
}

#[derive(Debug)]
pub struct CreateEmbeddingCommandResult {}

pub(crate) struct ClusterFacesCommand {
    pub(crate) config: ClusteringConfig,
    pub(crate) tx: oneshot::Sender<ClusterFacesCommandResult>,
}

impl ClusterFacesCommand {
    pub(crate) fn new(
        config: ClusteringConfig,
        tx: oneshot::Sender<ClusterFacesCommandResult>,
    ) -> Self {
        Self { config, tx }
    }

    pub(crate) async fn execute(self, worker: &mut CvWorker) -> Result<()> {
        tracing::info!("Starting face clustering");
        let result = self
            .do_execute(worker)
            .await
            .context("Failed to cluster faces")?;
        if let Err(e) = self.tx.send(result) {
            tracing::warn!("Failed to send clustering result: {:?}", e);
        }
        Ok(())
    }

    async fn do_execute(&self, worker: &mut CvWorker) -> Result<ClusterFacesCommandResult> {
        // Retrieve all face embeddings from database
        let embeddings_data = worker
            .db_worker
            .lock()
            .await
            .get_all_face_embeddings()
            .await
            .context("Failed to retrieve face embeddings")?;

        if embeddings_data.is_empty() {
            tracing::warn!("No face embeddings found for clustering");
            return Ok(ClusterFacesCommandResult {
                n_clusters: 0,
                n_processed: 0,
            });
        }

        tracing::info!("Clustering {} face embeddings", embeddings_data.len());

        // Extract embeddings and detection IDs
        let detection_ids: Vec<u32> = embeddings_data.iter().map(|(id, _)| *id).collect();
        let embeddings: Vec<[f32; 512]> = embeddings_data.into_iter().map(|(_, emb)| emb).collect();

        // Perform clustering
        let clustering_result = cluster_embeddings(&embeddings, self.config.clone())
            .context("Failed to cluster embeddings")?;

        tracing::info!(
            "Clustering completed: {} clusters found, {} noise points",
            clustering_result.n_clusters,
            clustering_result.labels.iter().filter(|&&l| l == -1).count()
        );

        // Prepare bulk update data
        let updates: Vec<(u32, Option<u32>)> = detection_ids
            .iter()
            .zip(clustering_result.labels.iter())
            .map(|(&detection_id, &label)| {
                let face_id = if label >= 0 {
                    Some(label as u32)
                } else {
                    None // Noise/outlier
                };
                (detection_id, face_id)
            })
            .collect();

        // Bulk update face_id in database
        worker
            .db_worker
            .lock()
            .await
            .bulk_update_face_ids(updates)
            .await
            .context("Failed to bulk update face IDs")?;

        tracing::info!("Face IDs updated in database");

        Ok(ClusterFacesCommandResult {
            n_clusters: clustering_result.n_clusters,
            n_processed: detection_ids.len(),
        })
    }
}

impl std::fmt::Debug for ClusterFacesCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClusterFacesCommand")
            .field("min_cluster_size", &self.config.min_cluster_size)
            .field("min_samples", &self.config.min_samples)
            .finish()
    }
}

#[derive(Debug)]
pub struct ClusterFacesCommandResult {
    pub n_clusters: usize,
    pub n_processed: usize,
}
