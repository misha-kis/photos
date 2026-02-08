use std::{num::NonZero, path::PathBuf, rc::Rc};

use eframe::egui::{self, TextureHandle, ahash::HashMap};
use image::RgbaImage;
use lru::LruCache;
use photos_app::{App, OneshotJobHandle};
use photos_domain::{ImageId, Uuid};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub(crate) trait CtxInto<T: Sized> {
    fn ctx_into(self, ctx: &egui::Context) -> T;
}

pub(crate) trait Storable: Sized {
    type Id: Eq + std::hash::Hash + Clone;
    type ReceiveAs: Sized + CtxInto<Self>;
    fn load(
        app: &App,
        id: &Self::Id,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<Self::ReceiveAs>;
}

pub(crate) struct Storage<T: Storable> {
    app: Rc<App>,
    cache: LruCache<T::Id, T>,
    jobs: HashMap<T::Id, OneshotJobHandle<T::ReceiveAs>>,
}

impl<T: Storable> Storage<T> {
    pub(crate) fn new(app: Rc<App>, lru_size: NonZero<usize>) -> Self {
        Self {
            app,
            cache: LruCache::new(lru_size),
            jobs: Default::default(),
        }
    }

    pub(crate) fn get(
        &mut self,
        id: &T::Id,
        ctx: &egui::Context,
        cancel: CancellationToken,
    ) -> Option<&T> {
        if self.cache.contains(id) {
            return self.cache.get(id);
        }

        if let Some(job) = self.jobs.get_mut(id) {
            return match job.try_recv() {
                Ok(Ok(value)) => {
                    self.jobs.remove(id);
                    self.cache.put(id.clone(), value.ctx_into(ctx));
                    self.cache.get(id)
                }
                Ok(Err(_)) | Err(oneshot::error::TryRecvError::Closed) => {
                    self.jobs.remove(id);
                    None
                }
                Err(oneshot::error::TryRecvError::Empty) => None,
            };
        }

        let job = T::load(self.app.as_ref(), id, cancel.child_token());
        self.jobs.insert(id.clone(), job);
        None
    }
}

#[derive(Clone)]
pub(crate) struct Thumbnail(pub(crate) TextureHandle);

impl Storable for Thumbnail {
    type Id = ImageId;
    type ReceiveAs = RgbaImage;

    fn load(
        app: &App,
        id: &Self::Id,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<Self::ReceiveAs> {
        app.get_thumbnail(id, 128, cancel)
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

#[derive(Clone)]
pub(crate) struct FaceThumbnail(pub(crate) TextureHandle);

impl Storable for FaceThumbnail {
    type Id = ImageId;
    type ReceiveAs = RgbaImage;

    fn load(
        app: &App,
        id: &Self::Id,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<Self::ReceiveAs> {
        app.get_face_detection_thumbnail(id, 128, cancel)
    }
}

impl CtxInto<FaceThumbnail> for RgbaImage {
    fn ctx_into(self, ctx: &egui::Context) -> FaceThumbnail {
        let texture_id = format!("face-thumbnail-{}", Uuid::new_v4());
        FaceThumbnail(ctx.load_texture(
            texture_id,
            egui::ColorImage::from_rgba_unmultiplied(
                [self.width() as _, self.height() as _],
                self.as_raw(),
            ),
            Default::default(),
        ))
    }
}

#[derive(Clone)]
pub(crate) struct ImportThumbnail(pub(crate) TextureHandle);

impl Storable for ImportThumbnail {
    type Id = PathBuf;
    type ReceiveAs = RgbaImage;

    fn load(
        app: &App,
        id: &Self::Id,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<Self::ReceiveAs> {
        app.get_thumbnail_from_file(id.clone(), 128, cancel)
    }
}

impl CtxInto<ImportThumbnail> for RgbaImage {
    fn ctx_into(self, ctx: &egui::Context) -> ImportThumbnail {
        let texture_id = format!("import-thumbnail-{}", Uuid::new_v4());
        ImportThumbnail(ctx.load_texture(
            texture_id,
            egui::ColorImage::from_rgba_unmultiplied(
                [self.width() as _, self.height() as _],
                self.as_raw(),
            ),
            Default::default(),
        ))
    }
}

pub(crate) struct FullImage(pub(crate) TextureHandle);

impl Storable for FullImage {
    type Id = (ImageId, (u32, u32));
    type ReceiveAs = RgbaImage;

    fn load(
        app: &App,
        id: &Self::Id,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<Self::ReceiveAs> {
        let (id, size) = id;
        app.get_image(*id, *size, cancel)
    }
}

impl CtxInto<FullImage> for RgbaImage {
    fn ctx_into(self, ctx: &egui::Context) -> FullImage {
        let texture_id = format!("import-thumbnail-{}", Uuid::new_v4());
        FullImage(ctx.load_texture(
            texture_id,
            egui::ColorImage::from_rgba_unmultiplied(
                [self.width() as _, self.height() as _],
                self.as_raw(),
            ),
            Default::default(),
        ))
    }
}

pub(crate) struct ImportItemPaths(pub(crate) Vec<PathBuf>);

impl Storable for ImportItemPaths {
    type Id = PathBuf;
    type ReceiveAs = Vec<PathBuf>;
    fn load(
        app: &App,
        id: &Self::Id,
        cancel: CancellationToken,
    ) -> OneshotJobHandle<Self::ReceiveAs> {
        app.discover_import_items(id.to_path_buf(), cancel)
    }
}

impl CtxInto<ImportItemPaths> for <ImportItemPaths as Storable>::ReceiveAs {
    fn ctx_into(self, _: &egui::Context) -> ImportItemPaths {
        ImportItemPaths(self)
    }
}
