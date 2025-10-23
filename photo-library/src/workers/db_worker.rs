use crate::DB_PATH;
use anyhow::{Result, anyhow};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, Sqlite, SqlitePool};
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

pub(crate) enum DbWorkerCmd {
    GetPhotoNameByPhotoId {
        photo_id: u32,
        response_tx: oneshot::Sender<Result<String>>,
    },
    InsertPhoto {
        photo_name: String,
        response_tx: oneshot::Sender<Result<u32>>,
    },
    InsertFaceDetection {
        photo_id: u32,
        face_box: cv::BoundingBox,
        response_tx: oneshot::Sender<Result<u32>>,
    },
    GetFaceDetectionsWithoutEmbedding {
        response_tx: oneshot::Sender<Result<Vec<(u32, cv::BoundingBox)>>>,
    },
    InsertFaceEmbedding {
        face_detection_id: u32,
        embedding: [f32; 512],
        response_tx: oneshot::Sender<Result<u32>>,
    },
}

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

pub(crate) struct DbWorkerProxy {
    cmd_tx: mpsc::Sender<DbWorkerCmd>,
}

impl DbWorkerProxy {
    pub(crate) async fn new(library_path: &PathBuf) -> Result<Self> {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
        let db_path = library_path.join(DB_PATH);
        let options = SqliteConnectOptions::new()
            .filename(db_path.to_str().unwrap())
            .create_if_missing(true);
        let db_pool = SqlitePool::connect_with(options).await?;
        let conn = db_pool.acquire().await?;
        init_db(conn).await?;

        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                let mut conn = db_pool
                    .acquire()
                    .await
                    .expect("failed to acquire db connection");
                match cmd {
                    DbWorkerCmd::GetPhotoNameByPhotoId {
                        photo_id,
                        response_tx,
                    } => {
                        let rows = sqlx::query("SELECT image_name FROM image WHERE image_id = ?")
                            .bind(photo_id)
                            .fetch_all(&db_pool)
                            .await;
                        let response = if let Ok(rows) = rows {
                            if rows.len() != 1 {
                                Err(anyhow!("too many rows"))
                            } else {
                                rows[0].try_get("image_name").map_err(|e| e.into())
                            }
                        } else {
                            Err(anyhow!("query error"))
                        };
                        response_tx
                            .send(response)
                            .expect("db_worker could not send response");
                    }
                    DbWorkerCmd::InsertPhoto {
                        photo_name,
                        response_tx,
                    } => {
                        let last_inserted_id =
                            sqlx::query("INSERT INTO image (image_name) VALUES (?)")
                                .bind(photo_name)
                                .execute(&mut *conn)
                                .await
                                .expect("failed to insert")
                                .last_insert_rowid();
                        response_tx
                            .send(Ok(last_inserted_id as u32))
                            .expect("db_worker could not send response");
                    }
                    DbWorkerCmd::InsertFaceDetection {
                        photo_id,
                        face_box,
                        response_tx,
                    } => {
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
                        response_tx
                            .send(Ok(last_inserted_id as u32))
                            .expect("db_worker could not send response");
                    }
                    DbWorkerCmd::GetFaceDetectionsWithoutEmbedding { response_tx } => {
                        let rows = sqlx::query("SELECT face_detection_id, roi_x1, roi_y1, roi_x2, roi_y2 FROM face_detection WHERE embedding IS NULL")
                            .fetch_all(&mut *conn)
                            .await
                            .expect("failed to fetch");

                        let detections = rows
                            .into_iter()
                            .map(|row| {
                                let face_detection_id = row.get(0);
                                let roi = cv::BoundingBox {
                                    x1: row.get(1),
                                    y1: row.get(2),
                                    x2: row.get(3),
                                    y2: row.get(4),
                                };
                                (face_detection_id, roi)
                            })
                            .collect();

                        response_tx
                            .send(Ok(detections))
                            .expect("db_worker could not send response");
                    }
                    DbWorkerCmd::InsertFaceEmbedding {
                        face_detection_id,
                        embedding,
                        response_tx,
                    } => {
                        let embedding = bytemuck::cast_slice(&embedding);
                        let last_inserted_id = sqlx::query(
                            "UPDATE face_detection SET embedding = ? WHERE face_detection_id  = ?",
                        )
                        .bind(embedding)
                        .bind(face_detection_id)
                        .execute(&mut *conn)
                        .await
                        .expect("cound not insert")
                        .last_insert_rowid();

                        response_tx
                            .send(Ok(last_inserted_id as u32))
                            .expect("db_worker could not send response");
                    }
                }
            }
        });

        Ok(Self { cmd_tx })
    }
    pub(crate) async fn get_photo_name_by_id(&self, photo_id: u32) -> Result<String> {
        let (response_tx, response_rx) = oneshot::channel();
        let cmd = DbWorkerCmd::GetPhotoNameByPhotoId {
            photo_id,
            response_tx,
        };
        self.cmd_tx.send(cmd).await?;
        response_rx.await?
    }
    pub(crate) async fn insert_photo(&self, photo_name: String) -> Result<u32> {
        let (response_tx, response_rx) = oneshot::channel();
        let cmd = DbWorkerCmd::InsertPhoto {
            photo_name,
            response_tx,
        };
        self.cmd_tx.send(cmd).await?;
        response_rx.await?
    }

    pub(crate) async fn insert_face_detection(
        &self,
        photo_id: u32,
        face_box: cv::BoundingBox,
    ) -> Result<u32> {
        let (response_tx, response_rx) = oneshot::channel();
        let cmd = DbWorkerCmd::InsertFaceDetection {
            photo_id,
            face_box,
            response_tx,
        };
        self.cmd_tx.send(cmd).await?;
        response_rx.await?
    }

    pub(crate) async fn get_face_detections_without_embedding(
        &self,
    ) -> Result<Vec<(u32, cv::BoundingBox)>> {
        let (response_tx, response_rx) = oneshot::channel();
        let cmd = DbWorkerCmd::GetFaceDetectionsWithoutEmbedding { response_tx };
        self.cmd_tx.send(cmd).await?;
        response_rx.await?
    }

    pub(crate) async fn insert_face_detection_embedding(
        &self,
        face_detection_id: u32,
        embedding: [f32; 512],
    ) -> Result<u32> {
        let (response_tx, response_rx) = oneshot::channel();
        let cmd = DbWorkerCmd::InsertFaceEmbedding {
            face_detection_id,
            embedding,
            response_tx,
        };
        self.cmd_tx.send(cmd).await?;
        response_rx.await?
    }
}
