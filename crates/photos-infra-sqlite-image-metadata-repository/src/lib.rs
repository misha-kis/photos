use photos_domain::{ImageId, ImageRecord};
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
        sqlx::query(r#"INSERT INTO image(uuid) VALUES ($1)"#)
            .bind(image_record.id)
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
            sqlx::query(r#"INSERT INTO image(uuid) VALUES ($1)"#)
                .bind(record.id)
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
        let exists =
            sqlx::query_scalar::<_, bool>(r#"SELECT EXISTS(SELECT 1 FROM image WHERE uuid = $1)"#)
                .bind(image_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?;

        if exists {
            // Note: The database only stores UUIDs, not full metadata.
            // Full ImageRecord with metadata should be retrieved via ImageRepository.
            Err(ImageMetadataRepositoryError::QueryFailed {
                err: format!(
                    "Image metadata not stored in database for id {}. Use ImageRepository to get full record.",
                    image_id
                ),
            })
        } else {
            Err(ImageMetadataRepositoryError::QueryFailed {
                err: format!("Image with id {} not found", image_id),
            })
        }
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
}
