use std::sync::Arc;

use iced::widget::scrollable;
use photos_app::App as PhotosApp;
use photos_domain::{ImageId, RgbaImage};
use std::path::PathBuf;

// ── Initialized App (gallery/library) messages ──────────────

#[derive(Debug, Clone)]
pub enum InitializedAppMessage {
    /// Close the current library and return to welcome screen
    CloseLibrary,
    /// Gallery scroll position changed
    Scrolled(scrollable::Viewport),
    /// A thumbnail image has been loaded
    ThumbnailLoaded(ImageId, RgbaImage),
    /// User clicked a thumbnail to open in fullscreen
    OpenImage(ImageId),
    /// Close fullscreen view (escape)
    CloseImage,
    /// Navigate to next image
    NextImage,
    /// Navigate to previous image
    PreviousImage,
    /// A full-size image has been loaded
    FullImageLoaded(ImageId, RgbaImage),
}

// ── App (welcome) messages ──────────────────────────────────

pub enum AppMessage {
    /// User wants to open the library selection dialog
    SelectLibrary,
    /// Result from the library folder picker dialog
    LibrarySelected(Option<PathBuf>),
    /// Backend is ready with image IDs — transition to gallery
    LibraryReady(Arc<PhotosApp>, Vec<ImageId>),
    /// Delegate to the initialized app
    InitializedAppMessage(InitializedAppMessage),
}

impl Clone for AppMessage {
    fn clone(&self) -> Self {
        match self {
            Self::SelectLibrary => Self::SelectLibrary,
            Self::LibrarySelected(path) => Self::LibrarySelected(path.clone()),
            Self::LibraryReady(app, ids) => Self::LibraryReady(app.clone(), ids.clone()),
            Self::InitializedAppMessage(msg) => Self::InitializedAppMessage(msg.clone()),
        }
    }
}

impl std::fmt::Debug for AppMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelectLibrary => write!(f, "AppMessage::SelectLibrary"),
            Self::LibrarySelected(p) => f
                .debug_tuple("AppMessage::LibrarySelected")
                .field(p)
                .finish(),
            Self::LibraryReady(_, ids) => f
                .debug_tuple("AppMessage::LibraryReady")
                .field(&ids.len())
                .finish(),
            Self::InitializedAppMessage(msg) => f
                .debug_tuple("AppMessage::InitializedAppMessage")
                .field(msg)
                .finish(),
        }
    }
}

// ── Top-level message ──────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    AppMessage(AppMessage),
}
