use chrono::{NaiveDateTime, Utc};
use exif::{Reader, Tag};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageEncoder, ImageReader};
use photos_domain::{BoundingBox, ImageId, ImageRecord, Timestamps, Uuid};
use photos_services::{ImageRepository, ImageRepositoryError, ResizeService};
use std::fs::{self, File, copy, create_dir_all};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

pub trait IntoInternal<T> {
    fn internal(self) -> Result<T, ImageRepositoryError>;
}

impl<T, E> IntoInternal<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn internal(self) -> Result<T, ImageRepositoryError> {
        self.map_err(|e| ImageRepositoryError::Internal(Box::new(e)))
    }
}

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
        tracing::debug!("getting original paths for {image_id}");
        let (bucket, filename) = image_path_parts(image_id, extension);
        self.path.join("originals").join(bucket).join(filename)
    }

    fn thumbnail_paths(&self, image_id: ImageId) -> Vec<PathBuf> {
        tracing::debug!("getting thumbnail paths for {image_id}");
        let thumbnails_path = self.path.join("thumbnails");
        let (bucket, filename) = image_path_parts(image_id, "jpeg");
        self.thumbnail_sizes
            .iter()
            .map(|thumbnail_size| {
                thumbnails_path
                    .join(thumbnail_size.to_string())
                    .join(&bucket)
                    .join(&filename)
            })
            .collect()
    }
}

impl<T: ResizeService> ImageRepository for FSImageRepository<T> {
    fn insert_image(&self, image_path: &Path) -> Result<ImageRecord, ImageRepositoryError> {
        tracing::info!("inserting image from path {:?}", image_path);
        let image_id = ImageId::now_v7();

        tracing::debug!("opening image");
        let reader = ImageReader::open(image_path)
            .internal()?
            .with_guessed_format()
            .internal()?;
        let format = reader.format().ok_or(ImageRepositoryError::ImageError {
            err: "no format".to_string(),
        })?;
        let image = reader.decode().internal()?;

        let original_path = self.original_path(image_id, format.extensions_str()[0]);
        ensure_dir(original_path.parent().expect("parent dir exists")).internal()?;

        tracing::debug!("copying original image");
        let orientation = read_orientation(image_path);
        let image = match &orientation {
            None | Some(1) => {
                copy(image_path, &original_path).internal()?;
                image
            }
            Some(orientation) => {
                let image = apply_orientation(image, *orientation);
                let file = File::create(image_path).internal()?;
                let mut writer = BufWriter::new(file);
                image.write_to(&mut writer, format).internal()?;
                image
            }
        };
        tracing::debug!("done copying original image");

        let thumbnail_paths = self.thumbnail_paths(image_id);
        let image = match read_orientation(image_path) {
            None => image,
            Some(orientation) => apply_orientation(image, orientation),
        };
        let width = image.width();
        let height = image.height();

        for (&thumbnail_size, thumbnail_path) in self.thumbnail_sizes.iter().zip(thumbnail_paths) {
            tracing::debug!("resizing image");
            let (width, height) = thumbnail_width_height(width, height, thumbnail_size);

            let resized_image = self
                .resize_service
                .resize(&image, width, height)
                .internal()?;

            ensure_dir(thumbnail_path.parent().expect("parent dir exists")).internal()?;

            let mut out_file = std::fs::File::create(&thumbnail_path).internal()?;

            tracing::debug!("writing resized image");
            let rgb_image = resized_image.to_rgb8();
            JpegEncoder::new(&mut out_file)
                .write_image(&rgb_image, width, height, image::ExtendedColorType::Rgb8)
                .internal()?;
        }
        let timestamps = read_timestamps_with_import_timestamp(&original_path, Utc::now())
            .ok_or(ImageRepositoryError::FailedToReadTimestamps)?;

        Ok(ImageRecord {
            id: image_id,
            format,
            timestamps,
        })
    }

    fn delete_image(&self, image_record: &ImageRecord) -> Result<(), ImageRepositoryError> {
        tracing::info!("deleting image");
        let original_path =
            self.original_path(image_record.id, image_record.format.extensions_str()[0]);
        if !original_path.exists() {
            return Err(ImageRepositoryError::ImageDoesNotExist);
        }
        std::fs::remove_file(original_path).internal()?;
        for thumbnail_path in self.thumbnail_paths(image_record.id) {
            if !thumbnail_path.exists() {
                return Err(ImageRepositoryError::ImageDoesNotExist);
            }
            std::fs::remove_file(thumbnail_path).internal()?;
        }
        Ok(())
    }

