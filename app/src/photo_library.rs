use std::{collections::HashMap, path::PathBuf};

use image::DynamicImage;
use photo_library::{Config, CvConfig, PhotoLibrary};
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) struct PhotoLibraryProxy {
    rt: tokio::runtime::Runtime,
    library: Arc<Mutex<PhotoLibrary>>,
    thumbnail_load_requests: HashMap<u32, Arc<Mutex<Option<DynamicImage>>>>,
    image_load_requests: HashMap<u32, Arc<Mutex<Option<DynamicImage>>>>,
    number_of_images: usize,
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
            number_of_images,
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
}
