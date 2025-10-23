use anyhow::Result;
use cv::{BoundingBox, FaceDetector, FaceEmbedder};
use image::DynamicImage;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

pub struct CvConfig {
    pub face_detector_model_path: PathBuf,
    pub face_embedder_model_path: PathBuf,
    pub face_detector_image_size: u32,
    pub face_embedder_image_size: u32,
}

enum CvWorkerCmd {
    GetFaces {
        img: DynamicImage,
        res_tx: oneshot::Sender<Result<Vec<BoundingBox>>>,
    },
    GetEmbedding {
        img: DynamicImage,
        res_tx: oneshot::Sender<Result<[f32; 512]>>,
    },
}

struct CvWorker {
    face_detector: FaceDetector,
    face_embedder: FaceEmbedder,
}

impl CvWorker {
    fn new(config: &CvConfig) -> Result<Self> {
        Ok(Self {
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
}

pub(crate) struct CvWorkerProxy {
    cmd_tx: mpsc::Sender<CvWorkerCmd>,
}
impl CvWorkerProxy {
    pub(crate) fn new(config: &CvConfig) -> Result<Self> {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(32);

        let mut cv_worker = CvWorker::new(config)?;

        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    CvWorkerCmd::GetFaces { img, res_tx } => {
                        let boxes = cv_worker.face_detector.detect(img);
                        res_tx.send(boxes).expect("could not send");
                    }
                    CvWorkerCmd::GetEmbedding { img, res_tx } => {
                        let embedding = cv_worker.face_embedder.generate_embedding(img);
                        res_tx.send(embedding).expect("could not send");
                    }
                }
            }
        });

        Ok(Self { cmd_tx })
    }

    pub(crate) async fn get_faces(&self, img: DynamicImage) -> Result<Vec<BoundingBox>> {
        let (res_tx, res_rx) = oneshot::channel();
        self.cmd_tx
            .send(CvWorkerCmd::GetFaces { img, res_tx })
            .await?;
        res_rx.await?
    }

    pub(crate) async fn get_embedding(&self, img: DynamicImage) -> Result<[f32; 512]> {
        let (res_tx, res_rx) = oneshot::channel();
        self.cmd_tx
            .send(CvWorkerCmd::GetEmbedding { img, res_tx })
            .await?;
        res_rx.await?
    }
}
