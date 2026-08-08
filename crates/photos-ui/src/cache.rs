use iced::widget::image;
use lru::LruCache;
use photos_domain::ImageId;
use std::num::NonZeroUsize;

/// Bounded LRU caches for image handles.
///
/// Two separate caches:
/// - `thumbnails`: caches thumbnail handles keyed by `(ImageId, thumbnail_size)`.
/// - `full_images`: caches full-size image handles keyed by `ImageId`.
///
/// Handles are stored as `RgbaImage` (raw pixel data). The caller converts
/// to an Iced `image::Handle` as needed via `image::Handle::from_rgba`.
pub struct ImageCache {
    thumbnails: LruCache<(ImageId, u32), image::Handle>,
    full_images: LruCache<ImageId, image::Handle>,
}

impl ImageCache {
    const THUMBNAIL_CAPACITY: usize = 512;
    const FULL_IMAGE_CAPACITY: usize = 32;

    pub fn new() -> Self {
        Self {
            thumbnails: LruCache::new(NonZeroUsize::new(Self::THUMBNAIL_CAPACITY).unwrap()),
            full_images: LruCache::new(NonZeroUsize::new(Self::FULL_IMAGE_CAPACITY).unwrap()),
        }
    }

    pub fn get_thumbnail(&mut self, id: ImageId, size: u32) -> Option<&image::Handle> {
        self.thumbnails.get(&(id, size))
    }

    /// Like `get_thumbnail` but does not update the LRU order.
    /// Useful for read-only access (e.g., in view functions).
    pub fn peek_thumbnail(&self, id: ImageId, size: u32) -> Option<&image::Handle> {
        self.thumbnails.peek(&(id, size))
    }

    pub fn insert_thumbnail(&mut self, id: ImageId, size: u32, handle: image::Handle) {
        self.thumbnails.put((id, size), handle);
    }

    pub fn contains_thumbnail(&self, id: ImageId, size: u32) -> bool {
        self.thumbnails.contains(&(id, size))
    }

    pub fn get_full(&mut self, id: ImageId) -> Option<&image::Handle> {
        self.full_images.get(&id)
    }

    pub fn insert_full(&mut self, id: ImageId, handle: image::Handle) {
        self.full_images.put(id, handle);
    }

    pub fn clear(&mut self) {
        self.thumbnails.clear();
        self.full_images.clear();
    }
}
