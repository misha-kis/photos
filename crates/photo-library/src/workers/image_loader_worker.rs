use crate::workers::db_worker::DbWorker;
use anyhow::{Context, Result};
use image::DynamicImage;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct FaceThumbnail {
    pub face_id: u32,
    pub thumbnail: DynamicImage,
    pub face_detection_id: u32,
}

pub struct ImageLoader {
    db_worker: Arc<Mutex<DbWorker>>,
    thumbnails_path: PathBuf,
    full_images_path: PathBuf,
    image_name_map: HashMap<u32, String>,
}

impl ImageLoader {
    pub async fn new(
        db_worker: Arc<Mutex<DbWorker>>,
        thumbnails_path: PathBuf,
        originals_path: PathBuf,
    ) -> Result<Self> {
        let image_name_map = db_worker.lock().await.get_image_names().await?;

        Ok(Self {
            db_worker,
            thumbnails_path,
            full_images_path: originals_path,
            image_name_map,
        })
    }

    pub(crate) async fn get_thumbnail(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self
            .image_name_map
            .get(&photo_id)
            .context("Image name not found")?;

        tracing::debug!("getting thumbnail from disk for photo id {}", photo_id);
        let path = self.thumbnails_path.join(format!("{}", 32)).join(name); // todo(other sizes)
        let result = image::open(path).context("Failed to load thumbnail from disk")?;
        Ok(result)
    }
    pub(crate) async fn get_full_image(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self
            .image_name_map
            .get(&photo_id)
            .context("Image name not found")?;

        tracing::debug!("getting image from disk for photo id {}", photo_id);
        let path = self.full_images_path.join(name);
        let result = image::open(path).context("Failed to load image from disk")?;
        Ok(result)
    }

    pub(crate) async fn get_image_no_cache(&mut self, photo_id: u32) -> Result<DynamicImage> {
        let name = self
            .image_name_map
            .get(&photo_id)
            .context("Image name not found")?;
        let path = self.full_images_path.join(name);
        tracing::debug!("getting image without cache at path {}", &path.display());
        let result = image::open(path)?;
        Ok(result)
    }

    pub(crate) async fn get_unique_face_thumbnails(&mut self) -> Result<Vec<FaceThumbnail>> {
        tracing::debug!("getting unique face thumbnails");
        let face_detections = self
            .db_worker
            .lock()
            .await
            .get_unique_face_detections()
            .await?;
        let mut full_images = Vec::new();
        tracing::debug!("got {} face detections", face_detections.len());
        for face_detection in face_detections.into_iter() {
            let full_image = self
                .get_full_image(face_detection.image_id)
                .await
                .context("Failed to get full image")?;
            full_images.push((
                full_image,
                face_detection.bounding_box,
                face_detection.detection_id,
                face_detection.face_id,
            ));
        }
        tracing::debug!("got {} full images", full_images.len());
        let face_thumbnails = full_images
            .into_par_iter()
            .map(|(full_image, bounding_box, detection_id, face_id)| {
                let x = bounding_box.x1 as u32;
                let y = bounding_box.y1 as u32;
                let w = bounding_box.x2 as u32 - x;
                let h = bounding_box.y2 as u32 - y;
                let thumbnail = full_image.crop_imm(x, y, w, h);
                FaceThumbnail {
                    face_id,
                    thumbnail,
                    face_detection_id: detection_id,
                }
            })
            .collect::<Vec<_>>();
        tracing::debug!("got {} face thumbnails", face_thumbnails.len());
        Ok(face_thumbnails)
    }

    pub(crate) async fn get_face_thumbnail(
        &mut self,
        face_detection_id: u32,
    ) -> Result<DynamicImage> {
        tracing::debug!(
            "getting face thumbnail for face detection id {}",
            face_detection_id
        );
        let face_detection = self
            .db_worker
            .lock()
            .await
            .get_face_detection(face_detection_id)
            .await?;
        let mut full_image = self.get_full_image(face_detection.0).await?;
        let bounding_box = face_detection.1;
        let x = bounding_box.x1 as u32;
        let y = bounding_box.y1 as u32;
        let w = bounding_box.x2 as u32 - x;
        let h = bounding_box.y2 as u32 - y;
        let thumbnail = full_image.crop(x, y, w, h);
        Ok(thumbnail)
    }
}

#[derive(Debug)]
pub(crate) struct UpdateImageNameMapCommand {
    pub(crate) new_image_name_map: HashMap<u32, String>,
}

impl UpdateImageNameMapCommand {
    pub(crate) async fn execute(self, image_loader: Arc<Mutex<ImageLoader>>) -> Result<()> {
        let mut image_loader = image_loader.lock().await;
        image_loader.image_name_map.extend(self.new_image_name_map);
        Ok(())
    }
}
