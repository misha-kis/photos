use iced::widget::{Row, button, container, grid, image, mouse_area, row, scrollable, text};
use iced::{Element, Length};
use photos_domain::ImageId;
use tokio_util::sync::CancellationToken;

use crate::cache::ImageCache;
use crate::fullscreen::FullscreenState;
use crate::message::InitializedAppMessage as Message;

/// Configuration for the gallery view.
#[derive(Debug, Clone)]
pub struct GalleryConfig {
    /// The size (width and height) of each thumbnail in pixels.
    pub thumbnail_size: u32,
    /// Number of rows beyond the viewport to prefetch in each direction.
    pub prefetch_rows: usize,
}

impl Default for GalleryConfig {
    fn default() -> Self {
        Self {
            thumbnail_size: 128,
            prefetch_rows: 5,
        }
    }
}

/// State for the gallery view.
///
/// All three vectors are kept parallel by index so we can efficiently
/// map from a visual position to its metadata.
pub struct GalleryState {
    /// Ordered list of image IDs in the gallery.
    pub image_ids: Vec<ImageId>,
    /// Loaded thumbnail handles, parallel to `image_ids`.
    pub handles: Vec<Option<iced::widget::image::Handle>>,
    /// Cancellation tokens for in-flight thumbnail requests.
    /// `None` means no request is in-flight for this position.
    pub tokens: Vec<Option<CancellationToken>>,
    /// The last known viewport (used to detect scroll changes).
    pub last_viewport: Option<scrollable::Viewport>,
    /// Gallery configuration.
    pub config: GalleryConfig,
    /// Fullscreen overlay state.
    pub fullscreen: FullscreenState,
}

impl GalleryState {
    pub fn new(image_ids: Vec<ImageId>) -> Self {
        let count = image_ids.len();
        Self {
            image_ids,
            handles: vec![None; count],
            tokens: vec![None; count],
            last_viewport: None,
            config: GalleryConfig::default(),
            fullscreen: FullscreenState::new(),
        }
    }

    /// Seed initial cancellation tokens for the first visible batch and spawn
    /// thumbnail fetch tasks. Returns the batch of tasks to run.
    pub fn spawn_initial_tasks(
        &mut self,
        cache: &crate::cache::ImageCache,
        backend: &std::sync::Arc<photos_app::App>,
    ) -> iced::Task<crate::message::InitializedAppMessage> {
        // Set tokens for the initial visible batch (approx 4 columns × 10 rows)
        let initial_batch = 40.min(self.image_ids.len());
        let size = self.config.thumbnail_size;
        for idx in 0..initial_batch {
            if !cache.contains_thumbnail(self.image_ids[idx], size) {
                self.tokens[idx] = Some(CancellationToken::new());
            }
        }

        spawn_thumbnail_tasks(self, cache, backend)
    }

    pub fn is_empty(&self) -> bool {
        self.image_ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.image_ids.len()
    }

    /// Returns the keyboard subscription for fullscreen navigation.
    pub fn subscription(&self) -> iced::Subscription<crate::message::Message> {
        use crate::message::{AppMessage, Message};

        if self.fullscreen.is_open {
            iced::keyboard::listen().map(|event| match event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                } => Message::AppMessage(AppMessage::InitializedAppMessage(
                    crate::message::InitializedAppMessage::CloseImage,
                )),
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft),
                    ..
                } => Message::AppMessage(AppMessage::InitializedAppMessage(
                    crate::message::InitializedAppMessage::PreviousImage,
                )),
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight),
                    ..
                } => Message::AppMessage(AppMessage::InitializedAppMessage(
                    crate::message::InitializedAppMessage::NextImage,
                )),
                _ => Message::AppMessage(AppMessage::InitializedAppMessage(
                    crate::message::InitializedAppMessage::CloseImage,
                )),
            })
        } else {
            iced::Subscription::none()
        }
    }

    /// Open the fullscreen overlay for the given image ID.
    pub fn open_fullscreen(&mut self, id: ImageId) {
        self.fullscreen.open(id);
    }

    /// Close the fullscreen overlay.
    pub fn close_fullscreen(&mut self) {
        self.fullscreen.close();
    }

    /// Navigate to the next image in fullscreen, returning its ID if successful.
    pub fn next_fullscreen_image(&mut self) -> Option<ImageId> {
        let current_id = self.fullscreen.current_id?;
        let idx = self.image_ids.iter().position(|id| *id == current_id)?;
        if idx + 1 < self.image_ids.len() {
            let next_id = self.image_ids[idx + 1];
            self.fullscreen.open(next_id);
            Some(next_id)
        } else {
            None
        }
    }

    /// Navigate to the previous image in fullscreen, returning its ID if successful.
    pub fn previous_fullscreen_image(&mut self) -> Option<ImageId> {
        let current_id = self.fullscreen.current_id?;
        let idx = self.image_ids.iter().position(|id| *id == current_id)?;
        if idx > 0 {
            let prev_id = self.image_ids[idx - 1];
            self.fullscreen.open(prev_id);
            Some(prev_id)
        } else {
            None
        }
    }

    /// Set the full-image handle for the currently-displayed fullscreen image.
    pub fn set_full_image_handle(&mut self, id: ImageId, handle: iced::widget::image::Handle) {
        if self.fullscreen.current_id == Some(id) {
            self.fullscreen.handle = Some(handle);
        }
    }

    /// Whether the fullscreen overlay is currently open.
    pub fn is_fullscreen_open(&self) -> bool {
        self.fullscreen.is_open
    }

    /// The currently-displayed fullscreen image ID.
    pub fn fullscreen_current_id(&self) -> Option<ImageId> {
        self.fullscreen.current_id
    }
}

