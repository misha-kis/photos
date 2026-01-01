use photos_domain::{ImageId, ImageRecord};
use photos_services::{ImageMetadataRepository, ImageMetadataRepositoryError};
use sqlx::types::Uuid;
use sqlx::{FromRow, QueryBuilder};
use std::path::PathBuf;

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
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl ImageMetadataRepository for SqliteImageMetadataRepository {
    async fn add_image_record(
        &self,
        image_record: ImageRecord,
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
        QueryBuilder::new(r#"INSERT INTO image(uuid) "#)
            .push_values(image_records, |mut b, image_record| {
                b.push(image_record.id);
            })
            .build()
            .execute(&self.pool)
            .await
            .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)
            .map(|_| ())
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
            uuid: String,
        }

        Ok(sqlx::query_as::<_, Row>(r#"SELECT uuid FROM image"#)
            .fetch_all(&self.pool)
            .await
            .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)?
            .iter()
            .map(|row| ImageId::parse_str(&row.uuid).unwrap())
            .collect())
    }
}
