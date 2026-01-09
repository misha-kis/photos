use photos_core::Uuid;
use photos_domain::{
    BoundingBox, ClusteredFaceDetection, FaceDetection, FaceDetectionWithEmbedding, ImageFormat,
    ImageId, ImageMeta, ImageRecord,
};
use photos_services::{ImageMetadataRepository, ImageMetadataRepositoryError};
use sqlx::FromRow;
use std::path::{Path, PathBuf};

pub struct SqliteImageMetadataRepository {
    pool: sqlx::SqlitePool,
}

impl SqliteImageMetadataRepository {
    pub async fn new(path: PathBuf) -> Result<Self, ImageMetadataRepositoryError> {
        tracing::info!("sqlite init");
        let db_path = path.join("db.sqlite");
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);
        tracing::debug!("sqlite connecting");
        let pool = sqlx::SqlitePool::connect_with(opts)
            .await
            .map_err(|_| ImageMetadataRepositoryError::CannotConnectOrCreate)?;
        tracing::debug!("sqlite loading migrations");
        let migrator = sqlx::migrate::Migrator::new(Path::new(
            "./crates/photos-infra-sqlite-image-metadata-repository/migrations",
        ))
        .await
        .map_err(|_| ImageMetadataRepositoryError::CannotConnectOrCreate)?;
        tracing::debug!("sqlite migrating");
        migrator
            .run(&pool)
            .await
            .map_err(|_| ImageMetadataRepositoryError::CannotConnectOrCreate)?;
        tracing::info!("sqlite init done");
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl ImageMetadataRepository for SqliteImageMetadataRepository {
    async fn add_image_record(
        &self,
        image_record: &ImageRecord,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::info!("sqlite inserting image record");
        sqlx::query(r#"INSERT INTO image(uuid, format_id) VALUES (?, ?)"#)
            .bind(image_record.id)
            .bind(image_record.meta.format.as_u8())
            .execute(&self.pool)
            .await
            .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)?;
        tracing::info!("sqlite inserting image record done");
        Ok(())
    }

    async fn add_image_record_bulk(
        &self,
        image_records: &[ImageRecord],
    ) -> Result<(), ImageMetadataRepositoryError> {
        if image_records.is_empty() {
            return Ok(());
        }

        tracing::info!(
            "sqlite bulk inserting {} image records",
            image_records.len()
        );
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        for record in image_records {
            sqlx::query(r#"INSERT INTO image(uuid, format_id) VALUES (?, ?)"#)
                .bind(record.id)
                .bind(record.meta.format.as_u8())
                .execute(&mut *tx)
                .await
                .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;
        }

        tx.commit()
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        tracing::info!("sqlite bulk insert done");
        Ok(())
    }

    async fn get_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<ImageRecord, ImageMetadataRepositoryError> {
        tracing::info!("sqlite getting image record for {}", image_id);
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
            format_id: i64,
        }

        let row = sqlx::query_as::<_, Row>(r#"SELECT uuid, format_id FROM image"#)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        let format = ImageFormat::try_from(row.format_id as u8)
            .map_err(|_| ImageMetadataRepositoryError::InvalidImageFormat)?;
        tracing::info!("sqlite getting image ids done");
        Ok(ImageRecord {
            id: row.uuid,
            meta: ImageMeta { format },
        })
    }

    async fn delete_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::info!("sqlite deleting image record for {}", image_id);
        let rows_affected = sqlx::query(r#"DELETE FROM image WHERE uuid = $1"#)
            .bind(image_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        if rows_affected.rows_affected() == 0 {
            return Err(ImageMetadataRepositoryError::QueryFailed {
                err: format!("Image with id {} not found", image_id),
            });
        }

        tracing::info!("sqlite delete done");
        Ok(())
    }

    async fn get_image_ids(&self) -> Result<Vec<ImageId>, ImageMetadataRepositoryError> {
        tracing::info!("sqlite getting image ids");
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
        }

        let result = sqlx::query_as::<_, Row>(r#"SELECT uuid FROM image"#)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?
            .iter()
            .map(|row| row.uuid)
            .collect();
        tracing::info!("sqlite getting image ids done");
        Ok(result)
    }

    async fn get_number_of_images(&self) -> Result<u64, ImageMetadataRepositoryError> {
        tracing::info!("sqlite getting number of images");
        #[derive(FromRow)]
        struct Row {
            uuid_count: u64,
        }
        let result = sqlx::query_as::<_, Row>(r#"SELECT COUNT(uuid) AS uuid_count FROM image"#)
            .fetch_one(&self.pool)
            .await
            .map(|row| row.uuid_count)
            .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError);
        tracing::info!("sqlite getting number of images done");
        result
    }

    async fn get_image_records_without_detections(
        &self,
    ) -> Result<Vec<ImageRecord>, ImageMetadataRepositoryError> {
        tracing::info!("sqlite getting image records without face detections");
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
            format_id: i64,
        }

        let result =
            sqlx::query_as::<_, Row>(r#"SELECT uuid, format_id FROM image WHERE is_analyzed = 0"#)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?
                .iter()
                .map(|row| {
                    if let Ok(format) = ImageFormat::try_from(row.format_id as u8) {
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
        tracing::info!("sqlite getting image records without face detections done");
        Ok(result)
    }

    async fn add_detections_to_image(
        &self,
        image_id: &ImageId,
        face_detections: Vec<FaceDetection>,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::info!("sqlite inserting detections");
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

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
                .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;
        }
        sqlx::query(r#"UPDATE image SET is_analyzed = 1 WHERE uuid = ?"#)
            .bind(image_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        tx.commit()
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        tracing::info!("sqlite inserting detections done");
        Ok(())
    }

    async fn get_detections_without_embeddings(
        &self,
    ) -> Result<Vec<(ImageRecord, FaceDetection)>, ImageMetadataRepositoryError> {
        tracing::info!("sqlite getting detections without embeddings");
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
        .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?
        .iter()
        .map(|row| {
            if let Ok(format) = ImageFormat::try_from(row.format_id as u8) {
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
        tracing::info!("sqlite getting detections without embeddings done");

        Ok(result)
    }

    async fn update_face_detection_with_embedding(
        &self,
        face_detection_with_embedding: FaceDetectionWithEmbedding,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::info!("sqlite udpating detection with embedding");
        let bytes: &[u8] = bytemuck::cast_slice(&face_detection_with_embedding.embedding);
        sqlx::query(
            r#"
UPDATE face_detection
SET embedding = ?
WHERE roi_x = ? AND roi_y = ? AND roi_w = ? AND roi_h = ? AND confidence = ?
"#,
        )
        .bind(bytes)
        .bind(face_detection_with_embedding.detection.bounding_box.x)
        .bind(face_detection_with_embedding.detection.bounding_box.y)
        .bind(face_detection_with_embedding.detection.bounding_box.w)
        .bind(face_detection_with_embedding.detection.bounding_box.h)
        .bind(face_detection_with_embedding.detection.confidence)
        .execute(&self.pool)
        .await
        .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)?;
        tracing::info!("sqlite udpating detection with embedding done");
        Ok(())
    }
    async fn get_detections_with_embeddings(
        &self,
    ) -> Result<Vec<FaceDetectionWithEmbedding>, ImageMetadataRepositoryError> {
        tracing::info!("sqlite getting detections with embeddings");
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
        .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?
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
        tracing::info!("sqlite getting detections with embeddings done");

        Ok(result)
    }

    async fn update_detections_with_clusters(
        &self,
        clustered_face_detections: &[ClusteredFaceDetection],
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::info!("sqlite updating clusters");
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        for detection in clustered_face_detections {
            let cluster_id = if let Some(cluster_id) = detection.cluster_id {
                cluster_id as i32
            } else {
                -1
            };
            sqlx::query(r#"UPDATE face_detection SET face_uuid = ? WHERE uuid = ?"#)
                .bind(cluster_id)
                .bind(detection.detection.detection.uuid)
                .execute(&mut *tx)
                .await
                .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)?;
        }
        tx.commit()
            .await
            .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)?;
        tracing::info!("sqlite updating clusters done");
        Ok(())
    }
}
