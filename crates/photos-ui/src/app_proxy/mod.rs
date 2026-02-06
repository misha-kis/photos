mod storage;

use eframe::egui::{self, TextureHandle};
use photos_app::{App, AppEvent};
use photos_core::Uuid;
use photos_domain::ImageId;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;

use storage::{FaceThumbnail, ImportThumbnail, Storable, Storage, Thumbnail};

pub struct AppProxy {
    app: Rc<App>,
    pub image_ids: Vec<ImageId>,
    pub face_clusters: Vec<(Uuid, Vec<Uuid>)>,
    thumbnail_storage: Storage<Thumbnail>,
    face_detection_thumbnail_storage: Storage<FaceThumbnail>,
    import_thumbnail_storage: Storage<ImportThumbnail>,
    import_discovery_receiver: Option<Receiver<AppEvent>>,
    import_workflow_receiver: Option<Receiver<AppEvent>>,
    discovered_items: Option<Vec<PathBuf>>,
}

impl AppProxy {
    pub fn new(
        gallery_dir: PathBuf,
        app_options: photos_app::config::Options,
    ) -> anyhow::Result<Self> {
        let app = Rc::new(photos_app::App::new(gallery_dir, app_options)?);

        let mut receiver = app.get_image_ids();
        let mut image_ids = Vec::new();

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            if let Some(AppEvent::ImageIdsReady { result }) = receiver.recv().await
                && let Ok(ids) = result
            {
                image_ids = ids;
            }
        });

        Ok(Self {
            app: app.clone(),
            image_ids,
            face_clusters: Vec::new(),
            thumbnail_storage: Storage::new(app.clone()),
            face_detection_thumbnail_storage: Storage::new(app.clone()),
            import_thumbnail_storage: Storage::new(app.clone()),
            import_discovery_receiver: None,
            import_workflow_receiver: None,
            discovered_items: None,
        })
    }

    pub fn number_of_images(&self) -> usize {
        self.image_ids.len()
    }

    pub(crate) fn get_thumbnail(
        &mut self,
        id: &<Thumbnail as Storable>::Id,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<TextureHandle> {
        self.thumbnail_storage
            .get(id, ctx, cancel)
            .cloned()
            .map(|x| x.0)
    }

    pub fn get_face_detection_thumbnail(
        &mut self,
        id: &Uuid,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<TextureHandle> {
        self.face_detection_thumbnail_storage
            .get(id, ctx, cancel)
            .cloned()
            .map(|x| x.0)
    }

    pub fn get_import_thumbnail(
        &mut self,
        path: &PathBuf,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<TextureHandle> {
        self.import_thumbnail_storage
            .get(path, ctx, cancel)
            .cloned()
            .map(|x| x.0)
    }

    pub fn request_discover_import_items(&mut self, path: &Path) {
        if self.import_discovery_receiver.is_none() {
            let receiver = self.app.discover_import_items(path.to_path_buf());
            self.import_discovery_receiver = Some(receiver);
        }
    }

    pub fn get_discovered_items(&self) -> Option<&Vec<PathBuf>> {
        self.discovered_items.as_ref()
    }

    pub fn start_import(&mut self, paths: Vec<PathBuf>) {
        let receiver = self.app.import_items(paths);
        self.import_workflow_receiver = Some(receiver);
    }

    pub fn get_import_workflow_receiver(&mut self) -> Option<&mut Receiver<AppEvent>> {
        self.import_workflow_receiver.as_mut()
    }

    pub fn get_discovery_receiver(&mut self) -> Option<&mut Receiver<AppEvent>> {
        self.import_discovery_receiver.as_mut()
    }

    pub fn process_events(&mut self) {
        if let Some(receiver) = &mut self.import_discovery_receiver
            && let Ok(AppEvent::ImportItemsDiscovered { result, .. }) = receiver.try_recv()
        {
            if let Ok(items) = result {
                self.discovered_items = Some(items);
            }
            self.import_discovery_receiver = None;
        }
    }

    pub fn refresh_images(&mut self) {
        let mut receiver = self.app.get_image_ids();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Some(AppEvent::ImageIdsReady { result }) = receiver.recv().await
                && let Ok(ids) = result
            {
                self.image_ids = ids;
            }
        });
    }

    pub fn refresh_face_clusters(&mut self) {
        let receiver = self.app.get_face_clusters();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Ok(AppEvent::FaceClustersReady { result }) = receiver.await
                && let Ok(clusters) = result
            {
                self.face_clusters = clusters;
            }
        })
    }
}

pub enum ImportProgress {
    Progress(u64, u64),
    Done,
}

impl ImportProgress {
    pub fn from_app_event(event: &photos_app::AppEvent) -> Option<Self> {
        match event {
            photos_app::AppEvent::ImportProgress { current, total, .. } => {
                Some(ImportProgress::Progress(*current, *total))
            }
            photos_app::AppEvent::ImportFinished { .. } => Some(ImportProgress::Done),
            _ => None,
        }
    }
}
