mod config;
mod workers;

use crate::config::Config;
use crate::workers::cv_worker::CvWorkerProxy;
use crate::workers::db_worker::DbWorkerProxy;
use crate::workers::image_loader_worker::ImageLoaderProxy;
use crate::workers::import_worker::ImportWorkerProxy;
use anyhow::{Context, Result, anyhow};
use image::DynamicImage;
use std::fs::create_dir;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const THUMBNAILS_SUBDIRECTORY: &str = "thumbnails";
const ORIGINALS_SUBDIRECTORY: &str = "originals";
const DB_PATH: &str = "db.db";

pub struct PhotoLibrary {
    db_worker_proxy: Arc<Mutex<DbWorkerProxy>>,
    image_loader_proxy: ImageLoaderProxy,
    import_worker_proxy: ImportWorkerProxy,
    cv_worker_proxy: CvWorkerProxy,
}

impl PhotoLibrary {
    pub async fn new(config: Config) -> Result<Self> {
        try_ensure_dir(&config.library_path)?;
        try_ensure_dir(&config.library_path.join(ORIGINALS_SUBDIRECTORY))?;
        try_ensure_dir(&config.library_path.join(THUMBNAILS_SUBDIRECTORY))?;
        for thumbnail_size in &config.thumbnail_sizes {
            try_ensure_dir(
                &config
                    .library_path
                    .join(THUMBNAILS_SUBDIRECTORY)
                    .join(format!("{thumbnail_size}")),
            )?;
        }
        let db_worker_proxy = Arc::new(Mutex::new(DbWorkerProxy::new(&config.library_path).await?));

        let image_loader_proxy = ImageLoaderProxy::new(
            db_worker_proxy.clone(),
            config.library_path.join(THUMBNAILS_SUBDIRECTORY),
            config.library_path.join(ORIGINALS_SUBDIRECTORY),
        )?;

        let import_worker_proxy = ImportWorkerProxy::new(
            db_worker_proxy.clone(),
            config.library_path.join(THUMBNAILS_SUBDIRECTORY),
            config.library_path.join(ORIGINALS_SUBDIRECTORY),
            config.thumbnail_sizes,
        );

        let cv_worker_proxy = CvWorkerProxy::new(&config.cv_config)?;

        Ok(Self {
            db_worker_proxy,
            image_loader_proxy,
            import_worker_proxy,
            cv_worker_proxy,
        })
    }

    pub async fn get_thumbnail(&mut self, photo_id: u32) -> Result<DynamicImage> {
        self.image_loader_proxy.load_thumbnail(photo_id).await
    }

    pub async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
        self.image_loader_proxy.load_full_image(photo_id).await
    }

    pub async fn import_photo(&mut self, photo_path: PathBuf) -> Result<()> {
        self.import_worker_proxy.import_photo(photo_path).await
    }
}

fn try_ensure_dir(path: &PathBuf) -> Result<()> {
    if !path.is_dir() {
        create_dir(path).context(format!(
            "expected successful creation of {}",
            path.to_str()
                .ok_or(anyhow!("the path is not a valid UTF-8 string"))?
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::workers::cv_worker::CvConfig;

    use super::*;
    use tempdir::TempDir;
    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn get_cv_config() -> CvConfig {
        CvConfig {
            face_detector_model_path: workspace_path().join("models").join("yolov12n-face.onnx"),
            face_embedder_model_path: workspace_path().join("models").join("facenet.onnx"),
            face_detector_image_size: 480,
            face_embedder_image_size: 160,
        }
    }

    #[tokio::test]
    async fn test_init_photo_library() {
        let temp_dir = TempDir::new("photo_library").unwrap();
        let config = Config::new(temp_dir.path().to_path_buf(), get_cv_config())
            .with_thumbnail_sizes(vec![32, 64]);
        let _ = PhotoLibrary::new(config).await.unwrap();
    }

    #[tokio::test]
    async fn test_import_photo_and_get_image() {
        let temp_dir = TempDir::new("photo_library").unwrap();
        let config = Config::new(temp_dir.path().to_path_buf(), get_cv_config())
            .with_thumbnail_sizes(vec![32]);
        let mut library = PhotoLibrary::new(config).await.unwrap();
        let new_image_path = workspace_path().join("test_data").join("example.jpeg");
        library
            .import_photo(new_image_path)
            .await
            .expect("could not import");

        let thumbnail = library.get_thumbnail(1).await.unwrap();
        assert_eq!(thumbnail.height(), 32);
        let full_image = library.get_full_image(1).await.unwrap();
        assert_eq!(full_image.height(), 1280);
    }

    #[tokio::test]
    async fn test_cv_worker() {
        let temp_dir = TempDir::new("photo_library").unwrap();
        let config = Config::new(temp_dir.path().to_path_buf(), get_cv_config())
            .with_thumbnail_sizes(vec![32]);
        let mut library = PhotoLibrary::new(config).await.unwrap();
        let new_image_path = workspace_path().join("test_data").join("example.jpeg");
        library
            .import_photo(new_image_path)
            .await
            .expect("could not import");

        let full_image = library.get_full_image(1).await.unwrap();
        assert_eq!(full_image.height(), 1280);

        let face_boxes = library.cv_worker_proxy.get_faces(full_image).await.unwrap();
        assert_eq!(face_boxes.len(), 1);
    }
}