/// Renders the gallery view.
pub fn gallery_view(state: &GalleryState, cache: &ImageCache) -> Element<'static, Message> {
    let thumbnail_size = state.config.thumbnail_size;
    let total = state.image_ids.len();
    let mut items: Vec<Element<'static, Message>> = Vec::with_capacity(total);

    for (idx, image_id) in state.image_ids.iter().enumerate() {
        let thumbnail: Element<'static, Message> =
            if let Some(handle) = cache.peek_thumbnail(*image_id, thumbnail_size) {
                image(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else if let Some(handle) = &state.handles[idx] {
                image(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            } else {
                // Placeholder while loading
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|theme: &iced::Theme| {
                        container::Style::default()
                            .border(iced::border::rounded(4).color(theme.palette().background))
                    })
                    .into()
            };

        let card = mouse_area(thumbnail).on_press(Message::OpenImage(*image_id));
        items.push(card.into());
    }

    let item_width = thumbnail_size as f32;
    let grid = grid(items).fluid(item_width).spacing(4);

    let content = scrollable(grid)
        .on_scroll(|viewport| Message::Scrolled(viewport))
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill);

    content.into()
}

/// Top bar for the gallery view.
pub fn gallery_top_bar(title: &str) -> Row<'_, Message> {
    row![
        text(title).size(18).width(Length::Fill),
        button("Close Library").on_press(Message::CloseLibrary),
    ]
    .spacing(8)
    .padding(8)
}

/// Handle gallery-related messages and return tasks.
pub fn update_gallery(
    state: &mut GalleryState,
    cache: &mut ImageCache,
    message: &Message,
) -> iced::Task<Message> {
    match message {
        Message::Scrolled(viewport) => on_scroll(state, cache, viewport),
        Message::ThumbnailLoaded(id, rgba) => {
            if let Some(idx) = state.image_ids.iter().position(|iid| iid == id) {
                let handle = iced::widget::image::Handle::from_rgba(
                    rgba.width(),
                    rgba.height(),
                    rgba.clone().into_raw(),
                );
                state.handles[idx] = Some(handle.clone());
                state.tokens[idx] = None;
                cache.insert_thumbnail(*id, state.config.thumbnail_size, handle);
            }
            iced::Task::none()
        }
        _ => iced::Task::none(),
    }
}

// ── Methods moved from app.rs ────────────────────────────────

impl GalleryState {
    /// Cancel all in-flight thumbnail tasks.
    pub fn cancel_all_tasks(&mut self) {
        for token in self.tokens.iter_mut().flatten() {
            token.cancel();
        }
    }

    /// Handle a message that may need access to the cache and backend.
    pub fn update(
        &mut self,
        message: crate::message::InitializedAppMessage,
        cache: &mut crate::cache::ImageCache,
        backend: &std::sync::Arc<photos_app::App>,
    ) -> iced::Task<crate::message::InitializedAppMessage> {
        match message {
            crate::message::InitializedAppMessage::Scrolled(viewport) => {
                if self.image_ids.is_empty() {
                    return iced::Task::none();
                }
                let _ = crate::gallery::update_gallery(
                    self,
                    cache,
                    &crate::message::InitializedAppMessage::Scrolled(viewport),
                );
                spawn_thumbnail_tasks(self, cache, backend)
            }
            crate::message::InitializedAppMessage::ThumbnailLoaded(id, rgba) => {
                crate::gallery::update_gallery(
                    self,
                    cache,
                    &crate::message::InitializedAppMessage::ThumbnailLoaded(id, rgba),
                )
            }
            crate::message::InitializedAppMessage::OpenImage(id) => {
                self.open_fullscreen(id);
                load_full_image(id, cache, backend)
            }
            crate::message::InitializedAppMessage::CloseImage => {
                self.close_fullscreen();
                iced::Task::none()
            }
            crate::message::InitializedAppMessage::NextImage => {
                if let Some(next_id) = self.next_fullscreen_image() {
                    load_full_image(next_id, cache, backend)
                } else {
                    iced::Task::none()
                }
            }
            crate::message::InitializedAppMessage::PreviousImage => {
                if let Some(prev_id) = self.previous_fullscreen_image() {
                    load_full_image(prev_id, cache, backend)
                } else {
                    iced::Task::none()
                }
            }
            crate::message::InitializedAppMessage::FullImageLoaded(id, rgba) => {
                if self.fullscreen_current_id() == Some(id) {
                    let handle = iced::widget::image::Handle::from_rgba(
                        rgba.width(),
                        rgba.height(),
                        rgba.clone().into_raw(),
                    );
                    self.set_full_image_handle(id, handle.clone());
                    cache.insert_full(id, handle);
                }
                iced::Task::none()
            }
            crate::message::InitializedAppMessage::CloseLibrary => unreachable!(),
        }
    }

