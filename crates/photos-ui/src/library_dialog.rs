use iced::widget::{button, container, text};
use iced::{Element, Length};

use crate::message::{AppMessage, Message};

/// The "no library selected" welcome screen with a single button
/// to open the folder picker.
pub fn welcome_view() -> Element<'static, Message> {
    container(
        button(
            text("Select Library")
                .size(24)
                .width(Length::Shrink)
                .height(Length::Shrink),
        )
        .on_press(Message::AppMessage(AppMessage::SelectLibrary))
        .padding(16),
    )
    .center(Length::Fill)
    .into()
}
