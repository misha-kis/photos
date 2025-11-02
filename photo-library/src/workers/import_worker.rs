use crate::workers::cv_worker::DetectFacesCommandResult;
use crate::{
    Command,
    workers::{cv_worker::DetectFacesCommand, db_worker::DbWorker},
};
use anyhow::{Context, Result, anyhow};
use futures::stream::{self, StreamExt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
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

    async fn import_many(&self, paths: Vec<PathBuf>) -> Result<Vec<u32>> {
        let concurrency = 8;

        let processed: Vec<_> = stream::iter(paths)
            .map(|path| {
                let originals_path = self.originals_path.clone();
                let thumbnails_path = self.thumbnails_path.clone();
                let thumbnail_sizes = self.thumbnail_sizes.clone();

                task::spawn_blocking(move || {
                    let img = image::open(&path)?;
                    let name = path.file_name().ok_or(anyhow!("invalid name"))?;
                    let name_str = name.to_str().ok_or(anyhow!("invalid name"))?;

                    let new_path = originals_path.join(name);
                    std::fs::copy(&path, &new_path)?;

                    for size in &thumbnail_sizes {
                        let thumbnail = img.thumbnail(*size, *size);
                        let thumb_path = thumbnails_path.join(format!("{size}")).join(name_str);
                        std::fs::create_dir_all(thumb_path.parent().unwrap())?;
                        thumbnail.save(&thumb_path)?;
                    }

                    Ok::<_, anyhow::Error>(name_str.to_string())
                })
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

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
            let mut db = self.db_worker.lock().await;
            db.insert_photos_bulk(image_names.clone()).await
        };

        Ok(image_ids)
    }

    pub(crate) async fn import(&self, path: &PathBuf) -> Result<Vec<u32>> {
        let meta = std::fs::metadata(path)?;
        if meta.is_file() {
            self.import_many(vec![path.clone()]).await
        } else if meta.is_dir() {
            self.import_many(
                std::fs::read_dir(path)?
                    .map(|entry| entry.unwrap().path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|ext| ext == "JPG" || ext == "jpeg" || ext == "PNG")
                    })
                    .collect(),
            )
            .await
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
        let resp = if let Ok(image_ids) = import_worker.import(&self.path).await {
            let mut rxs = Vec::new();
            let mut commands = Vec::new();
            for id in image_ids {
                let (tx, rx) = oneshot::channel();
                commands.push(Command::DetectFaces(DetectFacesCommand::new(id, tx)));
                rxs.push(rx);
            }

            if let Ok(()) = bulk_add_commands(commands, cmd_tx).await {
                Ok(ImportCommandResult { rxs })
            } else {
                Err(anyhow!("could not schedule a new command"))
            }
        } else {
            Err(anyhow!("could not import image"))
        };
        self.tx.send(resp).expect("is task cancelled?");
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
