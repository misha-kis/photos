use chrono::{DateTime, Utc};
use image::ImageFormat;
use photos_domain::{
    BoundingBox, ClusteredFaceDetection, FaceDetection, FaceDetectionWithEmbedding, ImageId,
    ImageRecord, Timestamps, Uuid,
};
use photos_services::{ImageMetadataRepository, ImageMetadataRepositoryError};
use sqlx::FromRow;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub trait IntoInternal<T> {
    fn internal(self) -> Result<T, ImageMetadataRepositoryError>;
}

impl<T, E> IntoInternal<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn internal(self) -> Result<T, ImageMetadataRepositoryError> {
        self.map_err(|e| {
            tracing::error!("{}", e.to_string());
            ImageMetadataRepositoryError::Internal(Box::new(e))
        })
    }
}

pub struct SqliteImageMetadataRepository {
    pool: sqlx::SqlitePool,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ImageRecordRow {
    pub image_uuid: ImageId,
    pub image_format_id: i64,
    pub image_exif_timestamp: Option<DateTime<Utc>>,
    pub image_os_timestamp: DateTime<Utc>,
    pub image_import_timestamp: DateTime<Utc>,
}

impl From<ImageRecordRow> for ImageRecord {
    fn from(row: ImageRecordRow) -> Self {
        Self {
            id: row.image_uuid,
            format: i64_to_format(row.image_format_id),
            timestamps: Timestamps {
                exif_timestamp: row.image_exif_timestamp,
                os_timestamp: row.image_os_timestamp,
                import_timestamp: row.image_import_timestamp,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct BoundingBoxRow {
    pub fd_roi_x: f32,
    pub fd_roi_y: f32,
    pub fd_roi_w: f32,
    pub fd_roi_h: f32,
}

impl From<BoundingBoxRow> for BoundingBox {
    fn from(row: BoundingBoxRow) -> Self {
        BoundingBox {
            x: row.fd_roi_x,
            y: row.fd_roi_y,
            w: row.fd_roi_w,
            h: row.fd_roi_h,
        }
    }
}

#[derive(Debug, Clone, PartialEq, FromRow)]
pub struct FaceDetectionRow {
    pub fd_uuid: Uuid,
    #[sqlx(flatten)]
    pub bounding_box: BoundingBoxRow,
    pub fd_confidence: f32,
}

impl From<FaceDetectionRow> for FaceDetection {
    fn from(row: FaceDetectionRow) -> Self {
        FaceDetection {
            uuid: row.fd_uuid,
            bounding_box: row.bounding_box.into(),
            confidence: row.fd_confidence,
        }
    }
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
        tracing::debug!("sqlite migrating");
        MIGRATOR.run(&pool).await.internal()?;
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
        let format_id = format_to_i64(image_record.format);
        sqlx::query(r#"INSERT INTO image(image_uuid, image_format_id, image_exif_timestamp, image_os_timestamp, image_import_timestamp) VALUES (?, ?, ?, ?, ?)"#)
            .bind(image_record.id)
            .bind(format_id)
            .bind(image_record.timestamps.exif_timestamp)
            .bind(image_record.timestamps.os_timestamp)
            .bind(image_record.timestamps.import_timestamp)
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

        for image_record in image_records {
            let format_id = format_to_i64(image_record.format);
            sqlx::query(r#"INSERT INTO image(image_uuid, image_format_id, image_exif_timestamp, image_os_timestamp, image_import_timestamp) VALUES (?, ?, ?, ?, ?)"#)
            .bind(image_record.id)
            .bind(format_id)
            .bind(image_record.timestamps.exif_timestamp)
            .bind(image_record.timestamps.os_timestamp)
            .bind(image_record.timestamps.import_timestamp)
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

        let row = sqlx::query_as::<_, ImageRecordRow>(
            r#"
SELECT image_uuid, image_format_id, image_exif_timestamp, image_os_timestamp, image_import_timestamp
FROM image
WHERE image_uuid = ?
"#,
        )
        .bind(image_id)
        .fetch_one(&self.pool)
        .await
        .internal()?;

        tracing::debug!("sqlite getting image ids done");
        Ok(row.into())
    }

    async fn delete_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite deleting image record for {}", image_id);
        let rows_affected = sqlx::query(r#"DELETE FROM image WHERE image_uuid = $1"#)
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
            image_uuid: ImageId,
        }

        let result = sqlx::query_as::<_, Row>(
            r#"SELECT image_uuid FROM image ORDER BY coalesce(image_exif_timestamp, image_os_timestamp)"#,
        )
        .fetch_all(&self.pool)
        .await
        .internal()?
        .iter()
        .map(|row| row.image_uuid)
        .collect();
        tracing::debug!("sqlite getting image ids done");
        Ok(result)
    }

    async fn get_face_clusters(
        &self,
    ) -> Result<Vec<(Uuid, Vec<Uuid>)>, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting face clusters");
        #[derive(FromRow)]
        struct Row {
            face_uuid: Uuid,
            fd_uuid: Uuid,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"
SELECT f.face_uuid, fd.fd_uuid
FROM face f
JOIN face_detection fd ON fd.face_uuid = f.face_uuid
WHERE f.face_uuid IS NOT NULL
ORDER BY f.face_uuid, fd.fd_uuid
"#,
        )
        .fetch_all(&self.pool)
        .await
        .internal()?;
        let mut clusters: Vec<(Uuid, Vec<Uuid>)> = Vec::new();
        let mut current_face: Option<Uuid> = None;
        for row in rows {
            if current_face != Some(row.face_uuid) {
                current_face = Some(row.face_uuid);
                clusters.push((row.face_uuid, vec![row.fd_uuid]));
            } else {
                clusters.last_mut().unwrap().1.push(row.fd_uuid);
            }
        }
        tracing::debug!("sqlite getting face clusters done");
        Ok(clusters)
    }

    async fn get_number_of_images(&self) -> Result<u64, ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting number of images");
        #[derive(FromRow)]
        struct Row {
            uuid_count: u64,
        }
        let result =
            sqlx::query_as::<_, Row>(r#"SELECT COUNT(image_uuid) AS uuid_count FROM image"#)
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

        let result =
            sqlx::query_as::<_, ImageRecordRow>(r#"SELECT image_uuid, image_format_id, image_exif_timestamp, image_os_timestamp, image_import_timestamp FROM image WHERE image_is_analyzed = 0"#)
                .fetch_all(&self.pool)
                .await
                .internal()?
                .into_iter()
                .map(|row| row.into())
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
            let result = sqlx::query(r#"INSERT INTO face_detection(fd_uuid, image_uuid, fd_roi_x, fd_roi_y, fd_roi_w, fd_roi_h, fd_confidence) VALUES (?, ?, ?, ?, ?, ?, ?)"#)
                .bind(detection.uuid)
                .bind(image_id)
                .bind(detection.bounding_box.x)
                .bind(detection.bounding_box.y)
                .bind(detection.bounding_box.w)
                .bind(detection.bounding_box.h)
                .bind(detection.confidence)
                .execute(&mut *tx)
                .await
                .internal();
            if let Err(e) = result {
                tracing::warn!("bad query: {e:?}");
            }
        }
        sqlx::query(r#"UPDATE image SET image_is_analyzed = 1 WHERE image_uuid = ?"#)
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
        #[derive(FromRow, Debug)]
        struct Row {
            #[sqlx(flatten)]
            image_record_row: ImageRecordRow,
            #[sqlx(flatten)]
            face_detection: FaceDetectionRow,
        }

        let result = sqlx::query_as::<_, Row>(
            r#"
SELECT i.image_uuid, image_format_id, image_exif_timestamp, image_os_timestamp, image_import_timestamp, fd_uuid, fd_roi_x, fd_roi_y, fd_roi_w, fd_roi_h, fd_confidence
FROM face_detection f
JOIN image i on i.image_uuid = f.image_uuid
WHERE fd_embedding IS NULL
"#,
        )
        .fetch_all(&self.pool)
        .await
        .internal()?
        .into_iter()
        .map(|row| {
            tracing::debug!("{row:?}");
            (row.image_record_row.into(), row.face_detection.into()) })
        .collect();
        tracing::debug!("sqlite getting detections without embeddings done");

        Ok(result)
    }

    async fn update_face_detection_with_embedding(
        &self,
        face_detection_with_embedding: FaceDetectionWithEmbedding,
    ) -> Result<(), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite updating detection with embedding");
        let bytes: &[u8] = bytemuck::cast_slice(&face_detection_with_embedding.embedding);
        sqlx::query(
            r#"
UPDATE face_detection
SET fd_embedding = ?
WHERE fd_uuid = ?
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
            #[sqlx(flatten)]
            face_detection: FaceDetectionRow,
            fd_embedding: Vec<u8>,
        }

        let result = sqlx::query_as::<_, Row>(
            r#"
SELECT fd_uuid, fd_roi_x, fd_roi_y, fd_roi_w, fd_roi_h, fd_confidence, fd_embedding
FROM face_detection
WHERE fd_embedding IS NOT NULL
"#,
        )
        .fetch_all(&self.pool)
        .await
        .internal()?
        .into_iter()
        .map(|row| FaceDetectionWithEmbedding {
            detection: row.face_detection.into(),
            embedding: bytemuck::cast_slice(&row.fd_embedding).try_into().unwrap(),
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
            let cluster_uuid = detection.cluster_id.map(|id| cluster_uuids[&id]);
            sqlx::query(
                r#"
INSERT INTO face (face_uuid) VALUES (?) ON CONFLICT DO NOTHING;
UPDATE face_detection SET face_uuid = ? WHERE fd_uuid = ?
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

    async fn get_bbox_and_image_for_detection_id(
        &self,
        detection_id: Uuid,
    ) -> Result<(BoundingBox, ImageRecord), ImageMetadataRepositoryError> {
        tracing::debug!("sqlite getting bbox and image for detection id {detection_id:?}");
        #[derive(FromRow)]
        struct Row {
            #[sqlx(flatten)]
            image_record_row: ImageRecordRow,
            #[sqlx(flatten)]
            bounding_box: BoundingBoxRow,
        }
        let row = sqlx::query_as::<_, Row>(
            r#"
SELECT i.image_uuid, image_format_id, image_exif_timestamp, image_os_timestamp, image_import_timestamp, fd_roi_x, fd_roi_y, fd_roi_w, fd_roi_h
FROM face_detection fd
JOIN image i ON fd.image_uuid = i.image_uuid
WHERE fd.fd_uuid = ?
"#,
        )
        .bind(detection_id)
        .fetch_one(&self.pool)
        .await
        .internal()?;
        tracing::debug!("sqlite getting bbox and image for detection id done");
        Ok((row.bounding_box.into(), row.image_record_row.into()))
    }
}

fn format_to_i64(format: ImageFormat) -> i64 {
    match format {
        ImageFormat::Png => 0,
        ImageFormat::Jpeg => 1,
        ImageFormat::Gif => 2,
        ImageFormat::WebP => 3,
        ImageFormat::Pnm => 4,
        ImageFormat::Tiff => 5,
        ImageFormat::Tga => 6,
        ImageFormat::Dds => 7,
        ImageFormat::Bmp => 8,
        ImageFormat::Ico => 9,
        ImageFormat::Hdr => 10,
        ImageFormat::OpenExr => 11,
        ImageFormat::Farbfeld => 12,
        ImageFormat::Avif => 13,
        ImageFormat::Qoi => 14,
        _ => unreachable!(),
    }
}

fn i64_to_format(format_id: i64) -> ImageFormat {
    match format_id {
        0 => ImageFormat::Png,
        1 => ImageFormat::Jpeg,
        2 => ImageFormat::Gif,
        3 => ImageFormat::WebP,
        4 => ImageFormat::Pnm,
        5 => ImageFormat::Tiff,
        6 => ImageFormat::Tga,
        7 => ImageFormat::Dds,
        8 => ImageFormat::Bmp,
        9 => ImageFormat::Ico,
        10 => ImageFormat::Hdr,
        11 => ImageFormat::OpenExr,
        12 => ImageFormat::Farbfeld,
        13 => ImageFormat::Avif,
        14 => ImageFormat::Qoi,
        _ => unreachable!(),
    }
}
