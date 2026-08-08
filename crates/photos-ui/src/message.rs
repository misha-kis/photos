use std::sync::Arc;

use iced::widget::scrollable;
use photos_app::App as PhotosApp;
use photos_domain::{ImageId, RgbaImage};
use std::path::PathBuf;

pub enum Message {
    /// User wants to open the library selection dialog
    SelectLibrary,
    /// Result from the library folder picker dialog
    LibrarySelected(Option<PathBuf>),
    /// Backend is ready with image IDs
    LibraryReady(Arc<PhotosApp>, Vec<ImageId>),
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

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelectLibrary => write!(f, "SelectLibrary"),
            Self::LibrarySelected(path) => f.debug_tuple("LibrarySelected").field(path).finish(),
            Self::LibraryReady(_, ids) => f.debug_tuple("LibraryReady").field(&ids.len()).finish(),
            Self::CloseLibrary => write!(f, "CloseLibrary"),
            Self::Scrolled(vp) => f.debug_tuple("Scrolled").field(vp).finish(),
            Self::ThumbnailLoaded(id, _) => f.debug_tuple("ThumbnailLoaded").field(id).finish(),
            Self::OpenImage(id) => f.debug_tuple("OpenImage").field(id).finish(),
            Self::CloseImage => write!(f, "CloseImage"),
            Self::NextImage => write!(f, "NextImage"),
            Self::PreviousImage => write!(f, "PreviousImage"),
            Self::FullImageLoaded(id, _) => f.debug_tuple("FullImageLoaded").field(id).finish(),
        }
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Self::SelectLibrary => Self::SelectLibrary,
            Self::LibrarySelected(path) => Self::LibrarySelected(path.clone()),
            Self::LibraryReady(app, ids) => Self::LibraryReady(app.clone(), ids.clone()),
            Self::CloseLibrary => Self::CloseLibrary,
            Self::Scrolled(viewport) => Self::Scrolled(*viewport),
            Self::ThumbnailLoaded(id, rgba) => Self::ThumbnailLoaded(*id, rgba.clone()),
            Self::OpenImage(id) => Self::OpenImage(*id),
            Self::CloseImage => Self::CloseImage,
            Self::NextImage => Self::NextImage,
            Self::PreviousImage => Self::PreviousImage,
            Self::FullImageLoaded(id, rgba) => Self::FullImageLoaded(*id, rgba.clone()),
        }
    }
}
