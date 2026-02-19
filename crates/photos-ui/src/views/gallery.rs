use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use eframe::egui;
use eframe::egui::TextureHandle;
use photos_domain::ImageId;
use std::ops::Div;
use std::rc::Rc;
use std::sync::RwLock;
use tokio_util::sync::CancellationToken;

const THUMBNAIL_SIZE: f32 = 128.0;

enum State {
    Gallery {
        grid: DynamicGrid<ImageId, egui::TextureHandle>,
        cancel: CancellationToken,
    },
    FullImage {
        image_ids: Rc<Vec<ImageId>>,
        selected_index: usize,
        cancel: CancellationToken,
    },
}

pub struct GalleryView {
    state: State,
}

impl GalleryView {
    pub fn new() -> Self {
        Self {
            state: State::Gallery {
                grid: DynamicGrid::new(THUMBNAIL_SIZE),
                cancel: CancellationToken::new(),
            },
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, app_proxy: &mut AppProxy) {
        match &mut self.state {
            State::Gallery { grid, cancel } => {
                let image_ids = Rc::new(app_proxy.image_ids.clone());
                let get_item_data = |image_id: &ImageId| -> Option<egui::TextureHandle> {
                    app_proxy.get_thumbnail(image_id, ctx, cancel.child_token())
                };
                let selected_index = Rc::new(RwLock::new(None));

                grid.show(
                    ui,
                    image_ids.clone().as_slice(),
                    get_item_data,
                    |ui, visible, size, texture_opt, click| {
                        image_view(ui, visible, size, || texture_opt.clone(), Some(click));
                    },
                    |index| *selected_index.write().unwrap() = Some(index),
                    true,
                );

                if let Some(selected_index) = *selected_index.read().unwrap() {
                    cancel.cancel();
                    self.state = State::FullImage {
                        image_ids,
                        selected_index,
                        cancel: CancellationToken::new(),
                    };
                }
            }

            State::FullImage {
                selected_index,
                cancel,
                image_ids,
            } => {
                let selected_id = &image_ids[*selected_index];
                let min_index = selected_index.saturating_sub(4);
                let max_index = selected_index
                    .saturating_add(4)
                    .min(image_ids.len().saturating_sub(1));

                let ids_to_load = &image_ids[min_index..max_index];

                egui::TopBottomPanel::bottom("nearby-images")
                    .resizable(false)
                    .default_height(THUMBNAIL_SIZE + ui.style().spacing.item_spacing.y)
                    .show(ctx, |ui| {
                        show_gallery_row(
                            ui,
                            image_ids,
                            *selected_index,
                            |image_id: &ImageId| {
                                app_proxy.get_thumbnail(image_id, ctx, cancel.child_token())
                            },
                            |ui, visible, size, try_get, click| {
                                image_view(ui, visible, size, || try_get.clone(), Some(click));
                            },
                            |_| {},
                        );
                    });

                let mut available_size = ui.available_size();
                available_size.y -= THUMBNAIL_SIZE - ui.style().spacing.item_spacing.y;
                for id in ids_to_load {
                    let id = (*id, (available_size.x as u32, available_size.y as u32));
                    app_proxy.get_image(&id, ctx, cancel.clone());
                    if *selected_id == id.0 {
                        let try_get = || app_proxy.get_image(&id, ctx, cancel.clone());
                        image_view(
                            ui,
                            true,
                            (available_size.x, available_size.y),
                            try_get,
                            Some(|| {}),
                        );
                    }
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    *selected_index = selected_index
                        .saturating_add(1)
                        .min(image_ids.len().saturating_sub(1))
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    *selected_index = selected_index.saturating_sub(1)
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel.cancel();
                    self.state = State::Gallery {
                        grid: DynamicGrid::new(THUMBNAIL_SIZE),
                        cancel: CancellationToken::new(),
                    }
                }
            }
        }
    }
}

impl Drop for GalleryView {
    fn drop(&mut self) {
        match &mut self.state {
            State::Gallery { cancel, .. } => cancel.cancel(),
            State::FullImage { cancel, .. } => cancel.cancel(),
        }
    }
}

fn show_gallery_row<FGet, FRender, FClick>(
    ui: &mut egui::Ui,
    items: &[ImageId],
    selected_item_index: usize,
    mut get_item_data: FGet,
    mut render_item: FRender,
    mut on_item_clicked: FClick,
) where
    FGet: FnMut(&ImageId) -> Option<TextureHandle>,
    FRender: FnMut(
        &mut egui::Ui,
        bool,
        (f32, f32),
        Option<TextureHandle>,
        &mut dyn FnMut(),
    ),
    FClick: FnMut(usize),
{
    let available_width = ui.available_width();
    let spacing = ui.style().spacing.item_spacing.x;

    let n_columns = ((available_width + spacing) / (THUMBNAIL_SIZE + spacing))
        .floor()
        .max(1.0) as usize;
    let show_extra = n_columns.div(2);

    let min_index = selected_item_index.saturating_sub(show_extra);
    let max_index = selected_item_index
        .saturating_add(show_extra)
        .min(items.len().saturating_sub(1));
    let items = &items[min_index..max_index];

    ui.horizontal(|ui| {
        for (item_index, id) in items.iter().enumerate() {
            let item_index = item_index + min_index;
            let current_index = item_index;
            let mut click_cb = || on_item_clicked(current_index);
            let tex = get_item_data(id);
            ui.push_id(*id, |ui| {
                render_item(
                    ui,
                    true,
                    (THUMBNAIL_SIZE, THUMBNAIL_SIZE),
                    tex,
                    &mut click_cb,
                );
            });
        }
    });
}
