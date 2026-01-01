use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageEncoder};
use photos_domain::{ImageId, ImageMeta, ImageRecord};
use photos_services::{ImageRepository, ImageRepositoryError, ResizeService};
use std::fs::{copy, create_dir_all};
use std::path::{Path, PathBuf};

pub struct FSImageRepository<T: ResizeService> {
    pub path: PathBuf,
    pub thumbnail_sizes: Vec<u32>,
    resize_service: T,
}

impl<T: ResizeService> FSImageRepository<T> {
    pub fn new(path: PathBuf, thumbnail_sizes: Vec<u32>, resize_service: T) -> Self {
        Self {
            path,
            thumbnail_sizes,
            resize_service,
        }
    }

    fn original_path(&self, image_id: ImageId, extension: &str) -> PathBuf {
        let image_id_string = image_id.to_string();
        let image_id_split = image_id_string.split_at(2);
        self.path
            .join("originals")
            .join(image_id_split.0)
            .join(image_id_split.1)
            .with_added_extension(extension)
    }

    fn thumbnail_path(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
    ) -> Result<PathBuf, ImageRepositoryError> {
        if !self.thumbnail_sizes.contains(&thumbnail_size) {
            Err(ImageRepositoryError::InvalidThumbnailSize)
        } else {
            let image_id_string = image_id.to_string();
            let image_id_split = image_id_string.split_at(2);
            Ok(self
                .path
                .join("thumbnails")
                .join(thumbnail_size.to_string())
                .join(image_id_split.0)
                .join(image_id_split.1)
                .with_added_extension("jpeg"))
        }
    }

    fn thumbnail_paths(&self, image_id: ImageId) -> Vec<PathBuf> {
        let thumbnails_path = self.path.join("thumbnails");
        let image_id_string = image_id.to_string();
        let image_id_split = image_id_string.split_at(2);
        self.thumbnail_sizes
            .iter()
            .map(|thumbnail_size| {
                thumbnails_path
                    .join(thumbnail_size.to_string())
                    .join(image_id_split.0)
                    .join(image_id_split.1)
                    .with_added_extension("jpeg")
            })
            .collect()
    }
}

impl<T: ResizeService> ImageRepository for FSImageRepository<T> {
    fn insert_image(&self, image_path: &Path) -> Result<ImageRecord, ImageRepositoryError> {
        let image_id = ImageId::now_v7();
        let extension = image_path
            .extension()
            .unwrap()
            .to_str()
            .expect("has extension");
        let original_path = self.original_path(image_id, extension);
        ensure_dir(original_path.parent().expect("parent dir exists"))
            .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;
        copy(image_path, original_path).map_err(|_| ImageRepositoryError::ImageRepositoryError)?;
        let thumbnail_paths = self.thumbnail_paths(image_id);
        let image =
            image::open(image_path).map_err(|_| ImageRepositoryError::ImageRepositoryError)?;
        let width = image.width();
        let height = image.height();

        for (&thumbnail_size, thumbnail_path) in self.thumbnail_sizes.iter().zip(thumbnail_paths) {
            let (width, height) = thumbnail_width_height(width, height, thumbnail_size);

            let resized_image = self
                .resize_service
                .resize(&image, width, height)
                .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;

            ensure_dir(thumbnail_path.parent().expect("parent dir exists"))
                .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;

            let mut out_file = std::fs::File::create(&thumbnail_path)
                .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;

            let rgb_image = resized_image.to_rgb8();
            JpegEncoder::new(&mut out_file)
                .write_image(&rgb_image, width, height, image.color().into())
                .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;
        }

        let format = photos_domain::ImageFormat::try_from(extension)
            .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;

        Ok(ImageRecord {
            id: image_id,
            meta: ImageMeta {
                dimensions: photos_domain::Dimensions { width, height },
                format,
            },
        })
    }

    fn delete_image(&self, image_record: &ImageRecord) -> Result<(), ImageRepositoryError> {
        let original_path =
            self.original_path(image_record.id, image_record.meta.format.as_ref().as_ref());
        if !original_path.exists() {
            return Err(ImageRepositoryError::ImageDoesNotExist);
        }
        std::fs::remove_file(original_path)
            .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;
        for thumbnail_path in self.thumbnail_paths(image_record.id) {
            if !thumbnail_path.exists() {
                return Err(ImageRepositoryError::ImageDoesNotExist);
            }
            std::fs::remove_file(thumbnail_path)
                .map_err(|_| ImageRepositoryError::ImageRepositoryError)?;
        }
        Ok(())
    }

    fn get_image(&self, image_record: &ImageRecord) -> Result<DynamicImage, ImageRepositoryError> {
        let path = self.original_path(image_record.id, image_record.meta.format.as_ref().as_ref());
        if !path.exists() {
            return Err(ImageRepositoryError::ImageDoesNotExist);
        }
        image::open(&path).map_err(|_| ImageRepositoryError::ImageRepositoryError)
    }

    fn get_thumbnail(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, ImageRepositoryError> {
        let path = self.thumbnail_path(image_id, thumbnail_size)?;
        if !path.exists() {
            return Err(ImageRepositoryError::ImageDoesNotExist);
        }
        let image = image::open(&path).map_err(|_| ImageRepositoryError::ImageRepositoryError)?;
        let width = image.width();
        let height = image.height();
        let (width, height) = thumbnail_width_height(width, height, thumbnail_size);

        self.resize_service
            .resize(&image, width, height)
            .map_err(|_| ImageRepositoryError::ImageRepositoryError)
    }
}

fn ensure_dir(dir: &Path) -> Result<(), std::io::Error> {
    if !dir.exists() {
        create_dir_all(dir)?;
    }
    Ok(())
}

fn thumbnail_width_height(
    original_width: u32,
    original_height: u32,
    thumbnail_size: u32,
) -> (u32, u32) {
    if original_width > original_height {
        (
            thumbnail_size,
            ((original_height * thumbnail_size) as f32 / original_width as f32) as u32,
        )
    } else {
        (
            ((original_width * thumbnail_size) as f32 / original_height as f32) as u32,
            thumbnail_size,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::FSImageRepository;
    use image::GenericImageView;
    use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
    use photos_services::{ImageRepository, ImageRepositoryError};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn test_image_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("test_data")
            .join(name)
    }

    #[test]
    fn test_insert_get_delete() {
        let temp = tempdir().unwrap();
        let base = temp.path().to_path_buf();

        let thumbnail_sizes = vec![512];
        let resize_service = FastImageResizeResizer::default();
        let repo = FSImageRepository::new(base.clone(), thumbnail_sizes.clone(), resize_service);

        let source = test_image_path("example.jpeg");
        let record = repo.insert_image(&source).unwrap();
        let id_string = record.id.to_string();
        let id_string_split = id_string.split_at(2);

        let original_path = base
            .join("originals")
            .join(id_string_split.0)
            .join(id_string_split.1)
            .with_added_extension(record.meta.format.as_ref());

        assert!(original_path.exists());

        let thumbnail_path = base
            .join("thumbnails")
            .join("512")
            .join(id_string_split.0)
            .join(id_string_split.1)
            .with_added_extension(record.meta.format.as_ref());

        assert!(thumbnail_path.exists());

        let thumb = repo.get_thumbnail(&record.id, 512).unwrap();
        let (w, h) = thumb.dimensions();

        assert!(w == 512 || h == 512);

        repo.delete_image(&record).unwrap();

        assert!(matches!(
            repo.delete_image(&record),
            Err(ImageRepositoryError::ImageDoesNotExist)
        ))
    }
}
