mod storage;

use eframe::egui::{self, TextureHandle};
use photos_app::{App, AppEvent, JobHandle};
use photos_domain::{ImageId, Uuid};
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tokio_util::sync::CancellationToken;

use crate::app_proxy::storage::FullImage;
use storage::{FaceThumbnail, ImportItemPaths, ImportThumbnail, Storable, Storage, Thumbnail};

pub struct AppProxy {
    app: Rc<App>,
    pub image_ids: Vec<ImageId>,
    pub face_clusters: Vec<(Uuid, Vec<Uuid>)>,
    thumbnails: Storage<Thumbnail>,
    face_detection_thumbnails: Storage<FaceThumbnail>,
    import_thumbnails: Storage<ImportThumbnail>,
    discovered_items: Storage<ImportItemPaths>,
    full_images: Storage<FullImage>,
    import_job_handle: Option<JobHandle>,
}

impl AppProxy {
    pub fn new(
        gallery_dir: PathBuf,
        app_options: photos_app::config::Options,
    ) -> anyhow::Result<Self> {
        let app = Rc::new(photos_app::App::new(gallery_dir, app_options)?);

        let receiver = app.get_image_ids();
        let mut image_ids = Vec::new();
        if let Ok(Ok(ids)) = receiver.rx.blocking_recv() {
            image_ids = ids;
        }

        Ok(Self {
            app: app.clone(),
            image_ids,
            face_clusters: Vec::new(),
            thumbnails: Storage::new(app.clone(), NonZero::new(2048).unwrap()),
            face_detection_thumbnails: Storage::new(app.clone(), NonZero::new(2048).unwrap()),
            import_thumbnails: Storage::new(app.clone(), NonZero::new(2048).unwrap()),
            discovered_items: Storage::new(app.clone(), NonZero::new(2048).unwrap()),
            full_images: Storage::new(app.clone(), NonZero::new(32).unwrap()),
            import_job_handle: None,
        })
    }

    pub fn number_of_images(&self) -> usize {
        self.image_ids.len()
    }

    pub(crate) fn get_image(
        &mut self,
        id: &<FullImage as Storable>::Id,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<TextureHandle> {
        self.full_images.get(id, ctx, cancel).map(|x| x.0.clone())
    }

    pub(crate) fn get_thumbnail(
        &mut self,
        id: &<Thumbnail as Storable>::Id,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<TextureHandle> {
        self.thumbnails.get(id, ctx, cancel).map(|x| x.0.clone())
    }

    pub fn get_face_detection_thumbnail(
        &mut self,
        id: &Uuid,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<TextureHandle> {
        self.face_detection_thumbnails
            .get(id, ctx, cancel)
            .map(|x| x.0.clone())
    }

    pub fn get_import_thumbnail(
        &mut self,
        path: &PathBuf,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<TextureHandle> {
        self.import_thumbnails
            .get(path, ctx, cancel)
            .map(|x| x.0.clone())
    }

    pub fn get_discovered_import_items(
        &mut self,
        path: &Path,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<&Vec<PathBuf>> {
        self.discovered_items
            .get(&path.to_path_buf(), ctx, cancel)
            .map(|x| &x.0)
    }

    pub fn start_import(&mut self, paths: Vec<PathBuf>) {
        let receiver = self.app.import_items(paths);
        self.import_job_handle = Some(receiver);
    }

    pub fn get_import_job_handle(&mut self) -> Option<&mut JobHandle> {
        self.import_job_handle.as_mut()
    }

    pub fn process_events(&mut self) {}

    pub fn refresh_images(&mut self) {
        let jh = self.app.get_image_ids();
        if let Ok(Ok(ids)) = jh.rx.blocking_recv() {
            self.image_ids = ids;
        }
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
