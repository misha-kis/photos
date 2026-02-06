use eframe::egui::{self, TextureHandle};
use image::{DynamicImage, RgbaImage};
use photos_app::{App, AppEvent, OneshotJobHandle};
use photos_core::Uuid;
use photos_domain::ImageId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;

pub(crate) trait CtxInto<T: Sized> {
    fn ctx_into(self, ctx: &egui::Context) -> T;
}

pub(crate) trait Storable: Sized {
    type Id: Eq + std::hash::Hash + Copy;
    type ReceiveAs: Sized + CtxInto<Self>;
    fn load(app: &App, id: Self::Id) -> OneshotJobHandle<Self::ReceiveAs>;
}

struct Storage<T: Storable> {
    app: Rc<App>,
    cache: HashMap<T::Id, T>,
    jobs: HashMap<T::Id, OneshotJobHandle<T::ReceiveAs>>,
}

impl<T: Storable> Storage<T> {
    fn new(app: Rc<App>) -> Self {
        Self {
            app,
            cache: Default::default(),
            jobs: Default::default(),
        }
    }

    fn get(&mut self, id: T::Id, ctx: &egui::Context) -> Option<&T> {
        if self.cache.contains_key(&id) {
            return self.cache.get(&id);
        }

        if let Some(job) = self.jobs.get_mut(&id) {
            return match job.rx.try_recv() {
                Ok(Ok(value)) => {
                    self.jobs.remove(&id);
                    self.cache.insert(id, value.ctx_into(ctx));
                    self.cache.get(&id)
                }
                Ok(Err(_)) | Err(oneshot::error::TryRecvError::Closed) => {
                    self.jobs.remove(&id);
                    None
                }
                Err(oneshot::error::TryRecvError::Empty) => None,
            };
        }

        let job = T::load(self.app.as_ref(), id);
        self.jobs.insert(id, job);
        None
    }
}

#[derive(Clone)]
pub(crate) struct Thumbnail(TextureHandle);

impl Storable for Thumbnail {
    type Id = ImageId;
    type ReceiveAs = RgbaImage;

    fn load(app: &App, id: Self::Id) -> OneshotJobHandle<Self::ReceiveAs> {
        app.get_thumbnail(id, 128)
    }
}

impl CtxInto<Thumbnail> for RgbaImage {
    fn ctx_into(self, ctx: &egui::Context) -> Thumbnail {
        let texture_id = format!("thumbnail-{}", Uuid::new_v4());
        Thumbnail(ctx.load_texture(
            texture_id,
            egui::ColorImage::from_rgba_unmultiplied(
                [self.width() as _, self.height() as _],
                self.as_raw(),
            ),
            Default::default(),
        ))
    }
}

pub struct AppProxy {
    app: Rc<App>,
    thumbnail_size: u32,
    pub image_ids: Vec<ImageId>,
    pub face_clusters: Vec<(Uuid, Vec<Uuid>)>,
    thumbnail_storage: Storage<Thumbnail>,
    face_detection_thumbnail_receivers: HashMap<Uuid, oneshot::Receiver<AppEvent>>,
    import_thumbnail_receivers: HashMap<PathBuf, Receiver<AppEvent>>,
    import_discovery_receiver: Option<Receiver<AppEvent>>,
    import_workflow_receiver: Option<Receiver<AppEvent>>,
    face_detection_thumbnail_cache: HashMap<Uuid, DynamicImage>,
    import_thumbnail_cache: HashMap<PathBuf, DynamicImage>,
    discovered_items: Option<Vec<PathBuf>>,
}

impl AppProxy {
    pub fn new(
        gallery_dir: PathBuf,
        app_options: photos_app::config::Options,
    ) -> anyhow::Result<Self> {
        let thumbnail_size = app_options.thumbnail_sizes[0];
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
        let thumbnail_storage = Storage::<Thumbnail>::new(app.clone());

        Ok(Self {
            app,
            thumbnail_size,
            image_ids,
            face_clusters: Vec::new(),
            thumbnail_storage,
            face_detection_thumbnail_receivers: HashMap::new(),
            import_thumbnail_receivers: HashMap::new(),
            import_discovery_receiver: None,
            import_workflow_receiver: None,
            face_detection_thumbnail_cache: HashMap::new(),
            import_thumbnail_cache: HashMap::new(),
            discovered_items: None,
        })
    }

    pub fn number_of_images(&self) -> usize {
        self.image_ids.len()
    }

    pub(crate) fn get_thumbnail(
        &mut self,
        id: <Thumbnail as Storable>::Id,
        ctx: &egui::Context,
    ) -> Option<TextureHandle> {
        self.thumbnail_storage.get(id, ctx).cloned().map(|x| x.0)
    }

    pub fn request_face_detection_thumbnail(
        &mut self,
        detection_id: Uuid,
    ) -> &mut oneshot::Receiver<AppEvent> {
        if !self
            .face_detection_thumbnail_receivers
            .contains_key(&detection_id)
            && !self
                .face_detection_thumbnail_cache
                .contains_key(&detection_id)
        {
            let receiver = self
                .app
                .get_face_detection_thumbnail(detection_id, self.thumbnail_size);
            self.face_detection_thumbnail_receivers
                .insert(detection_id, receiver);
        }
        self.face_detection_thumbnail_receivers
            .get_mut(&detection_id)
            .unwrap()
    }

    pub fn get_cached_face_detection_thumbnail(&self, id: &Uuid) -> Option<&DynamicImage> {
        self.face_detection_thumbnail_cache.get(id)
    }

    pub fn request_import_thumbnail(&mut self, path: &PathBuf) -> &mut Receiver<AppEvent> {
        if !self.import_thumbnail_receivers.contains_key(path)
            && !self.import_thumbnail_cache.contains_key(path)
        {
            let receiver = self
                .app
                .get_thumbnail_from_file(path.clone(), self.thumbnail_size);
            self.import_thumbnail_receivers
                .insert(path.clone(), receiver);
        }
        self.import_thumbnail_receivers.get_mut(path).unwrap()
    }

    pub fn get_cached_import_thumbnail(&self, path: &PathBuf) -> Option<&DynamicImage> {
        self.import_thumbnail_cache.get(path)
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
        let mut completed_detection_thumbnails = Vec::new();
        for (id, receiver) in &mut self.face_detection_thumbnail_receivers {
            if let Ok(AppEvent::FaceDetectionThumbnailReady {
                detection_id,
                result,
            }) = receiver.try_recv()
            {
                if let Ok(image) = result {
                    self.face_detection_thumbnail_cache
                        .insert(detection_id, image);
                }
                completed_detection_thumbnails.push(*id);
            }
        }
        for id in completed_detection_thumbnails {
            self.face_detection_thumbnail_receivers.remove(&id);
        }

        let mut completed_import_thumbnails = Vec::new();
        for receiver in self.import_thumbnail_receivers.values_mut() {
            if let Ok(AppEvent::ThumbnailFromFileReady { path, result }) = receiver.try_recv() {
                if let Ok(image) = result {
                    self.import_thumbnail_cache.insert(path.clone(), image);
                }
                completed_import_thumbnails.push(path);
            }
        }
        for path in completed_import_thumbnails {
            self.import_thumbnail_receivers.remove(&path);
        }

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

    pub fn cancel_import_thumbnail_requests(&mut self) {
        self.import_thumbnail_receivers.clear();
        self.import_thumbnail_cache.clear();
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
