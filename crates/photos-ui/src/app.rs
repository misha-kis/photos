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

/// Gallery / library view state — library is loaded.
pub struct InitializedApp {
    /// The underlying photos-app backend (shared for async tasks).
    app_backend: std::sync::Arc<photos_app::App>,
    /// Image caches.
    cache: crate::cache::ImageCache,
    /// Gallery state (image IDs, handles, cancellation tokens, fullscreen).
    gallery: crate::gallery::GalleryState,
}

impl InitializedApp {
    pub fn new(
        backend: std::sync::Arc<photos_app::App>,
        ids: Vec<photos_domain::ImageId>,
    ) -> (Self, Task<InitializedAppMessage>) {
        let cache = crate::cache::ImageCache::new();
        let mut gallery = crate::gallery::GalleryState::new(ids);

        // Set tokens for the initial visible batch (approx 4 columns × 10 rows)
        let initial_batch = 40.min(gallery.image_ids.len());
        let size = gallery.config.thumbnail_size;
        for idx in 0..initial_batch {
            if !cache.contains_thumbnail(gallery.image_ids[idx], size) {
                gallery.tokens[idx] = Some(tokio_util::sync::CancellationToken::new());
            }
        }

        let task = spawn_thumbnail_tasks(&mut gallery, &cache, &backend);

        (
            Self {
                app_backend: backend,
                cache,
                gallery,
            },
            task,
        )
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        self.gallery.subscription()
    }

    pub fn update(&mut self, message: InitializedAppMessage) -> Task<InitializedAppMessage> {
        match message {
            InitializedAppMessage::CloseLibrary => {
                self.cache.clear();
                for token in self.gallery.tokens.iter_mut().flatten() {
                    token.cancel();
                }
                Task::none()
            }
            InitializedAppMessage::Scrolled(viewport) => {
                if self.gallery.image_ids.is_empty() {
                    return Task::none();
                }
                let _ = crate::gallery::update_gallery(
                    &mut self.gallery,
                    &mut self.cache,
                    &message::InitializedAppMessage::Scrolled(viewport),
                );
                spawn_thumbnail_tasks(&mut self.gallery, &self.cache, &self.app_backend)
            }
            InitializedAppMessage::ThumbnailLoaded(id, rgba) => crate::gallery::update_gallery(
                &mut self.gallery,
                &mut self.cache,
                &message::InitializedAppMessage::ThumbnailLoaded(id, rgba),
            ),
            InitializedAppMessage::OpenImage(id) => {
                self.gallery.open_fullscreen(id);
                load_full_image(id, &mut self.cache, &self.app_backend)
            }
            InitializedAppMessage::CloseImage => {
                self.gallery.close_fullscreen();
                Task::none()
            }
            InitializedAppMessage::NextImage => {
                if let Some(next_id) = self.gallery.next_fullscreen_image() {
                    load_full_image(next_id, &mut self.cache, &self.app_backend)
                } else {
                    Task::none()
                }
            }
            InitializedAppMessage::PreviousImage => {
                if let Some(prev_id) = self.gallery.previous_fullscreen_image() {
                    load_full_image(prev_id, &mut self.cache, &self.app_backend)
                } else {
                    Task::none()
                }
            }
            InitializedAppMessage::FullImageLoaded(id, rgba) => {
                if self.gallery.fullscreen_current_id() == Some(id) {
                    let handle = iced::widget::image::Handle::from_rgba(
                        rgba.width(),
                        rgba.height(),
                        rgba.clone().into_raw(),
                    );
                    self.gallery.set_full_image_handle(id, handle.clone());
                    self.cache.insert_full(id, handle);
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, InitializedAppMessage> {
        use iced::Length;
        use iced::widget::{column, stack};

        let content = column![
            crate::gallery::gallery_top_bar("Photos"),
            crate::gallery::gallery_view(&self.gallery, &self.cache),
        ]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill);

        if let Some(overlay) = crate::fullscreen::fullscreen_view(&self.gallery.fullscreen) {
            stack![content, overlay].into()
        } else {
            content.into()
        }
    }
}

fn spawn_thumbnail_tasks(
    state: &mut crate::gallery::GalleryState,
    cache: &crate::cache::ImageCache,
    backend: &std::sync::Arc<photos_app::App>,
) -> Task<InitializedAppMessage> {
    use message::InitializedAppMessage;
    let backend = backend.clone();
    let mut tasks: Vec<Task<InitializedAppMessage>> = Vec::new();
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
            let task = Task::perform(
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
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

fn load_full_image(
    id: photos_domain::ImageId,
    _cache: &mut crate::cache::ImageCache,
    backend: &std::sync::Arc<photos_app::App>,
) -> Task<InitializedAppMessage> {
    use message::InitializedAppMessage;
    let backend = backend.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    Task::perform(
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

async fn open_library_dialog() -> Option<std::path::PathBuf> {
    let file_handle = rfd::AsyncFileDialog::new()
        .set_title("Select a photo library")
        .pick_folder()
        .await;
    file_handle.map(|fh| fh.path().to_path_buf())
}
