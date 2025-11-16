use std::{collections::HashMap, path::PathBuf};

use image::DynamicImage;
use photo_library::{Config, CvConfig, FaceDetection, FaceThumbnail, PhotoLibrary};
use std::sync::Arc;
use tokio::sync::Mutex;
use cv::ClusteringConfig;

pub(crate) struct PhotoLibraryProxy {
    rt: tokio::runtime::Runtime,
    library: Arc<Mutex<PhotoLibrary>>,
    thumbnail_load_requests: HashMap<u32, Arc<Mutex<Option<DynamicImage>>>>,
    image_load_requests: HashMap<u32, Arc<Mutex<Option<DynamicImage>>>>,
    face_thumbnail_load_requests: HashMap<u32, Arc<Mutex<Option<DynamicImage>>>>,
    number_of_images: usize,
    clustering_in_progress: Arc<Mutex<bool>>,
}

impl PhotoLibraryProxy {
    pub fn new(gallery_dir: PathBuf) -> Self {
        let workspace_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let cv_cfg = CvConfig {
            face_detector_model_path: workspace_path.join("models").join("yolov12n-face.onnx"),
            face_embedder_model_path: workspace_path.join("models").join("facenet.onnx"),
            face_detector_image_size: 480,
            face_embedder_image_size: 160,
        };
        let cfg = Config::new(gallery_dir, cv_cfg);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let library = rt.block_on(PhotoLibrary::new(cfg)).unwrap();
        // let to_import_path = PathBuf::from("/Users/misha-kis/Pictures/pics2");
        // rt.block_on(library.import_photo(to_import_path)).unwrap();

        let number_of_images = rt.block_on(async {
            library.get_number_of_images().await.unwrap()
        });

        let library = Arc::new(Mutex::new(library));
        Self {
            rt,
            library,
            thumbnail_load_requests: HashMap::new(),
            image_load_requests: HashMap::new(),
            face_thumbnail_load_requests: HashMap::new(),
            number_of_images,
            clustering_in_progress: Arc::new(Mutex::new(false)),
        }
    }

    pub fn get_number_of_images(&self) -> usize {
        self.number_of_images
    }

    pub fn try_get_thumbnail(&mut self, id: u32) -> Option<DynamicImage> {
        if let Some(thumbnail) = self.thumbnail_load_requests.get(&id).cloned() {
            self.rt.block_on(thumbnail.lock()).clone()
        } else {
            let result = Arc::new(Mutex::new(None));
            self.thumbnail_load_requests.insert(id, result.clone());
            let library = self.library.clone();
            self.rt.spawn(async move {
                let mut library = library.lock().await;
                let future = library.get_thumbnail(id);
                let thumbnail = future.await.unwrap();
                result.lock().await.replace(thumbnail);
            });
            None
        }
    }

    pub fn try_get_image(&mut self, id: u32) -> Option<DynamicImage> {
        if let Some(image) = self.image_load_requests.get(&id).cloned() {
            self.rt.block_on(image.lock()).clone()
        } else {
            let result = Arc::new(Mutex::new(None));
            self.image_load_requests.insert(id, result.clone());
            let library = self.library.clone();
            self.rt.spawn(async move {
                let mut library = library.lock().await;
                let future = library.get_full_image(id);
                let image = future.await.unwrap();
                result.lock().await.replace(image);
            });
            None
        }
    }

    pub fn import_photo(&mut self, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let library = self.library.clone();
        self.rt.block_on(async {
            let mut library = library.lock().await;
            library.import_photo(path).await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        })?;
        Ok(())
    }

    pub fn refresh_image_count(&mut self) {
        let library = self.library.clone();
        self.number_of_images = self.rt.block_on(async {
            let library = library.lock().await;
            library.get_number_of_images().await.unwrap()
        });
    }

    pub fn is_clustering_in_progress(&self) -> bool {
        self.rt.block_on(async {
            *self.clustering_in_progress.lock().await
        })
    }

    pub fn start_clusterization(&mut self) {
        if self.is_clustering_in_progress() {
            return;
        }

        let library = self.library.clone();
        let clustering_in_progress = self.clustering_in_progress.clone();
        
        self.rt.block_on(async {
            *clustering_in_progress.lock().await = true;
        });
        
        self.rt.spawn(async move {
            let result = {
                let mut library = library.lock().await;
                let config = ClusteringConfig::default();
                library.cluster_faces(config).await
            };
            
            match result {
                Ok(cluster_result) => {
                    tracing::info!("Clustering completed: {} clusters, {} processed", 
                        cluster_result.n_clusters, cluster_result.n_processed);
                }
                Err(e) => {
                    tracing::error!("Clustering failed: {}", e);
                }
            }
            
            *clustering_in_progress.lock().await = false;
        });
    }

    pub fn get_faces_grouped_by_id(&mut self) -> Option<HashMap<u32, Vec<FaceDetection>>> {
        let library = self.library.clone();
        let faces = self.rt.block_on(async {
            let library = library.lock().await;
            library.get_faces_grouped_by_id().await
        });
        
        faces.ok()
    }

    pub fn get_unique_face_thumbnails(&mut self) -> Option<Vec<FaceThumbnail>> {
        let library = self.library.clone();
        let faces = self.rt.block_on(async {
            let library = library.lock().await;
            library.get_unique_face_thumbnails().await
        });
        
        faces.ok()
    }

    pub fn try_get_face_thumbnail(&mut self, detection_id: u32) -> Option<DynamicImage> {
        if let Some(thumbnail) = self.face_thumbnail_load_requests.get(&detection_id).cloned() {
            self.rt.block_on(thumbnail.lock()).clone()
        } else {
            let result = Arc::new(Mutex::new(None));
            self.face_thumbnail_load_requests.insert(detection_id, result.clone());
            let library = self.library.clone();
            self.rt.spawn(async move {
                let mut library = library.lock().await;
                let future = library.get_face_thumbnail(detection_id);
                if let Ok(thumbnail) = future.await {
                    result.lock().await.replace(thumbnail);
                }
            });
            None
        }
    }
}
