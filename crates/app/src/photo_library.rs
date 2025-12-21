use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use cv::ClusteringConfig;
use image::DynamicImage;
use parking_lot::RwLock;
use photo_library::{Config, CvConfig, FaceDetection, FaceThumbnail, PhotoLibrary};
use tokio::runtime::Runtime;

type SharedImage = Arc<RwLock<Option<Result<DynamicImage>>>>;

pub(crate) struct PhotoLibraryProxy {
    library: Arc<tokio::sync::Mutex<PhotoLibrary>>,
    runtime: Runtime,
    thumbnail_load_requests: HashMap<u32, SharedImage>,
    image_load_requests: HashMap<u32, SharedImage>,
    face_thumbnail_load_requests: HashMap<u32, SharedImage>,
    number_of_images: usize,
    clustering_in_progress: Arc<std::sync::atomic::AtomicBool>,
}

impl PhotoLibraryProxy {
    pub fn new(gallery_dir: PathBuf) -> Result<Self> {
        let workspace_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .context("missing workspace parent")?
            .to_path_buf();
        let cv_cfg = CvConfig {
            face_detector_model_path: workspace_path.join("models").join("yolov12n-face.onnx"),
            face_embedder_model_path: workspace_path.join("models").join("facenet.onnx"),
            face_detector_image_size: 480,
            face_embedder_image_size: 160,
        };
        let cfg = Config::new(gallery_dir, cv_cfg);
        let runtime =
            Runtime::new().context("Failed to start background tokio runtime for photo library")?;

        let library = runtime.block_on(PhotoLibrary::new(cfg))?;

        let number_of_images = runtime.block_on(async { library.get_number_of_images().await })?;

        let library = Arc::new(tokio::sync::Mutex::new(library));
        Ok(Self {
            library,
            runtime,
            thumbnail_load_requests: HashMap::new(),
            image_load_requests: HashMap::new(),
            face_thumbnail_load_requests: HashMap::new(),
            number_of_images,
            clustering_in_progress: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    pub fn get_number_of_images(&self) -> usize {
        self.number_of_images
    }

    pub fn try_get_thumbnail(&mut self, id: u32) -> Result<Option<DynamicImage>> {
        if let Some(entry) = self.thumbnail_load_requests.get(&id) {
            let result = entry.write().take();
            if let Some(result) = result {
                self.thumbnail_load_requests.remove(&id);
                return result.map(Some);
            }
            return Ok(None);
        }

        let entry = Arc::new(RwLock::new(None));
        self.thumbnail_load_requests.insert(id, entry.clone());
        let library = self.library.clone();
        self.runtime.handle().spawn(async move {
            let result = async {
                let mut library = library.lock().await;
                library.get_thumbnail(id).await
            }
            .await;
            entry.write().replace(result);
        });

        Ok(None)
    }

    pub fn try_get_image(&mut self, id: u32) -> Result<Option<DynamicImage>> {
        if let Some(entry) = self.image_load_requests.get(&id) {
            let result = entry.write().take();
            if let Some(result) = result {
                self.image_load_requests.remove(&id);
                return result.map(Some);
            }
            return Ok(None);
        }

        let entry = Arc::new(RwLock::new(None));
        self.image_load_requests.insert(id, entry.clone());
        let library = self.library.clone();
        self.runtime.handle().spawn(async move {
            let result = async {
                let mut library = library.lock().await;
                library.get_full_image(id).await
            }
            .await;
            entry.write().replace(result);
        });

        Ok(None)
    }

    pub fn import_photo(&mut self, path: PathBuf) -> Result<()> {
        let library = self.library.clone();
        self.runtime.block_on(async {
            let mut library = library.lock().await;
            library.import_photo(path).await?;
            Ok::<(), anyhow::Error>(())
        })?;
        Ok(())
    }

    pub fn refresh_image_count(&mut self) -> Result<()> {
        let library = self.library.clone();
        self.number_of_images = self.runtime.block_on(async {
            let library = library.lock().await;
            library.get_number_of_images().await
        })?;
        Ok(())
    }

    pub fn is_clustering_in_progress(&self) -> bool {
        self.clustering_in_progress
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn start_clusterization(&mut self) {
        if self
            .clustering_in_progress
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {
            return;
        }

        let library = self.library.clone();
        let flag = self.clustering_in_progress.clone();

        self.runtime.handle().spawn(async move {
            let result = {
                let mut library = library.lock().await;
                let config = ClusteringConfig::default();
                library.cluster_faces(config).await
            };

            match result {
                Ok(cluster_result) => {
                    tracing::info!(
                        "Clustering completed: {} clusters, {} processed",
                        cluster_result.n_clusters,
                        cluster_result.n_processed
                    );
                }
                Err(e) => {
                    tracing::error!("Clustering failed: {}", e);
                }
            }

            flag.store(false, std::sync::atomic::Ordering::Release);
        });
    }

    pub fn get_faces_grouped_by_id(&mut self) -> Result<HashMap<u32, Vec<FaceDetection>>> {
        let library = self.library.clone();
        self.runtime.block_on(async {
            let library = library.lock().await;
            library.get_faces_grouped_by_id().await
        })
    }

    pub fn get_unique_face_thumbnails(&mut self) -> Result<Vec<FaceThumbnail>> {
        let library = self.library.clone();
        self.runtime.block_on(async {
            let library = library.lock().await;
            library.get_unique_face_thumbnails().await
        })
    }

    pub fn try_get_face_thumbnail(&mut self, detection_id: u32) -> Result<Option<DynamicImage>> {
        if let Some(entry) = self.face_thumbnail_load_requests.get(&detection_id) {
            let result = entry.write().take();
            if let Some(result) = result {
                self.face_thumbnail_load_requests.remove(&detection_id);
                return result.map(Some);
            }
            return Ok(None);
        }

        let entry = Arc::new(RwLock::new(None));
        self.face_thumbnail_load_requests
            .insert(detection_id, entry.clone());
        let library = self.library.clone();
        self.runtime.handle().spawn(async move {
            let result = async {
                let mut library = library.lock().await;
                library.get_face_thumbnail(detection_id).await
            }
            .await;
            entry.write().replace(result);
        });

        Ok(None)
    }
}
