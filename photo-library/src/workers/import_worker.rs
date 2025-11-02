use crate::workers::cv_worker::DetectFacesCommandResult;
use crate::{
    Command,
    workers::{cv_worker::DetectFacesCommand, db_worker::DbWorker},
};
use anyhow::{Result, anyhow};
use std::fmt::Formatter;
use std::sync::Arc;
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use tokio::sync::{Mutex, mpsc, oneshot};

pub(crate) struct ImportWorker {
    db_worker: Arc<Mutex<DbWorker>>,
    thumbnails_path: PathBuf,
    originals_path: PathBuf,
    thumbnail_sizes: Vec<u32>,
}

impl ImportWorker {
    pub(crate) fn new(
        db_worker: Arc<Mutex<DbWorker>>,
        thumbnails_path: PathBuf,
        originals_path: PathBuf,
        thumbnail_sizes: Vec<u32>,
    ) -> Self {
        Self {
            db_worker,
            thumbnails_path,
            originals_path,
            thumbnail_sizes,
        }
    }

    pub(crate) async fn import(&self, path: &PathBuf) -> Result<u32> {
        match image::open(&path) {
            Ok(img) => {
                let name = path.file_name().ok_or(anyhow!("invalid name"))?;
                let image_id = self
                    .db_worker
                    .lock()
                    .await
                    .insert_photo(name.to_str().ok_or(anyhow!("invalid name"))?.into())
                    .await;
                let new_path = self.originals_path.join(name);
                std::fs::copy(&path, new_path)?;
                for size in &self.thumbnail_sizes {
                    let thumbnail = img.thumbnail(*size, *size);
                    thumbnail.save(self.thumbnails_path.join(format!("{size}")).join(name))?
                }

                Ok(image_id)
            }
            Err(err) => Err(anyhow!("failed to open img, {}", err)),
        }
    }
}

pub(crate) struct ImportCommand {
    pub(crate) path: PathBuf,
    pub(crate) tx: oneshot::Sender<Result<ImportCommandResult>>,
}

impl ImportCommand {
    pub(crate) async fn execute(
        self,
        import_worker: &ImportWorker,
        cmd_tx: &mpsc::Sender<Command>,
    ) {
        tracing::debug!("Importing image");
        let resp = if let Ok(image_id) = import_worker.import(&self.path).await {
            let (res_tx, res_rx) = oneshot::channel();
            if let Ok(()) = cmd_tx
                .send(Command::DetectFaces(DetectFacesCommand::new(
                    image_id, res_tx,
                )))
                .await
            {
                Ok(ImportCommandResult { rx: res_rx })
            } else {
                Err(anyhow!("could not schedule a new command"))
            }
        } else {
            Err(anyhow!("could not import image"))
        };
        self.tx.send(resp).expect("is task cancelled?");
    }
}

impl std::fmt::Debug for ImportCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportCommand")
            .field("path", &self.path)
            .finish()
    }
}

pub(crate) struct ImportCommandResult {
    pub(crate) rx: oneshot::Receiver<DetectFacesCommandResult>,
}

impl std::fmt::Debug for ImportCommandResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportCommandResult").finish()
    }
}
