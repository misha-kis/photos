use crate::{
    Command,
    workers::{db_worker::DbWorker, image_loader_worker::ImageLoader},
};
use anyhow::Result;
use cv::{BoundingBox, FaceDetector, FaceEmbedder};
use futures::future::join_all;
use image::DynamicImage;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf, rc::Rc};
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
        self.tx.send(DetectFacesCommandResult { rxs: result_rxs });
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

pub(crate) struct DetectFacesCommandResult {
    pub(crate) rxs: Vec<oneshot::Receiver<CreateEmbeddingCommandResult>>,
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
        let result = self.do_execute(worker).await;
        self.tx.send(result?).expect("cound not send");
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
            .ok_or(anyhow::anyhow!("Face detection not found"))?;
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
pub(crate) struct CreateEmbeddingCommandResult {}