    fn get_image(
        &self,
        image_record: &ImageRecord,
        resize: Option<(u32, u32)>,
    ) -> Result<DynamicImage, ImageRepositoryError> {
        // todo(static assert size >= 0)
        tracing::info!("getting image {:?}", image_record.id);
        let path = self.get_original_path(image_record);
        if !path.exists() {
            return Err(ImageRepositoryError::ImageDoesNotExist);
        }
        let image = image::open(&path).internal()?;
        let image = match read_orientation(&path) {
            None => image,
            Some(orientation) => apply_orientation(image, orientation),
        };

        if let Some(size) = resize {
            self.resize_service
                .resize(&image, size.0, size.1)
                .internal()
        } else {
            Ok(image)
        }
    }

    fn get_face_thumbnail(
        &self,
        image_record: &ImageRecord,
        bounding_box: BoundingBox,
        thumbnail_size: u32,
    ) -> Result<DynamicImage, ImageRepositoryError> {
        let image = self.get_image(image_record, None).internal()?;
        let image = image.crop_imm(
            bounding_box.x as u32,
            bounding_box.y as u32,
            bounding_box.w as u32,
            bounding_box.h as u32,
        );
        self.resize_service
            .resize(&image, thumbnail_size, thumbnail_size)
            .internal()
    }

    fn save_face_thumbnail(
        &self,
        cluster_id: Uuid,
        thumbnail: &DynamicImage,
    ) -> Result<PathBuf, ImageRepositoryError> {
        let path = self.get_face_thumbnail_path(cluster_id);
        if let Some(parent) = path.parent() {
            ensure_dir(parent).internal()?;
        }
        let file = File::create(&path).internal()?;
        let mut writer = BufWriter::new(file);
        let encoder = JpegEncoder::new_with_quality(&mut writer, 85);
        thumbnail.write_with_encoder(encoder).internal()?;
        Ok(path)
    }

    fn get_face_thumbnail_path(&self, cluster_id: Uuid) -> PathBuf {
        self.path
            .join("face_thumbnails")
            .join(cluster_id.to_string())
            .with_added_extension("jpeg")
    }

    fn get_thumbnail_path(
        &self,
        image_id: &ImageId,
        thumbnail_size: u32,
    ) -> Result<PathBuf, ImageRepositoryError> {
        if !self.thumbnail_sizes.contains(&thumbnail_size) {
            tracing::error!("thumbnail_path: invalid thumbnail size {thumbnail_size}");
            Err(ImageRepositoryError::InvalidThumbnailSize)
        } else {
            let (bucket, filename) = image_path_parts(*image_id, "jpeg");
            let path = self
                .path
                .join("thumbnails")
                .join(thumbnail_size.to_string())
                .join(bucket)
                .join(filename);
            tracing::debug!("thumbnail_path: {image_id}@{thumbnail_size}px: {:?}", path);
            Ok(path)
        }
    }

    fn get_original_path(&self, image_record: &ImageRecord) -> PathBuf {
        self.original_path(image_record.id, image_record.format.extensions_str()[0])
    }
}

fn image_path_parts(image_id: ImageId, extension: &str) -> (String, String) {
    let image_id = image_id.to_string();
    let bucket = image_id[image_id.len() - 2..].to_string();
    (bucket, image_id + "." + extension)
}

fn read_timestamps_with_import_timestamp(
    path: &Path,
    import_timestamp: chrono::DateTime<Utc>,
) -> Option<Timestamps> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(file);

    let exif_timestamp = Reader::new()
        .read_from_container(&mut bufreader)
        .ok()
        .and_then(|exif| {
            exif.get_field(Tag::DateTimeOriginal, exif::In::PRIMARY)
                .or_else(|| exif.get_field(Tag::DateTimeDigitized, exif::In::PRIMARY))
                .or_else(|| exif.get_field(Tag::DateTime, exif::In::PRIMARY))
                .map(|f| {
                    let raw = f.display_value().to_string();
                    NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M:%S")
                        .unwrap()
                        .and_utc()
                })
        });
    let meta = fs::metadata(path).ok()?;
    let os_timestamp = meta
        .created()
        .ok()
        .or_else(|| meta.modified().ok())
        .map(|date| date.into())?;

    Some(Timestamps {
        exif_timestamp,
        os_timestamp,
        import_timestamp,
    })
}

fn read_orientation(path: &Path) -> Option<u32> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(file);
    let exif = Reader::new().read_from_container(&mut bufreader).ok()?;

    exif.get_field(Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
}

fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        1 => img,             // normal
        2 => img.fliph(),     // mirror horizontal
        3 => img.rotate180(), // rotate 180
        4 => img.flipv(),     // mirror vertical
        5 => img.rotate90().fliph(),
        6 => img.rotate90(), // rotate 90 CW
        7 => img.rotate270().fliph(),
        8 => img.rotate270(), // rotate 270 CW
        _ => img,
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
