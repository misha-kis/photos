use crate::workers::db_worker::DbWorkerProxy;
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

pub(crate) struct ImportWorker {
    db_worker: Arc<Mutex<DbWorkerProxy>>,
    thumbnails_path: PathBuf,
    originals_path: PathBuf,
    thumbnail_sizes: Vec<u32>,
}

impl ImportWorker {
    async fn import(&self, path: &PathBuf) -> Result<()> {
        match image::open(&path) {
            Ok(img) => {
                let name = path.file_name().ok_or(anyhow!("invalid name"))?;
                self.db_worker
                    .lock()
                    .await
                    .insert_photo(name.to_str().ok_or(anyhow!("invalid name"))?.into())
                    .await?;
                let new_path = self.originals_path.join(name);
                std::fs::copy(&path, new_path)?;
                for size in &self.thumbnail_sizes {
                    let thumbnail = img.thumbnail(*size, *size);
                    thumbnail.save(self.thumbnails_path.join(format!("{size}")).join(name))?
                }

                Ok(())
            }
            Err(err) => Err(anyhow!("failed to open img")),
        }
    }
}

pub(crate) struct ImportWorkerProxy {
    handle: JoinHandle<()>,
    cmd_tx: Sender<Vec<PathBuf>>,
    res_rx: Receiver<Vec<anyhow::Error>>,
}

impl ImportWorkerProxy {
    pub(crate) fn new(
        db_worker: Arc<Mutex<DbWorkerProxy>>,
        thumbnails_path: PathBuf,
        originals_path: PathBuf,
        thumbnail_sizes: Vec<u32>,
    ) -> Self {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<Vec<PathBuf>>(32);
        let (res_tx, res_rx) = mpsc::channel(32);

        let import_worker = ImportWorker {
            db_worker,
            thumbnails_path,
            originals_path,
            thumbnail_sizes,
        };

        let handle = tokio::spawn(async move {
            while let Some(paths) = cmd_rx.recv().await {
                let mut failed_to_import = Vec::new();
                for path in paths {
                    if let Err(e) = import_worker.import(&path).await {
                        failed_to_import.push(e);
                    }
                }
                res_tx
                    .send(failed_to_import)
                    .await
                    .expect("expected to send");
            }
        });

        Self {
            handle,
            cmd_tx,
            res_rx,
        }
    }

    pub(crate) async fn import_photo(&mut self, photo_path: PathBuf) -> Result<()> {
        self.cmd_tx.send(vec![photo_path]).await?;
        let _res = self.res_rx.recv().await.expect("no receive or what?");
        Ok(())
    }
}
