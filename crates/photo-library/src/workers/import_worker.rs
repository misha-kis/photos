use crate::workers::cv_worker::DetectFacesCommandResult;
use crate::workers::image_loader_worker::UpdateImageNameMapCommand;
use crate::{
    Command,
    workers::{cv_worker::DetectFacesCommand, db_worker::DbWorker},
};
use anyhow::{Result, anyhow};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task;

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

    async fn import_many(&self, paths: Vec<PathBuf>) -> Result<HashMap<u32, String>> {
        tracing::info!("Copying {} images", paths.len());
        let concurrency = 8;

        let processed: Vec<_> = stream::iter(paths)
            .map(|path| {
                let originals_path = self.originals_path.clone();
                let thumbnails_path = self.thumbnails_path.clone();
                let thumbnail_sizes = self.thumbnail_sizes.clone();

                task::spawn_blocking(move || {
                    tracing::info!("Copying image {}", path.display());
                    let img = image::open(&path)?;
                    let name = path.file_name().ok_or(anyhow!("invalid name"))?;
                    let name_str = name.to_str().ok_or(anyhow!("invalid name"))?;

                    let new_path = originals_path.join(name);
                    std::fs::copy(&path, &new_path)?;

                    for size in &thumbnail_sizes {
                        let thumbnail = img.thumbnail(*size, *size);
                        let thumb_path = thumbnails_path.join(format!("{size}")).join(name_str);
                        if let Some(parent) = thumb_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        thumbnail.save(&thumb_path)?;
                    }
                    tracing::info!("Copied image {}", path.display());

                    Ok::<_, anyhow::Error>(name_str.to_string())
                })
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;
        tracing::info!("Done copying images");

        let image_names: Vec<String> = processed
            .into_iter()
            .filter_map(|res| match res {
                Ok(Ok(name)) => Some(name),
                Ok(Err(err)) => {
                    tracing::error!("Error processing image: {}", err);
                    None
                }
                Err(join_err) => {
                    tracing::error!("Task join error: {}", join_err);
                    None
                }
            })
            .collect();

        let image_ids = {
            let db = self.db_worker.lock().await;
            db.insert_photos_bulk(image_names.clone()).await
        };

        Ok(image_ids.into_iter().zip(image_names.into_iter()).collect())
    }

    pub(crate) async fn import(&self, path: &PathBuf) -> Result<HashMap<u32, String>> {
        let meta = std::fs::metadata(path)?;
        if meta.is_file() {
            tracing::debug!("Import worker importing file: {}", path.display());
            self.import_many(vec![path.clone()]).await
        } else if meta.is_dir() {
            tracing::debug!("Import worker importing directory: {}", path.display());
            let paths: Vec<PathBuf> = std::fs::read_dir(path)?
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png"))
                        .unwrap_or(false)
                })
                .collect();
            self.import_many(paths).await
        } else {
            Err(anyhow!("invalid path"))
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
        let resp = if let Ok(new_image_name_map) = import_worker.import(&self.path).await {
            let mut rxs = Vec::new();
            let mut commands = Vec::new();
            for (id, _) in &new_image_name_map {
                let (tx, rx) = oneshot::channel();
                commands.push(Command::DetectFaces(DetectFacesCommand::new(*id, tx)));
                rxs.push(rx);
            }
            commands.insert(
                0,
                Command::UpdateImageNameMap(UpdateImageNameMapCommand { new_image_name_map }),
            );

            if let Ok(()) = bulk_add_commands(commands, cmd_tx).await {
                Ok(ImportCommandResult { rxs })
            } else {
                Err(anyhow!("could not schedule a new command"))
            }
        } else {
            Err(anyhow!("could not import image"))
        };
        if let Err(e) = self.tx.send(resp) {
            tracing::warn!("Import command receiver dropped: {:?}", e);
        }
    }
}

async fn bulk_add_commands(commands: Vec<Command>, cmd_tx: &mpsc::Sender<Command>) -> Result<()> {
    for command in commands {
        cmd_tx.send(command).await?
    }
    Ok(())
}

impl std::fmt::Debug for ImportCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportCommand")
            .field("path", &self.path)
            .finish()
    }
}

pub struct ImportCommandResult {
    pub rxs: Vec<oneshot::Receiver<DetectFacesCommandResult>>,
}

impl std::fmt::Debug for ImportCommandResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportCommandResult").finish()
    }
}
