/// The main application state.
///
/// Constructed synchronously for Iced's boot cycle.
/// The backend is created when the user selects a library.
pub struct App {
    /// The underlying photos-app backend (shared for async tasks).
    /// `None` until the user selects a library.
    app_backend: Option<std::sync::Arc<photos_app::App>>,
    /// Image caches.
    cache: crate::cache::ImageCache,
    /// Current screen.
    screen: Screen,
    /// Fullscreen overlay state.
    fullscreen: crate::fullscreen::FullscreenState,
}

enum Screen {
    Welcome,
    Gallery(crate::gallery::GalleryState),
}

impl App {
    pub fn new() -> Self {
        Self {
            app_backend: None,
            cache: crate::cache::ImageCache::new(),
            screen: Screen::Welcome,
            fullscreen: crate::fullscreen::FullscreenState::new(),
        }
    }

    pub fn title(&self) -> String {
        "Photos".to_string()
    }

    pub fn subscription(&self) -> iced::Subscription<crate::message::Message> {
        use crate::message::Message;
        if self.fullscreen.is_open {
            iced::keyboard::listen().map(|event| match event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                } => Message::CloseImage,
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft),
                    ..
                } => Message::PreviousImage,
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight),
                    ..
                } => Message::NextImage,
                _ => Message::CloseImage,
            })
        } else {
            iced::Subscription::none()
        }
    }

    pub fn update(
        &mut self,
        message: crate::message::Message,
    ) -> iced::Task<crate::message::Message> {
        use crate::message::Message;
        match message {
            Message::SelectLibrary => {
                iced::Task::perform(open_library_dialog(), Message::LibrarySelected)
            }
            Message::LibrarySelected(Some(path)) => {
                let handle = tokio::runtime::Handle::current();
                let app_options = photos_app::config::Options::default();
                let path_clone = path.clone();

                iced::Task::perform(
                    async move {
                        let backend = photos_app::App::new(path_clone, app_options, handle)
                            .await
                            .map_err(|e| format!("{e}"))?;

                        // Discover import items
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
                        Ok((backend, ids)) => Message::LibraryReady(backend, ids),
                        Err(e) => {
                            tracing::error!("Failed to open library: {e}");
                            Message::CloseLibrary
                        }
                    },
                )
            }
            Message::LibrarySelected(None) => iced::Task::none(),
            Message::LibraryReady(backend, ids) => {
                self.app_backend = Some(backend);
                let mut gallery = crate::gallery::GalleryState::new(ids);

                // Set tokens for the initial visible batch (approx 4 columns × 10 rows)
                // so spawn_thumbnail_tasks picks them up.
                let initial_batch = 40.min(gallery.image_ids.len());
                let size = gallery.config.thumbnail_size;
                for idx in 0..initial_batch {
                    if !self.cache.contains_thumbnail(gallery.image_ids[idx], size) {
                        gallery.tokens[idx] = Some(tokio_util::sync::CancellationToken::new());
                    }
                }

                let task = if let Some(ref backend) = self.app_backend {
                    spawn_thumbnail_tasks(&mut gallery, &self.cache, backend)
                } else {
                    iced::Task::none()
                };

                self.screen = Screen::Gallery(gallery);
                task
            }
            Message::CloseLibrary => {
                self.cache.clear();
                if let Screen::Gallery(gallery) = &mut self.screen {
                    for token in gallery.tokens.iter_mut().flatten() {
                        token.cancel();
                    }
                }
                self.screen = Screen::Welcome;
                self.app_backend = None;
                iced::Task::none()
            }
            Message::Scrolled(viewport) => {
                let gallery = match &mut self.screen {
                    Screen::Gallery(g) => g,
                    _ => return iced::Task::none(),
                };
                if gallery.image_ids.is_empty() {
                    return iced::Task::none();
                }
                if let Some(ref backend) = self.app_backend {
                    let _ = crate::gallery::update_gallery(
                        gallery,
                        &mut self.cache,
                        &Message::Scrolled(viewport),
                    );
                    return spawn_thumbnail_tasks(gallery, &self.cache, backend);
                } else {
                    iced::Task::none()
                }
            }
            Message::ThumbnailLoaded(id, rgba) => {
                if let Screen::Gallery(gallery) = &mut self.screen {
                    crate::gallery::update_gallery(
                        gallery,
                        &mut self.cache,
                        &Message::ThumbnailLoaded(id, rgba),
                    )
                } else {
                    iced::Task::none()
                }
            }
            Message::OpenImage(id) => {
                self.fullscreen.open(id);
                if let Some(ref backend) = self.app_backend {
                    load_full_image(id, &mut self.cache, backend)
                } else {
                    iced::Task::none()
                }
            }
            Message::CloseImage => {
                self.fullscreen.close();
                iced::Task::none()
            }
            Message::NextImage => {
                let image_ids = match &self.screen {
                    Screen::Gallery(g) => &g.image_ids,
                    _ => return iced::Task::none(),
                };
                if let Some(current_id) = self.fullscreen.current_id {
                    if let Some(idx) = image_ids.iter().position(|id| *id == current_id) {
                        if idx + 1 < image_ids.len() {
                            let next_id = image_ids[idx + 1];
                            self.fullscreen.open(next_id);
                            if let Some(ref backend) = self.app_backend {
                                return load_full_image(next_id, &mut self.cache, backend);
                            }
                        }
                    }
                }
                iced::Task::none()
            }
            Message::PreviousImage => {
                let image_ids = match &self.screen {
                    Screen::Gallery(g) => &g.image_ids,
                    _ => return iced::Task::none(),
                };
                if let Some(current_id) = self.fullscreen.current_id {
                    if let Some(idx) = image_ids.iter().position(|id| *id == current_id) {
                        if idx > 0 {
                            let prev_id = image_ids[idx - 1];
                            self.fullscreen.open(prev_id);
                            if let Some(ref backend) = self.app_backend {
                                return load_full_image(prev_id, &mut self.cache, backend);
                            }
                        }
                    }
                }
                iced::Task::none()
            }
            Message::FullImageLoaded(id, rgba) => {
                if self.fullscreen.current_id == Some(id) {
                    let handle = iced::widget::image::Handle::from_rgba(
                        rgba.width(),
                        rgba.height(),
                        rgba.clone().into_raw(),
                    );
                    self.fullscreen.handle = Some(handle.clone());
                    self.cache.insert_full(id, handle);
                }
                iced::Task::none()
            }
        }
    }

    pub fn view(&self) -> iced::Element<'_, crate::message::Message> {
        use iced::Length;
        use iced::widget::{column, stack};

        match &self.screen {
            Screen::Welcome => crate::library_dialog::welcome_view(),
            Screen::Gallery(gallery) => {
                let content = column![
                    crate::gallery::gallery_top_bar("Photos"),
                    crate::gallery::gallery_view(gallery, &self.cache),
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
    }
}

fn spawn_thumbnail_tasks(
    state: &mut crate::gallery::GalleryState,
    cache: &crate::cache::ImageCache,
    backend: &std::sync::Arc<photos_app::App>,
) -> iced::Task<crate::message::Message> {
    use crate::message::Message;
    let backend = backend.clone();
    let mut tasks: Vec<iced::Task<Message>> = Vec::new();
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
                    Ok((id, rgba)) => Message::ThumbnailLoaded(id, rgba),
                    Err(_) => Message::CloseImage,
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
) -> iced::Task<crate::message::Message> {
    use crate::message::Message;
    let backend = backend.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    iced::Task::perform(
        async move {
            let result = backend.get_image_async(id, None, cancel).await;
            result.map(|rgba| (id, rgba))
        },
        |result| match result {
            Ok((id, rgba)) => Message::FullImageLoaded(id, rgba),
            Err(_) => Message::CloseImage,
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
