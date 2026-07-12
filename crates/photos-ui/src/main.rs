mod gallery;

use std::path::PathBuf;
use std::sync::Arc;

use gallery::thumbnail_store::{GalleryConfig, ThumbnailStatus, ThumbnailStore};
use iced::task::Task;
use iced::widget::{button, column, container, grid, image, row, scrollable, text};
use iced::{ContentFit, Element, Fill, Length, Pixels, Theme};
use photos_app::{App, AppError, config};
use photos_domain::{ImageId, RgbaImage};
use tokio::runtime::Handle;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::prelude::*;

fn main() -> iced::Result {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    iced::application(Ui::boot, Ui::update, Ui::view)
        .title(app_title)
        .theme(app_theme)
        .run()
}

fn app_title(_: &Ui) -> String {
    "Photos UI".to_string()
}

fn app_theme(_: &Ui) -> Theme {
    Theme::TokyoNight
}

#[derive(Default)]
struct Ui {
    state: RootState,
}

#[derive(Default)]
enum RootState {
    #[default]
    ClosedGallery,
    Opening {
        path: PathBuf,
    },
    OpenGallery {
        session: GallerySession,
    },
    Error {
        message: String,
    },
}

struct GallerySession {
    path: PathBuf,
    app: Arc<App>,
    image_ids: Vec<ImageId>,
    scene: GalleryScene,
    thumbs: ThumbnailStore,
    config: GalleryConfig,
    grid: GridState,
}

enum GalleryScene {
    Grid,
    FullImage {
        selected_index: usize,
        status: FullImageStatus,
    },
}

enum FullImageStatus {
    Idle,
    Loading,
    Loaded(image::Handle),
    Failed(String),
}

#[derive(Clone, Copy)]
struct GridState {
    card_size: f32,
}

impl Default for GridState {
    fn default() -> Self {
        Self { card_size: 170.0 }
    }
}

impl GridState {
    fn columns_for_width(&self, width: f32, spacing: f32) -> usize {
        let card = self.card_size.max(1.0);
        let available = width.max(card);
        let per_col = card + spacing;
        let cols = ((available + spacing) / per_col).floor() as usize;

        cols.max(1)
    }
}

#[derive(Clone)]
enum Message {
    OpenGalleryPressed,
    GalleryPathSelected(Option<PathBuf>),
    GalleryInitialized(Result<(PathBuf, Arc<App>, Vec<ImageId>), String>),
    CloseGalleryPressed,
    GridScrolled(scrollable::Viewport),
    ThumbnailLoadStarted {
        image_id: ImageId,
        request_id: u64,
    },
    ThumbnailLoaded {
        image_id: ImageId,
        request_id: u64,
        result: Result<image::Handle, String>,
    },
    OpenImage(usize),
    FullImageLoaded(Result<image::Handle, String>),
    BackToGrid,
}

