use iced::widget::{image, mouse_area, opaque};
use iced::{Element, Length};
use photos_domain::ImageId;

use crate::message::Message;

/// State for the fullscreen image viewer overlay.
pub struct FullscreenState {
    /// The currently displayed image ID, if any.
    pub current_id: Option<ImageId>,
    /// The loaded full-size image handle.
    pub handle: Option<image::Handle>,
    /// Whether the fullscreen is currently shown.
    pub is_open: bool,
}

impl FullscreenState {
    pub fn new() -> Self {
        Self {
            current_id: None,
            handle: None,
            is_open: false,
        }
    }

    /// Opens the fullscreen view for the given image ID.
    pub fn open(&mut self, id: ImageId) {
        self.current_id = Some(id);
        self.handle = None;
        self.is_open = true;
    }

    /// Closes the fullscreen view.
    pub fn close(&mut self) {
        self.is_open = false;
        self.current_id = None;
        self.handle = None;
    }
}

/// Renders the fullscreen overlay, or returns `None` if not visible.
pub fn fullscreen_view(state: &FullscreenState) -> Option<Element<'_, Message>> {
    if !state.is_open {
        return None;
    }

    let content: Element<'_, Message> = if let Some(handle) = &state.handle {
        image::viewer(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        iced::widget::text("Loading...")
            .size(32)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };

    let overlay = opaque(
        mouse_area(iced::widget::container(content).center(Length::Fill).style(
            |_theme: &iced::Theme| {
                iced::widget::container::Style::default().background(iced::color!(0x000000))
            },
        ))
        .on_press(Message::CloseImage),
    );

    Some(overlay.into())
}