    /// Render the gallery view.
    pub fn view(
        &self,
        cache: &crate::cache::ImageCache,
    ) -> iced::Element<'_, crate::message::InitializedAppMessage> {
        use iced::Length;
        use iced::widget::{column, stack};

        let content = column![
            crate::gallery::gallery_top_bar("Photos"),
            crate::gallery::gallery_view(self, cache),
        ]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill);

        if let Some(overlay) = crate::fullscreen::fullscreen_view(&self.fullscreen) {
            stack![content, overlay].into()
        } else {
            content.into()
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────

fn spawn_thumbnail_tasks(
    state: &mut GalleryState,
    cache: &ImageCache,
    backend: &std::sync::Arc<photos_app::App>,
) -> iced::Task<crate::message::InitializedAppMessage> {
    use crate::message::InitializedAppMessage;
    let backend = backend.clone();
    let mut tasks: Vec<iced::Task<InitializedAppMessage>> = Vec::new();
    let size = state.config.thumbnail_size;

    for idx in 0..state.image_ids.len() {
        let image_id = state.image_ids[idx];

        if cache.contains_thumbnail(image_id, size) {
            state.tokens[idx] = None;
            continue;
        }

        if let Some(token) = state.tokens[idx].as_ref() {
            if token.is_cancelled() {
                state.tokens[idx] = None;
                continue;
            }

            let token = token.child_token();
            let backend = backend.clone();
            let task = iced::Task::perform(
                async move {
                    let result = backend.get_thumbnail_async(image_id, size, token).await;
                    result.map(|rgba| (image_id, rgba))
                },
                |result| match result {
                    Ok((id, rgba)) => InitializedAppMessage::ThumbnailLoaded(id, rgba),
                    Err(_) => InitializedAppMessage::CloseImage,
                },
            );
            tasks.push(task);
        }
    }

    if tasks.is_empty() {
        iced::Task::none()
    } else {
        iced::Task::batch(tasks)
    }
}

fn load_full_image(
    id: photos_domain::ImageId,
    _cache: &mut crate::cache::ImageCache,
    backend: &std::sync::Arc<photos_app::App>,
) -> iced::Task<crate::message::InitializedAppMessage> {
    use crate::message::InitializedAppMessage;
    let backend = backend.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    iced::Task::perform(
        async move {
            let result = backend.get_image_async(id, None, cancel).await;
            result.map(|rgba| (id, rgba))
        },
        |result| match result {
            Ok((id, rgba)) => InitializedAppMessage::FullImageLoaded(id, rgba),
            Err(_) => InitializedAppMessage::CloseImage,
        },
    )
}

/// Called when the user scrolls. Cancels out-of-range requests and
/// starts new ones for the visible + prefetch window.
fn on_scroll(
    state: &mut GalleryState,
    cache: &ImageCache,
    viewport: &scrollable::Viewport,
) -> iced::Task<Message> {
    // Estimate visible rows from viewport bounds
    let bounds = viewport.bounds();
    let viewport_y = bounds.y;
    let viewport_height = bounds.height;

    // Rough estimate of item pitch (thumbnail_size + spacing)
    let item_pitch = state.config.thumbnail_size as f32 + 4.0;
    let total_items = state.image_ids.len() as f32;
    // Estimate number of columns (used to compute approximate content height)
    let est_columns = 4.0;
    let total_height = (total_items / est_columns).ceil() * item_pitch;

    if total_height <= 0.0 {
        return iced::Task::none();
    }

    let first_idx_ratio = (viewport_y / total_height).clamp(0.0, 1.0);
    let last_idx_ratio = ((viewport_y + viewport_height) / total_height).clamp(0.0, 1.0);

    let first_idx = (first_idx_ratio * total_items) as usize;
    let last_idx = ((last_idx_ratio * total_items) as usize).min(state.image_ids.len());

    // Expand by prefetch rows
    let prefetch = state.config.prefetch_rows;
    let prefetch_items = (prefetch as f32 * est_columns) as usize;
    let first_idx = first_idx.saturating_sub(prefetch_items);
    let last_idx = (last_idx.saturating_add(prefetch_items)).min(state.image_ids.len());

    // Cancel tokens outside the prefetch window
    for idx in 0..state.image_ids.len() {
        if idx < first_idx || idx >= last_idx {
            if let Some(token) = state.tokens[idx].take() {
                token.cancel();
            }
        }
    }

    // Create tokens for images inside the prefetch window that aren't cached
    // and don't already have a request in-flight
    let size = state.config.thumbnail_size;
    for idx in first_idx..last_idx {
        if state.tokens[idx].is_some() {
            continue; // already loading
        }
        if cache.peek_thumbnail(state.image_ids[idx], size).is_some() {
            continue; // already cached
        }
        state.tokens[idx] = Some(CancellationToken::new());
    }

    // The actual task spawning happens in app.rs which has access to the backend.
    iced::Task::none()
}
