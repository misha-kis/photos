use image::DynamicImage;
use photos_app::AppEvent;
use photos_core::Uuid;
use photos_domain::ImageId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot;

pub struct AppProxy {
    app: Arc<photos_app::App>,
    thumbnail_size: u32,
    pub image_ids: Vec<ImageId>,
    pub face_ids: Vec<Uuid>,
    /// (cluster_face_uuid, detection_uuids in that cluster)
    pub face_clusters: Vec<(Uuid, Vec<Uuid>)>,
    face_thumbnail_receivers: HashMap<ImageId, oneshot::Receiver<AppEvent>>,
    face_detection_thumbnail_receivers: HashMap<Uuid, oneshot::Receiver<AppEvent>>,
    thumbnail_receivers: HashMap<ImageId, Receiver<AppEvent>>,
    import_thumbnail_receivers: HashMap<PathBuf, Receiver<AppEvent>>,
    import_discovery_receiver: Option<Receiver<AppEvent>>,
    import_workflow_receiver: Option<Receiver<AppEvent>>,
    thumbnail_cache: HashMap<ImageId, DynamicImage>,
    face_thumbnail_cache: HashMap<ImageId, DynamicImage>,
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
        let app = Arc::new(photos_app::App::new(gallery_dir, app_options)?);

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
            app,
            thumbnail_size,
            image_ids,
            face_ids: Vec::new(),
            face_clusters: Vec::new(),
            thumbnail_receivers: HashMap::new(),
            face_thumbnail_receivers: HashMap::new(),
            face_detection_thumbnail_receivers: HashMap::new(),
            import_thumbnail_receivers: HashMap::new(),
            import_discovery_receiver: None,
            import_workflow_receiver: None,
            thumbnail_cache: HashMap::new(),
            face_thumbnail_cache: HashMap::new(),
            face_detection_thumbnail_cache: HashMap::new(),
            import_thumbnail_cache: HashMap::new(),
            discovered_items: None,
        })
    }

    pub fn number_of_images(&self) -> usize {
        self.image_ids.len()
    }

    pub fn request_thumbnail(&mut self, id: ImageId) -> &mut Receiver<AppEvent> {
        if !self.thumbnail_receivers.contains_key(&id) && !self.thumbnail_cache.contains_key(&id) {
            let receiver = self.app.get_thumbnail(id, self.thumbnail_size);
            self.thumbnail_receivers.insert(id, receiver);
        }
        self.thumbnail_receivers.get_mut(&id).unwrap()
    }

    pub fn request_face_thumbnail(&mut self, id: Uuid) -> &mut oneshot::Receiver<AppEvent> {
        if !self.face_thumbnail_receivers.contains_key(&id)
            && !self.face_thumbnail_cache.contains_key(&id)
        {
            let receiver = self.app.get_face_thumbnail(id, self.thumbnail_size);
            self.face_thumbnail_receivers.insert(id, receiver);
        }
        self.face_thumbnail_receivers.get_mut(&id).unwrap()
    }

    pub fn get_cached_thumbnail(&self, id: &ImageId) -> Option<&DynamicImage> {
        self.thumbnail_cache.get(id)
    }

    pub fn get_cached_face_thumbnail(&self, id: &ImageId) -> Option<&DynamicImage> {
        self.face_thumbnail_cache.get(id)
    }

    pub fn request_face_detection_thumbnail(&mut self, detection_id: Uuid) -> &mut oneshot::Receiver<AppEvent> {
        if !self.face_detection_thumbnail_receivers.contains_key(&detection_id)
            && !self.face_detection_thumbnail_cache.contains_key(&detection_id)
        {
            let receiver = self
                .app
                .get_face_detection_thumbnail(detection_id, self.thumbnail_size);
            self.face_detection_thumbnail_receivers.insert(detection_id, receiver);
        }
        self.face_detection_thumbnail_receivers.get_mut(&detection_id).unwrap()
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
        let mut completed_thumbnails = Vec::new();
        for (id, receiver) in &mut self.thumbnail_receivers {
            if let Ok(AppEvent::ThumbnailReady { image_id, result }) = receiver.try_recv() {
                if let Ok(image) = result {
                    self.thumbnail_cache.insert(image_id, image);
                }
                completed_thumbnails.push(*id);
            }
        }
        for id in completed_thumbnails {
            self.thumbnail_receivers.remove(&id);
        }

        let mut completed_face_thumbnails = Vec::new();
        for (id, receiver) in &mut self.face_thumbnail_receivers {
            if let Ok(AppEvent::FaceThumbnailReady { face_id, result }) = receiver.try_recv() {
                if let Ok(image) = result {
                    self.face_thumbnail_cache.insert(face_id, image);
                }
                completed_face_thumbnails.push(*id);
            }
        }
        for id in completed_face_thumbnails {
            self.face_thumbnail_receivers.remove(&id);
        }

        let mut completed_detection_thumbnails = Vec::new();
        for (id, receiver) in &mut self.face_detection_thumbnail_receivers {
            if let Ok(AppEvent::FaceDetectionThumbnailReady { detection_id, result }) = receiver.try_recv() {
                if let Ok(image) = result {
                    self.face_detection_thumbnail_cache.insert(detection_id, image);
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

    pub fn refresh_face_ids(&mut self) {
        let receiver = self.app.get_face_ids();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Ok(AppEvent::FaceIdsReady { result }) = receiver.await
                && let Ok(ids) = result
            {
                self.face_ids = ids;
            }
        })
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
