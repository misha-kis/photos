use crate::ORIGINALS_SUBDIRECTORY;
use crate::workers::db_worker::{DbWorkerCmd, insert_photo};
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

pub(crate) struct ImportWorker {
    db_worker_tx: Sender<DbWorkerCmd>,
    thumbnails_path: PathBuf,
    originals_path: PathBuf,
    thumbnail_sizes: Vec<u32>,
}

impl ImportWorker {
    async fn import(&self, path: &PathBuf) -> Result<()> {
        match image::open(&path) {
            Ok(img) => {
                let name = path.file_name().ok_or(anyhow!("invalid name"))?;
                insert_photo(
                    &self.db_worker_tx,
                    name.to_str().ok_or(anyhow!("invalid name"))?.into(),
                )
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

pub(crate) fn spawn_import_worker(
    db_worker_tx: Sender<DbWorkerCmd>,
    thumbnails_path: PathBuf,
    originals_path: PathBuf,
    thumbnail_sizes: Vec<u32>,
) -> (
    JoinHandle<()>,
    Sender<Vec<PathBuf>>,
    Receiver<Vec<anyhow::Error>>,
) {
    let (import_cmd_tx, mut import_cmd_rx) = mpsc::channel::<Vec<PathBuf>>(32);
    let (import_res_tx, import_res_rx) = mpsc::channel(32);

    let import_worker = ImportWorker {
        db_worker_tx,
        thumbnails_path,
        originals_path,
        thumbnail_sizes,
    };

    let worker_thread = tokio::spawn(async move {
        while let Some(paths) = import_cmd_rx.recv().await {
            let mut failed_to_import = Vec::new();
            for path in paths {
                if let Err(e) = import_worker.import(&path).await {
                    failed_to_import.push(e);
                }
            }
            import_res_tx
                .send(failed_to_import)
                .await
                .expect("expected to send");
        }
    });

    (worker_thread, import_cmd_tx, import_res_rx)
}
