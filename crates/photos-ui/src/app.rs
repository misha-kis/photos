use iced::Task;
use message::{AppMessage, InitializedAppMessage, Message};

use crate::message;

/// Welcome screen state — no library loaded yet.
/// Owns an optional `InitializedApp` for when the gallery is active.
pub struct App {
    initialized: Option<InitializedApp>,
}

impl App {
    pub fn new() -> Self {
        Self { initialized: None }
    }

    pub fn title(&self) -> String {
        "Photos".to_string()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        match &self.initialized {
            Some(child) => child.subscription(),
            None => iced::Subscription::none(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AppMessage(app_msg) => self.update_app(app_msg),
        }
    }

    fn update_app(&mut self, message: AppMessage) -> Task<Message> {
        match message {
            AppMessage::SelectLibrary => Task::perform(open_library_dialog(), |path| {
                Message::AppMessage(AppMessage::LibrarySelected(path))
            }),
            AppMessage::LibrarySelected(Some(path)) => {
                let handle = tokio::runtime::Handle::current();
                let app_options = photos_app::config::Options::default();
                let path_clone = path.clone();

                Task::perform(
                    async move {
                        let backend = photos_app::App::new(path_clone, app_options, handle)
                            .await
                            .map_err(|e| format!("{e}"))?;

                        let cancel = tokio_util::sync::CancellationToken::new();
                        let items = backend
                            .discover_import_items_async(path.join("imports"), cancel)
                            .await
                            .map_err(|e| format!("{e}"))?;
                        let _handle = backend.import_items(items).await;

                        let ids = backend
                            .get_image_ids_async()
                            .await
                            .map_err(|e| format!("{e}"))?;

                        Ok::<_, String>((std::sync::Arc::new(backend), ids))
                    },
                    |result| match result {
                        Ok((backend, ids)) => {
                            Message::AppMessage(AppMessage::LibraryReady(backend, ids))
                        }
                        Err(e) => {
                            tracing::error!("Failed to open library: {e}");
                            Message::AppMessage(AppMessage::SelectLibrary)
                        }
                    },
                )
            }
            AppMessage::LibrarySelected(None) => Task::none(),
            AppMessage::LibraryReady(backend, ids) => {
                let (child, task) = InitializedApp::new(backend, ids);
                let mapped =
                    task.map(|msg| Message::AppMessage(AppMessage::InitializedAppMessage(msg)));
                self.initialized = Some(child);
                mapped
            }
            AppMessage::InitializedAppMessage(sub) => match sub {
                InitializedAppMessage::CloseLibrary => {
                    self.initialized = None;
                    Task::none()
                }
                other => {
                    if let Some(child) = &mut self.initialized {
                        child
                            .update(other)
                            .map(|msg| Message::AppMessage(AppMessage::InitializedAppMessage(msg)))
                    } else {
                        Task::none()
                    }
                }
            },
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message> {
        match &self.initialized {
            Some(child) => {
                use crate::message::AppMessage;
                child
                    .view()
                    .map(|msg| Message::AppMessage(AppMessage::InitializedAppMessage(msg)))
            }
            None => crate::library_dialog::welcome_view(),
        }
    }
}

/// Possible states for an initialized app.
pub enum InitializedAppState {
    /// Gallery view displaying images in a grid.
    Gallery(crate::gallery::GalleryState),
}

impl InitializedAppState {
    /// Cancel all in-flight tasks for the current state.
    pub fn cancel_all_tasks(&mut self) {
        match self {
            InitializedAppState::Gallery(gallery) => gallery.cancel_all_tasks(),
        }
    }

    /// Handle a message for the current state.
    pub fn update(
        &mut self,
        message: InitializedAppMessage,
        cache: &mut crate::cache::ImageCache,
        backend: &std::sync::Arc<photos_app::App>,
    ) -> iced::Task<InitializedAppMessage> {
        match self {
            InitializedAppState::Gallery(gallery) => gallery.update(message, cache, backend),
        }
    }

    /// Render the current state.
    pub fn view(
        &self,
        cache: &crate::cache::ImageCache,
    ) -> iced::Element<'_, InitializedAppMessage> {
        match self {
            InitializedAppState::Gallery(gallery) => gallery.view(cache),
        }
    }
}

/// Gallery / library view state — library is loaded.
pub struct InitializedApp {
    /// The underlying photos-app backend (shared for async tasks).
    app_backend: std::sync::Arc<photos_app::App>,
    /// Image caches.
    cache: crate::cache::ImageCache,
    /// Current app state.
    state: InitializedAppState,
}

impl InitializedApp {
    pub fn new(
        backend: std::sync::Arc<photos_app::App>,
        ids: Vec<photos_domain::ImageId>,
    ) -> (Self, Task<InitializedAppMessage>) {
        let cache = crate::cache::ImageCache::new();
        let mut gallery = crate::gallery::GalleryState::new(ids);

        let task = gallery.spawn_initial_tasks(&cache, &backend);

        (
            Self {
                app_backend: backend,
                cache,
                state: InitializedAppState::Gallery(gallery),
            },
            task,
        )
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        match &self.state {
            InitializedAppState::Gallery(gallery) => gallery.subscription(),
        }
    }

    pub fn update(&mut self, message: InitializedAppMessage) -> Task<InitializedAppMessage> {
        match message {
            InitializedAppMessage::CloseLibrary => {
                self.cache.clear();
                self.state.cancel_all_tasks();
                Task::none()
            }
            other => self.state.update(other, &mut self.cache, &self.app_backend),
        }
    }

    pub fn view(&self) -> iced::Element<'_, InitializedAppMessage> {
        self.state.view(&self.cache)
    }
}

async fn open_library_dialog() -> Option<std::path::PathBuf> {
    let file_handle = rfd::AsyncFileDialog::new()
        .set_title("Select a photo library")
        .pick_folder()
        .await;
    file_handle.map(|fh| fh.path().to_path_buf())
}