impl Ui {
    fn boot() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenGalleryPressed => {
                Task::perform(pick_folder(), Message::GalleryPathSelected)
            }
            Message::GalleryPathSelected(Some(path)) => {
                self.state = RootState::Opening { path: path.clone() };
                Task::perform(init_gallery(path), Message::GalleryInitialized)
            }
            Message::GalleryPathSelected(None) => Task::none(),
            Message::GalleryInitialized(Ok((path, app_handle, image_ids))) => {
                let mut session = GallerySession {
                    path,
                    app: app_handle,
                    image_ids,
                    scene: GalleryScene::Grid,
                    thumbs: ThumbnailStore::new(),
                    config: GalleryConfig::default(),
                    grid: GridState::default(),
                };
                let task = session.schedule_thumbnail_tasks(0, 6);
                self.state = RootState::OpenGallery { session };
                task
            }
            Message::GalleryInitialized(Err(message)) => {
                self.state = RootState::Error { message };
                Task::none()
            }
            Message::CloseGalleryPressed => {
                if let RootState::OpenGallery { session } = &mut self.state {
                    session.thumbs.clear();
                }
                self.state = RootState::ClosedGallery;
                Task::none()
            }
            Message::GridScrolled(viewport) => {
                if let RootState::OpenGallery { session } = &mut self.state
                    && matches!(session.scene, GalleryScene::Grid)
                {
                    let (first_row, last_row) = session.visible_rows(viewport);
                    tracing::info!("scroll: {first_row}:{last_row}");
                    session.schedule_thumbnail_tasks(first_row, last_row)
                } else {
                    Task::none()
                }
            }
            Message::ThumbnailLoadStarted {
                image_id,
                request_id,
            } => {
                if let RootState::OpenGallery { session } = &mut self.state {
                    session.thumbs.mark_loading(image_id, request_id)
                }
                Task::none()
            }
            Message::ThumbnailLoaded {
                image_id,
                request_id,
                result,
            } => {
                if let RootState::OpenGallery { session } = &mut self.state {
                    match result {
                        Ok(handle) => session.thumbs.finish_loaded(image_id, request_id, handle),
                        Err(err) => session.thumbs.finish_failed(image_id, request_id, err),
                    }
                }
                Task::none()
            }
            Message::OpenImage(index) => {
                if let RootState::OpenGallery { session } = &mut self.state {
                    if let Some(image_id) = session.image_ids.get(index).copied() {
                        session.scene = GalleryScene::FullImage {
                            selected_index: index,
                            status: FullImageStatus::Loading,
                        };
                        let app_handle = session.app.clone();
                        let cancel = CancellationToken::new();
                        Task::perform(
                            async move { load_full_image_handle(app_handle, image_id, cancel).await },
                            Message::FullImageLoaded,
                        )
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                }
            }
            Message::FullImageLoaded(result) => {
                if let RootState::OpenGallery { session } = &mut self.state
                    && let GalleryScene::FullImage {
                        selected_index: _,
                        status,
                    } = &mut session.scene
                {
                    *status = match result {
                        Ok(handle) => FullImageStatus::Loaded(handle),
                        Err(message) => FullImageStatus::Failed(message),
                    };
                }
                Task::none()
            }
            Message::BackToGrid => {
                if let RootState::OpenGallery { session } = &mut self.state {
                    session.scene = GalleryScene::Grid;
                    session.schedule_thumbnail_tasks(0, 6)
                } else {
                    Task::none()
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.state {
            RootState::ClosedGallery => {
                let content = column![
                    text("Photos").size(42),
                    button("Open gallery").on_press(Message::OpenGalleryPressed),
                ]
                .spacing(16)
                .align_x(iced::Alignment::Center);

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
            }
            RootState::Opening { path } => container(
                column![
                    text("Opening gallery...").size(28),
                    text(path.display().to_string()),
                ]
                .spacing(8),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
            RootState::Error { message } => container(
                column![
                    text("Error").size(30),
                    text(message),
                    button("Back").on_press(Message::CloseGalleryPressed),
                ]
                .spacing(8),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
            RootState::OpenGallery { session } => match &session.scene {
                GalleryScene::Grid => session.view_grid(),
                GalleryScene::FullImage {
                    selected_index,
                    status,
                } => session.view_full_image(*selected_index, status),
            },
        }
    }
}

impl GallerySession {
    fn view_grid(&self) -> Element<'_, Message> {
        let card_size = self.grid.card_size;

        let images = self.image_ids.iter().enumerate().map(|(idx, id)| {
            let card = match self.thumbs.status(id) {
                ThumbnailStatus::Loaded { handle, .. } => button(
                    image(handle)
                        .width(Fill)
                        .height(Fill)
                        .content_fit(ContentFit::Cover),
                )
                .on_press(Message::OpenImage(idx)),
                ThumbnailStatus::Failed { message } => {
                    button(text(format!("Failed\n{message}"))).on_press(Message::OpenImage(idx))
                }
                ThumbnailStatus::Queued { .. } | ThumbnailStatus::Loading { .. } => {
                    button(text("Loading...")).on_press(Message::OpenImage(idx))
                }
                ThumbnailStatus::Unloaded => button(text("...")).on_press(Message::OpenImage(idx)),
            };

            container(card)
                .width(Length::Fixed(card_size))
                .height(Length::Fixed(card_size))
                .into()
        });

        let gallery = grid(images).fluid(Pixels(card_size)).spacing(Pixels(10.0));

        let content = column![
            row![
                text(self.path.display().to_string()),
                button("Close gallery").on_press(Message::CloseGalleryPressed),
            ]
            .spacing(12),
            scrollable(gallery).on_scroll(Message::GridScrolled)
        ]
        .spacing(12)
        .padding(12);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_full_image<'a>(
        &'a self,
        selected_index: usize,
        status: &'a FullImageStatus,
    ) -> Element<'a, Message> {
        let body: Element<'_, Message> = match status {
            FullImageStatus::Idle => text("Idle").into(),
            FullImageStatus::Loading => text("Loading image...").into(),
            FullImageStatus::Loaded(handle) => image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            FullImageStatus::Failed(err) => text(err).into(),
        };

        let header = row![
            button("Back").on_press(Message::BackToGrid),
            button("Close gallery").on_press(Message::CloseGalleryPressed),
            text(format!(
                "Image {}/{}",
                selected_index + 1,
                self.image_ids.len()
            )),
        ]
        .spacing(10);

        container(column![header, body].spacing(10).padding(12))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn visible_rows(&self, viewport: scrollable::Viewport) -> (usize, usize) {
        let spacing = 10.0;
        let columns = self
            .grid
            .columns_for_width(viewport.bounds().width, spacing);
        let row_height = self.grid.card_size + spacing;

        let content_y = viewport.absolute_offset().y.max(0.0);
        let viewport_height = viewport.bounds().height.max(1.0);

        let first_row = (content_y / row_height).floor() as usize;
        let last_row = ((content_y + viewport_height) / row_height).ceil() as usize;

        let total_rows = self.image_ids.len().div_ceil(columns);
        if total_rows == 0 {
            return (0, 0);
        }

        (first_row.min(total_rows - 1), last_row.min(total_rows - 1))
    }

    fn schedule_thumbnail_tasks(&mut self, first_row: usize, last_row: usize) -> Task<Message> {
        let columns = self.grid.columns_for_width(1200.0, 10.0);

        let actions = self.thumbs.mark_visible_and_collect(
            &self.image_ids,
            columns,
            first_row,
            last_row,
            &self.config,
        );

        let mut tasks = Vec::new();
        for request in actions.requests {
            let app = self.app.clone();
            let thumb_size = self.config.thumbnail_size;
            let image_id = request.image_id;
            let request_id = request.request_id;
            let cancel = request.cancel;

            tasks.push(Task::done(Message::ThumbnailLoadStarted {
                image_id,
                request_id,
            }));

            tasks.push(Task::perform(
                async move {
                    let result = load_thumbnail_handle(app, image_id, thumb_size, cancel).await;
                    Message::ThumbnailLoaded {
                        image_id,
                        request_id,
                        result,
                    }
                },
                |msg| msg,
            ));
        }

        Task::batch(tasks)
    }
}

async fn init_gallery(path: PathBuf) -> Result<(PathBuf, Arc<App>, Vec<ImageId>), String> {
    let app = App::new(path.clone(), config::Options::default(), Handle::current())
        .await
        .map_err(format_app_error)?;
    let app = Arc::new(app);
    let image_ids = app.get_image_ids_async().await.map_err(format_app_error)?;
    Ok((path, app, image_ids))
}

async fn pick_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .pick_folder()
        .await
        .map(|h| h.path().to_path_buf())
}

async fn load_thumbnail_handle(
    app: Arc<App>,
    image_id: ImageId,
    thumbnail_size: u32,
    cancel: CancellationToken,
) -> Result<image::Handle, String> {
    let rgba = app
        .get_thumbnail_async(image_id, thumbnail_size, cancel)
        .await
        .map_err(format_app_error)?;
    Ok(rgba_to_handle(rgba))
}

async fn load_full_image_handle(
    app: Arc<App>,
    image_id: ImageId,
    cancel: CancellationToken,
) -> Result<image::Handle, String> {
    let rgba = app
        .get_image_async(image_id, None, cancel)
        .await
        .map_err(format_app_error)?;
    Ok(rgba_to_handle(rgba))
}

fn rgba_to_handle(img: RgbaImage) -> image::Handle {
    image::Handle::from_rgba(img.width(), img.height(), img.into_raw())
}

fn format_app_error(err: AppError) -> String {
    err.to_string()
}
