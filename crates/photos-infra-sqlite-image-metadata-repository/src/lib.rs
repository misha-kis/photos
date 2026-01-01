use photos_domain::{ImageId, ImageRecord};
use photos_services::{ImageMetadataRepository, ImageMetadataRepositoryError};
use sqlx::{FromRow, QueryBuilder};
use std::path::{Path, PathBuf};

pub struct SqliteImageMetadataRepository {
    pool: sqlx::SqlitePool,
}

impl SqliteImageMetadataRepository {
    pub async fn new(path: PathBuf) -> Result<Self, ImageMetadataRepositoryError> {
        let db_path = path.join("db.sqlite");
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);
        let pool = sqlx::SqlitePool::connect_with(opts)
            .await
            .map_err(|_| ImageMetadataRepositoryError::CannotConnectOrCreate)?;
        let migrator = sqlx::migrate::Migrator::new(Path::new(
            "./crates/photos-infra-sqlite-image-metadata-repository/migrations",
        ))
        .await
        .map_err(|_| ImageMetadataRepositoryError::CannotConnectOrCreate)?;
        migrator
            .run(&pool)
            .await
            .map_err(|_| ImageMetadataRepositoryError::CannotConnectOrCreate)?;
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl ImageMetadataRepository for SqliteImageMetadataRepository {
    async fn add_image_record(
        &self,
        image_record: &ImageRecord,
    ) -> Result<(), ImageMetadataRepositoryError> {
        sqlx::query(r#"INSERT INTO image(uuid) VALUES ($1)"#)
            .bind(image_record.id)
            .execute(&self.pool)
            .await
            .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)?;
        Ok(())
    }

    async fn add_image_record_bulk(
        &self,
        image_records: &[ImageRecord],
    ) -> Result<(), ImageMetadataRepositoryError> {
        for record in image_records {
            self.add_image_record(record).await?;
        }
        Ok(())
        // QueryBuilder::new(r#"INSERT INTO image(uuid) "#)
        //     .push_values(image_records, |mut b, image_record| {
        //         b.push(image_record.id);
        //     })
        //     .build()
        //     .execute(&self.pool)
        //     .await
        //     .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)
        //     .map(|_| ())
    }

    async fn get_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<ImageRecord, ImageMetadataRepositoryError> {
        todo!()
    }

    async fn delete_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<(), ImageMetadataRepositoryError> {
        todo!()
    }

    async fn get_image_ids(&self) -> Result<Vec<ImageId>, ImageMetadataRepositoryError> {
        #[derive(FromRow)]
        struct Row {
            uuid: ImageId,
        }

        Ok(sqlx::query_as::<_, Row>(r#"SELECT uuid FROM image"#)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ImageMetadataRepositoryError::QueryFailed { err: e.to_string() })?
            .iter()
            .map(|row| row.uuid)
            .collect())
    }

    async fn get_number_of_images(&self) -> Result<u64, ImageMetadataRepositoryError> {
        #[derive(FromRow)]
        struct Row {
            uuid_count: u64,
        }
        sqlx::query_as::<_, Row>(r#"SELECT COUNT(uuid) AS uuid_count FROM image"#)
            .fetch_one(&self.pool)
            .await
            .map(|row| row.uuid_count)
            .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)
    }
}
