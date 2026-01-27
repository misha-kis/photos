use image::ImageFormat;
use photos_core::Uuid;
use photos_domain::{
    BoundingBox, ClusteredFaceDetection, FaceDetection, FaceDetectionWithEmbedding, ImageId,
    ImageMeta, ImageRecord,
};
use photos_services::{ImageMetadataRepository, ImageMetadataRepositoryError};
use sqlx::FromRow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub trait IntoInternal<T> {
    fn internal(self) -> Result<T, ImageMetadataRepositoryError>;
}

impl<T, E> IntoInternal<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn internal(self) -> Result<T, ImageMetadataRepositoryError> {
        self.map_err(|e| ImageMetadataRepositoryError::Internal(Box::new(e)))
    }
}

pub struct SqliteImageMetadataRepository {
    pool: sqlx::SqlitePool,
}

impl SqliteImageMetadataRepository {
    pub async fn new(path: PathBuf) -> Result<Self, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite init");
        let db_path = path.join("db.sqlite");
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);
        tracing::debug!("sqlite connecting");
        let pool = sqlx::SqlitePool::connect_with(opts).await.internal()?;
        tracing::debug!("sqlite loading migrations");
        let migrator = sqlx::migrate::Migrator::new(Path::new(
            "./crates/photos-infra-sqlite-image-metadata-repository/migrations",
        ))
        .await
        .internal()?;
        tracing::debug!("sqlite migrating");
        migrator.run(&pool).await.internal()?;
        tracing::debug!("sqlite init done");
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl ImageMetadataRepository for SqliteImageMetadataRepository {
    async fn add_image_record(
        &self,
        image_record: &ImageRecord,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite inserting image record");
        let format_id = format_to_i64(image_record.meta.format)?;
        sqlx::query(r#"INSERT INTO image(uuid, format_id) VALUES (?, ?)"#)
            .bind(image_record.id)
            .bind(format_id)
            .execute(&self.pool)
            .await
            .internal()?;
        tracing::debug!("sqlite inserting image record done");
        Ok(())
    }

    async fn add_image_record_bulk(
        &self,
        image_records: &[ImageRecord],
    ) -> Result<(), ImageMetadataRepositoryError> {
        if image_records.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            "sqlite bulk inserting {} image records",
            image_records.len()
        );
        let mut tx = self.pool.begin().await.internal()?;

        for record in image_records {
            let format_id = format_to_i64(record.meta.format)?;
            sqlx::query(r#"INSERT INTO image(uuid, format_id) VALUES (?, ?)"#)
                .bind(record.id)
                .bind(format_id)
                .execute(&mut *tx)
                .await
                .internal()?;
        }

        tx.commit().await.internal()?;

        tracing::debug!("sqlite bulk insert done");
        Ok(())
    }

    async fn get_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<ImageRecord, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting image record for {}", image_id);
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
            format_id: i64,
        }

        let row = sqlx::query_as::<_, Row>(r#"SELECT uuid, format_id FROM image"#)
            .fetch_one(&self.pool)
            .await
            .internal()?;

        let format = i64_to_format(row.format_id)?;
        tracing::debug!("sqlite getting image ids done");
        Ok(ImageRecord {
            id: row.uuid,
            meta: ImageMeta { format },
        })
    }

    async fn delete_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite deleting image record for {}", image_id);
        let rows_affected = sqlx::query(r#"DELETE FROM image WHERE uuid = $1"#)
            .bind(image_id)
            .execute(&self.pool)
            .await
            .internal()?;

        if rows_affected.rows_affected() == 0 {
            return Err(ImageMetadataRepositoryError::QueryFailed {
                err: format!("Image with id {} not found", image_id),
            });
        }

        tracing::debug!("sqlite delete done");
        Ok(())
    }

    async fn get_image_ids(&self) -> Result<Vec<ImageId>, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting image ids");
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
        }

        let result = sqlx::query_as::<_, Row>(r#"SELECT uuid FROM image"#)
            .fetch_all(&self.pool)
            .await
            .internal()?
            .iter()
            .map(|row| row.uuid)
            .collect();
        tracing::debug!("sqlite getting image ids done");
        Ok(result)
    }

    async fn get_face_ids(&self) -> Result<Vec<Uuid>, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting face ids");
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
        }
        let result = sqlx::query_as::<_, Row>(r#"SELECT uuid FROM face"#)
            .fetch_all(&self.pool)
            .await
            .internal()?
            .iter()
            .map(|row| row.uuid)
            .collect();
        tracing::debug!("sqlite getting face ids done");
        Ok(result)
    }

    async fn get_number_of_images(&self) -> Result<u64, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting number of images");
        #[derive(FromRow)]
        struct Row {
            uuid_count: u64,
        }
        let result = sqlx::query_as::<_, Row>(r#"SELECT COUNT(uuid) AS uuid_count FROM image"#)
            .fetch_one(&self.pool)
            .await
            .map(|row| row.uuid_count)
            .internal();
        tracing::debug!("sqlite getting number of images done");
        result
    }

    async fn get_image_records_without_detections(
        &self,
    ) -> Result<Vec<ImageRecord>, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting image records without face detections");
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
            format_id: i64,
        }

        let result =
            sqlx::query_as::<_, Row>(r#"SELECT uuid, format_id FROM image WHERE is_analyzed = 0"#)
                .fetch_all(&self.pool)
                .await
                .internal()?
                .iter()
                .map(|row| {
                    if let Ok(format) = i64_to_format(row.format_id) {
                        Ok(ImageRecord {
                            id: row.uuid,
                            meta: ImageMeta { format },
                        })
                    } else {
                        tracing::error!("image with id {} has unsupported format", row.uuid);
                        Err(ImageMetadataRepositoryError::InvalidImageFormat)
                    }
                })
                .filter_map(|maybe_record| maybe_record.ok())
                .collect();
        tracing::debug!("sqlite getting image records without face detections done");
        Ok(result)
    }

    async fn add_detections_to_image(
        &self,
        image_id: &ImageId,
        face_detections: Vec<FaceDetection>,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite inserting detections");
        let mut tx = self.pool.begin().await.internal()?;

        for detection in face_detections {
            sqlx::query(r#"INSERT INTO face_detection(uuid, image_uuid, roi_x, roi_y, roi_w, roi_h, confidence) VALUES (?, ?, ?, ?, ?, ?, ?)"#)
                .bind(detection.uuid)
                .bind(image_id)
                .bind(detection.bounding_box.x)
                .bind(detection.bounding_box.y)
                .bind(detection.bounding_box.w)
                .bind(detection.bounding_box.h)
                .bind(detection.confidence)
                .execute(&mut *tx)
                .await
                .internal()?;
        }
        sqlx::query(r#"UPDATE image SET is_analyzed = 1 WHERE uuid = ?"#)
            .bind(image_id)
            .execute(&mut *tx)
            .await
            .internal()?;

        tx.commit().await.internal()?;

        tracing::debug!("sqlite inserting detections done");
        Ok(())
    }

    async fn get_detections_without_embeddings(
        &self,
    ) -> Result<Vec<(ImageRecord, FaceDetection)>, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting detections without embeddings");
        #[derive(FromRow)]
        struct Row {
            uuid: Uuid,
            image_uuid: ImageId,
            format_id: i64,
            roi_x: f32,
            roi_y: f32,
            roi_w: f32,
            roi_h: f32,
            confidence: f64,
        }

        let result = sqlx::query_as::<_, Row>(
            r#"
SELECT face_detection.uuid, image_uuid, format_id, roi_x, roi_y, roi_w, roi_h, confidence
FROM face_detection
JOIN image i on i.uuid = face_detection.image_uuid
WHERE embedding IS NULL
"#,
        )
        .fetch_all(&self.pool)
        .await
        .internal()?
        .iter()
        .map(|row| {
            if let Ok(format) = i64_to_format(row.format_id) {
                let image_record = ImageRecord {
                    id: row.image_uuid,
                    meta: ImageMeta { format },
                };
                let detection = FaceDetection {
                    uuid: row.uuid,
                    bounding_box: BoundingBox {
                        x: row.roi_x,
                        y: row.roi_y,
                        w: row.roi_w,
                        h: row.roi_h,
                    },
                    confidence: row.confidence as f32,
                };
                Ok((image_record, detection))
            } else {
                tracing::error!("image with id {} has unsupported format", row.image_uuid);
                Err(ImageMetadataRepositoryError::InvalidImageFormat)
            }
        })
        .filter_map(|maybe_record| maybe_record.ok())
        .collect();
        tracing::debug!("sqlite getting detections without embeddings done");

        Ok(result)
    }

    async fn update_face_detection_with_embedding(
        &self,
        face_detection_with_embedding: FaceDetectionWithEmbedding,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite udpating detection with embedding");
        let bytes: &[u8] = bytemuck::cast_slice(&face_detection_with_embedding.embedding);
        sqlx::query(
            r#"
UPDATE face_detection
SET embedding = ?
WHERE uuid = ?
"#,
        )
        .bind(bytes)
        .bind(face_detection_with_embedding.detection.uuid)
        .execute(&self.pool)
        .await
        .internal()?;
        tracing::debug!("sqlite udpating detection with embedding done");
        Ok(())
    }
    async fn get_detections_with_embeddings(
        &self,
    ) -> Result<Vec<FaceDetectionWithEmbedding>, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting detections with embeddings");
        #[derive(FromRow)]
        struct Row {
            uuid: Uuid,
            roi_x: f32,
            roi_y: f32,
            roi_w: f32,
            roi_h: f32,
            confidence: f64,
            embedding: Vec<u8>,
        }

        let result = sqlx::query_as::<_, Row>(
            r#"
SELECT uuid, roi_x, roi_y, roi_w, roi_h, confidence, embedding
FROM face_detection
WHERE embedding IS NOT NULL
"#,
        )
        .fetch_all(&self.pool)
        .await
        .internal()?
        .iter()
        .map(|row| FaceDetectionWithEmbedding {
            detection: FaceDetection {
                uuid: row.uuid,
                bounding_box: BoundingBox {
                    x: row.roi_x,
                    y: row.roi_y,
                    w: row.roi_w,
                    h: row.roi_h,
                },
                confidence: row.confidence as f32,
            },
            embedding: bytemuck::cast_slice(&row.embedding).try_into().unwrap(),
        })
        .collect();
        tracing::debug!("sqlite getting detections with embeddings done");

        Ok(result)
    }

    async fn update_detections_with_clusters(
        &self,
        clustered_face_detections: &[ClusteredFaceDetection],
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite updating clusters");
        let mut tx = self.pool.begin().await.internal()?;

        let unique_cluster_ids: HashSet<u32> = clustered_face_detections
            .iter()
            .filter_map(|d| d.cluster_id)
            .collect();
        let cluster_uuids: HashMap<u32, Uuid> = unique_cluster_ids
            .into_iter()
            .map(|id| (id, Uuid::now_v7()))
            .collect();

        for detection in clustered_face_detections {
            let cluster_uuid = if let Some(cluster_id) = detection.cluster_id {
                cluster_uuids[&cluster_id]
            } else {
                Uuid::nil()
            };
            sqlx::query(
                r#"
INSERT INTO face (uuid) VALUES (?) ON CONFLICT DO NOTHING;
UPDATE face_detection SET face_uuid = ? WHERE uuid = ?
"#,
            )
            .bind(cluster_uuid)
            .bind(cluster_uuid)
            .bind(detection.detection.detection.uuid)
            .execute(&mut *tx)
            .await
            .internal()?;
        }
        tx.commit().await.internal()?;
        tracing::debug!("sqlite updating clusters done");
        Ok(())
    }

    async fn get_min_detection_bbox_and_image_for_face_id(
        &self,
        face_id: Uuid,
    ) -> Result<(BoundingBox, ImageRecord), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting min detection for face id");
        #[derive(FromRow)]
        struct Row {
            roi_x: f32,
            roi_y: f32,
            roi_w: f32,
            roi_h: f32,
            image_uuid: ImageId,
            format_id: i64,
        }
        let row = sqlx::query_as::<_, Row>(
            r#"
SELECT roi_x, roi_y, roi_w, roi_h, image_uuid, format_id
FROM (SELECT min(uuid) uuid
      FROM face_detection
      WHERE face_uuid = ?) min_uuid
         JOIN face_detection fd ON fd.uuid = min_uuid.uuid
         JOIN image i ON fd.image_uuid = i.uuid
"#,
        )
        .bind(face_id)
        .fetch_one(&self.pool)
        .await
        .internal()?;

        let format = i64_to_format(row.format_id)?;
        let result = (
            BoundingBox {
                x: row.roi_x,
                y: row.roi_y,
                w: row.roi_w,
                h: row.roi_h,
            },
            ImageRecord {
                id: row.image_uuid,
                meta: ImageMeta { format },
            },
        );

        tracing::debug!("sqlite getting min detection for face id");
        Ok(result)
    }
}

