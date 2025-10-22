use crate::DB_PATH;
use anyhow::{Result, anyhow};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, Row, Sqlite, SqlitePool};
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

pub(crate) enum DbWorkerCmd {
    GetPhotoNameByPhotoId {
        photo_id: u32,
        response_tx: oneshot::Sender<Result<String>>,
    },
    InsertPhoto {
        photo_name: String,
        response_tx: oneshot::Sender<Result<u32>>,
    },
}

async fn init_db(mut conn: PoolConnection<Sqlite>) -> Result<()> {
    sqlx::query(
        r#"
CREATE TABLE IF NOT EXISTS photo
(
photo_id INTEGER PRIMARY KEY NOT NULL,
photo_name TEXT NOT NULL
);
            "#,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(crate) async fn spawn_db_worker(
    library_path: &PathBuf,
) -> Result<(JoinHandle<()>, mpsc::Sender<DbWorkerCmd>)> {
    let (db_cmd_tx, mut db_cmd_rx) = mpsc::channel(32);
    let db_path = library_path.join(DB_PATH);
    let options = SqliteConnectOptions::new()
        .filename(db_path.to_str().unwrap())
        .create_if_missing(true);
    let db_pool = SqlitePool::connect_with(options).await?;
    let conn = db_pool.acquire().await?;
    init_db(conn).await?;

    let worker = tokio::spawn(async move {
        while let Some(cmd) = db_cmd_rx.recv().await {
            let mut conn = db_pool
                .acquire()
                .await
                .expect("failed to acquire db connection");
            match cmd {
                DbWorkerCmd::GetPhotoNameByPhotoId {
                    photo_id,
                    response_tx,
                } => {
                    let rows = sqlx::query("SELECT photo_name FROM photo WHERE photo_id = ?")
                        .bind(photo_id)
                        .fetch_all(&db_pool)
                        .await;
                    let response = if let Ok(rows) = rows {
                        if rows.len() != 1 {
                            Err(anyhow!("too many rows"))
                        } else {
                            rows[0].try_get("photo_name").map_err(|e| e.into())
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
                    let last_inserted_id = sqlx::query("INSERT INTO photo (photo_name) VALUES (?)")
                        .bind(photo_name)
                        .execute(&mut *conn)
                        .await
                        .expect("failed to insert")
                        .last_insert_rowid();
                    response_tx
                        .send(Ok(last_inserted_id as u32))
                        .expect("db_worker could not send response");
                }
            }
        }
    });

    Ok((worker, db_cmd_tx))
}

pub(crate) async fn get_photo_name_by_id(
    db_cmd_tx: &mpsc::Sender<DbWorkerCmd>,
    photo_id: u32,
) -> Result<String> {
    let (response_tx, response_rx) = oneshot::channel();
    let cmd = DbWorkerCmd::GetPhotoNameByPhotoId {
        photo_id,
        response_tx,
    };
    db_cmd_tx.send(cmd).await?;
    response_rx.await?
}

pub(crate) async fn insert_photo(
    db_cmd_tx: &mpsc::Sender<DbWorkerCmd>,
    photo_name: String,
) -> Result<u32> {
    let (response_tx, response_rx) = oneshot::channel();
    let cmd = DbWorkerCmd::InsertPhoto {
        photo_name,
        response_tx,
    };
    db_cmd_tx.send(cmd).await?;
    response_rx.await?
}
