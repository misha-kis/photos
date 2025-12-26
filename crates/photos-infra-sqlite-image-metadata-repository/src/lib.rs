use photos_domain::{ImageId, ImageRecord};
use photos_services::{ImageMetadataRepository, ImageMetadataRepositoryError};

pub struct SqliteImageMetadataRepository {
    pool: sqlx::SqlitePool,
}

#[async_trait::async_trait]
impl ImageMetadataRepository for SqliteImageMetadataRepository {
    async fn add_image_record(
        &mut self,
        image_meta: ImageRecord,
    ) -> Result<(), ImageMetadataRepositoryError> {
        sqlx::query(
            r#"
INSERT INTO image(uuid) VALUES ($1)
            "#,
        )
        .bind(image_meta.id)
        .execute(&self.pool)
        .await
        .map_err(|_| ImageMetadataRepositoryError::ImageMetadataRepositoryError)?;
        Ok(())
    }

    async fn get_image_record(
        &self,
        image_id: ImageId,
    ) -> Result<ImageRecord, ImageMetadataRepositoryError> {
        todo!()
    }

    async fn delete_image_record(
        &mut self,
        image_id: ImageId,
    ) -> Result<(), ImageMetadataRepositoryError> {
        todo!()
    }
}