fn format_to_i64(format: ImageFormat) -> Result<i64, ImageMetadataRepositoryError> {
    match format {
        ImageFormat::Png => Ok(0),
        ImageFormat::Jpeg => Ok(1),
        ImageFormat::Gif => Ok(2),
        ImageFormat::WebP => Ok(3),
        ImageFormat::Pnm => Ok(4),
        ImageFormat::Tiff => Ok(5),
        ImageFormat::Tga => Ok(6),
        ImageFormat::Dds => Ok(7),
        ImageFormat::Bmp => Ok(8),
        ImageFormat::Ico => Ok(9),
        ImageFormat::Hdr => Ok(10),
        ImageFormat::OpenExr => Ok(11),
        ImageFormat::Farbfeld => Ok(12),
        ImageFormat::Avif => Ok(13),
        ImageFormat::Qoi => Ok(14),
        _ => Err(ImageMetadataRepositoryError::InvalidImageFormat),
    }
}

fn i64_to_format(format_id: i64) -> Result<ImageFormat, ImageMetadataRepositoryError> {
    match format_id {
        0 => Ok(ImageFormat::Png),
        1 => Ok(ImageFormat::Jpeg),
        2 => Ok(ImageFormat::Gif),
        3 => Ok(ImageFormat::WebP),
        4 => Ok(ImageFormat::Pnm),
        5 => Ok(ImageFormat::Tiff),
        6 => Ok(ImageFormat::Tga),
        7 => Ok(ImageFormat::Dds),
        8 => Ok(ImageFormat::Bmp),
        9 => Ok(ImageFormat::Ico),
        10 => Ok(ImageFormat::Hdr),
        11 => Ok(ImageFormat::OpenExr),
        12 => Ok(ImageFormat::Farbfeld),
        13 => Ok(ImageFormat::Avif),
        14 => Ok(ImageFormat::Qoi),
        _ => Err(ImageMetadataRepositoryError::InvalidImageFormat),
    }
}
