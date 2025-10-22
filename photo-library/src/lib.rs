mod config;
mod workers;

use crate::config::Config;
use crate::workers::db_worker::{DbWorkerCmd, spawn_db_worker};
use crate::workers::image_loader_worker::ImageLoadCmd;
use crate::workers::import_worker::spawn_import_worker;
use anyhow::{Context, Result, anyhow};
use image::DynamicImage;
use std::fs::create_dir;
use std::path::PathBuf;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

const THUMBNAILS_SUBDIRECTORY: &str = "thumbnails";
const ORIGINALS_SUBDIRECTORY: &str = "originals";
const DB_PATH: &str = "db.db";

pub struct PhotoLibrary {
    db_worker: JoinHandle<()>,
    db_cmd_tx: Sender<DbWorkerCmd>,
    thumbnail_worker: JoinHandle<()>,
    thumbnail_cmd_tx: Sender<ImageLoadCmd>,
    thumbnail_res_rx: Receiver<Result<DynamicImage>>,
    import_worker: JoinHandle<()>,
    import_cmd_tx: Sender<Vec<PathBuf>>,
    import_res_rx: Receiver<Vec<anyhow::Error>>,
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
        let (db_worker, db_cmd_tx) = spawn_db_worker(&config.library_path).await?;

        let (thumbnail_worker, thumbnail_cmd_tx, thumbnail_res_rx) =
            workers::image_loader_worker::spawn_image_loader(
                db_cmd_tx.clone(),
                config.library_path.join(THUMBNAILS_SUBDIRECTORY),
                config.library_path.join(ORIGINALS_SUBDIRECTORY),
            );

        let (import_worker, import_cmd_tx, import_res_rx) = spawn_import_worker(
            db_cmd_tx.clone(),
            config.library_path.join(THUMBNAILS_SUBDIRECTORY),
            config.library_path.join(ORIGINALS_SUBDIRECTORY),
            config.thumbnail_sizes,
        );
        Ok(Self {
            db_worker,
            db_cmd_tx,
            thumbnail_worker,
            thumbnail_cmd_tx,
            thumbnail_res_rx,
            import_worker,
            import_cmd_tx,
            import_res_rx,
        })
    }

    pub async fn get_thumbnail(&mut self, photo_id: u32) -> Result<DynamicImage> {
        self.thumbnail_cmd_tx
            .send(ImageLoadCmd::LoadThumbnail(photo_id))
            .await?;
        self.thumbnail_res_rx.recv().await.unwrap()
    }

    pub async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
        self.thumbnail_cmd_tx
            .send(ImageLoadCmd::LoadFullImage(photo_id))
            .await?;
        self.thumbnail_res_rx.recv().await.unwrap()
    }

    pub async fn import_photo(&mut self, photo_path: PathBuf) -> Result<()> {
        self.import_cmd_tx.send(vec![photo_path]).await?;
        let _res = self
            .import_res_rx
            .recv()
            .await
            .expect("no receive or what?");
        Ok(())
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
    use super::*;
    use tempdir::TempDir;
    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }
    #[tokio::test]
    async fn test_init_photo_library() {
        let config = Config::new(workspace_path().join("test_data").join("example_library"))
            .with_thumbnail_sizes(vec![32, 64]);
        let _ = PhotoLibrary::new(config).await.unwrap();
    }

    #[tokio::test]
    async fn test_import_photo_and_get_image() {
        let temp_dir = TempDir::new("photo_library").unwrap();
        let config = Config::new(temp_dir.path().to_path_buf()).with_thumbnail_sizes(vec![32]);
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
}
