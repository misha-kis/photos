use iced::widget::{Row, button, container, grid, image, mouse_area, row, scrollable, text};
use iced::{Element, Length};
use photos_domain::ImageId;
use tokio_util::sync::CancellationToken;

use crate::cache::ImageCache;
use crate::message::Message;

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
        }
    }

    pub fn is_empty(&self) -> bool {
        self.image_ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.image_ids.len()
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
