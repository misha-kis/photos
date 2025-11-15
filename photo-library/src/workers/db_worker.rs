use crate::DB_PATH;
use anyhow::{Result, anyhow};
use cv::BoundingBox;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Acquire, Row, Sqlite, SqlitePool};
use std::path::PathBuf;

async fn init_db(mut conn: PoolConnection<Sqlite>) -> Result<()> {
    sqlx::query(
        r#"
CREATE TABLE IF NOT EXISTS image (
    image_id INTEGER PRIMARY KEY AUTOINCREMENT,
    image_name TEXT NOT NULL UNIQUE,
    image_created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS face_detection (
    face_detection_id INTEGER PRIMARY KEY AUTOINCREMENT,
    image_id INTEGER NOT NULL,
    roi_x1 INTEGER NOT NULL CHECK (roi_x1 >= 0),
    roi_y1 INTEGER NOT NULL CHECK (roi_y1 >= 0),
    roi_x2 INTEGER NOT NULL CHECK (roi_x2 > roi_x1),
    roi_y2 INTEGER NOT NULL CHECK (roi_y2 > roi_y1),
    embedding BLOB DEFAULT NULL,
    face_id INTEGER DEFAULT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (image_id) REFERENCES image(image_id) ON DELETE CASCADE ON UPDATE CASCADE
);
            "#,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(crate) struct DbWorker {
    pool: sqlx::SqlitePool,
}

impl DbWorker {
    pub(crate) async fn new(library_path: &PathBuf) -> Result<Self> {
        let db_path = library_path.join(DB_PATH);
        let options = SqliteConnectOptions::new()
            .filename(db_path.to_str().unwrap())
            .create_if_missing(true);
        let db_pool = SqlitePool::connect_with(options).await?;
        let conn = db_pool.acquire().await?;
        init_db(conn).await?;
        Ok(Self { pool: db_pool })
    }

    pub(crate) async fn get_photo_name_by_photo_id(&self, photo_id: u32) -> Result<String> {
        let rows = sqlx::query("SELECT image_name FROM image WHERE image_id = ?")
            .bind(photo_id)
            .fetch_all(&self.pool)
            .await?;
        if rows.len() != 1 {
            Err(anyhow!("too many rows"))
        } else {
            rows[0].try_get("image_name").map_err(|e| e.into())
        }
    }

    pub(crate) async fn insert_photos_bulk(&self, photo_names: Vec<String>) -> Vec<u32> {
        tracing::info!("Writing image info to db");
        if photo_names.is_empty() {
            return Vec::new();
        }

        let mut qb = sqlx::QueryBuilder::new("INSERT INTO image (image_name) ");
        qb.push_values(photo_names.iter(), |mut b, name| {
            b.push_bind(name);
        });
        qb.push(" RETURNING image_id");

        let rows = qb
            .build_query_as::<(i64,)>()
            .fetch_all(&self.pool)
            .await
            .expect("failed to bulk insert photos");

        rows.into_iter().map(|(id,)| id as u32).collect()
    }

    pub(crate) async fn get_face_detection(&self, detection_id: u32) -> Option<(u32, BoundingBox)> {
        let rows = sqlx::query("SELECT image_id, roi_x1, roi_y1, roi_x2, roi_y2 FROM face_detection WHERE face_detection_id = ?")
            .bind(detection_id)
            .fetch_all(&self.pool)
            .await.expect("failed to fetch");
        if rows.len() != 1 {
            None
        } else {
            let row = &rows[0];
            let image_id = row.try_get("image_id").expect("failed to get image_id");
            let x1 = row.try_get("roi_x1").expect("failed to get roi_x1");
            let y1 = row.try_get("roi_y1").expect("failed to get roi_y1");
            let x2 = row.try_get("roi_x2").expect("failed to get roi_x2");
            let y2 = row.try_get("roi_y2").expect("failed to get roi_y2");
            Some((image_id, BoundingBox::new(x1, y1, x2, y2)))
        }
    }

    pub(crate) async fn insert_face_detection(
        &self,
        photo_id: u32,
        face_box: cv::BoundingBox,
    ) -> Result<u32> {
        let mut conn = self.pool.acquire().await?;
        let last_inserted_id =
            sqlx::query("INSERT INTO face_detection (image_id, roi_x1, roi_y1, roi_x2, roi_y2) VALUES (?, ?, ?, ?, ?)")
                .bind(photo_id)
                .bind(face_box.x1)
                .bind(face_box.y1)
                .bind(face_box.x2)
                .bind(face_box.y2)
                .execute(&mut *conn)
                .await
                .expect("failed to insert")
                .last_insert_rowid();
        Ok(last_inserted_id as u32)
    }

    pub(crate) async fn insert_face_embedding(
        &self,
        face_detection_id: u32,
        face_embedding: [f32; 512],
    ) -> Result<i64> {
        let mut conn = self.pool.acquire().await?;

        let embedding = bytemuck::cast_slice(&face_embedding);
        let last_inserted_id =
            sqlx::query("UPDATE face_detection SET embedding = ? WHERE face_detection_id  = ?")
                .bind(embedding)
                .bind(face_detection_id)
                .execute(&mut *conn)
                .await
                .expect("cound not insert")
                .last_insert_rowid();
        Ok(last_inserted_id)
    }

    pub(crate) async fn get_number_of_images(&self) -> Result<u32> {
        sqlx::query("SELECT COUNT(image_id) AS count FROM image")
            .fetch_one(&self.pool)
            .await?
            .try_get("count")
            .map_err(|e| e.into())
    }

    /// Get all face detections with their embeddings
    /// Returns a vector of (detection_id, embedding) tuples
    /// Only includes detections that have embeddings
    pub(crate) async fn get_all_face_embeddings(&self) -> Result<Vec<(u32, [f32; 512])>> {
        let rows = sqlx::query(
            "SELECT face_detection_id, embedding FROM face_detection WHERE embedding IS NOT NULL"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            let detection_id: i64 = row.try_get("face_detection_id")?;
            let embedding_blob: Vec<u8> = row.try_get("embedding")?;
            
            if embedding_blob.len() == 512 * 4 {
                let embedding_slice: &[f32] = bytemuck::cast_slice(&embedding_blob);
                if embedding_slice.len() == 512 {
                    let mut embedding = [0f32; 512];
                    embedding.copy_from_slice(embedding_slice);
                    result.push((detection_id as u32, embedding));
                }
            } else {
                tracing::error!("Invalid embedding blob length: {}", embedding_blob.len());
            }
        }
        Ok(result)
    }

    pub(crate) async fn bulk_update_face_ids(
        &self,
        updates: Vec<(u32, Option<u32>)>,
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        let mut conn = self.pool.acquire().await?;
        let mut tx = conn.begin().await?;
        let mut query_builder = sqlx::QueryBuilder::new(
            "WITH updates(detection_id, face_id) AS (VALUES "
        );
        
        let mut separated = query_builder.separated(", ");
        for (detection_id, face_id) in &updates {
            separated.push("(");
            separated.push_bind(detection_id);
            separated.push(",");
            separated.push_bind(face_id.map(|id| id as i64));
            separated.push(")");
        }
        
        query_builder.push(
            ") UPDATE face_detection \
             SET face_id = (SELECT face_id FROM updates WHERE updates.detection_id = face_detection.face_detection_id) \
             WHERE face_detection_id IN (SELECT detection_id FROM updates)"
        );
        
        query_builder
            .build()
            .execute(&mut *tx)
            .await?;
        
        tx.commit().await?;
        Ok(())
    }
}
