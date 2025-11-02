use crate::DB_PATH;
use anyhow::{Result, anyhow};
use cv::BoundingBox;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, Sqlite, SqlitePool};
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

    pub(crate) async fn insert_photo(&self, photo_name: &str) -> u32 {
        let last_inserted_id = sqlx::query("INSERT INTO image (image_name) VALUES (?)")
            .bind(photo_name)
            .execute(&self.pool)
            .await
            .expect("failed to insert")
            .last_insert_rowid();
        last_inserted_id as u32
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
}
