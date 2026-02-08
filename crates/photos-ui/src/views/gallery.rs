use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use eframe::egui;
use photos_domain::ImageId;
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
                        image_view(ui, visible, size, || Ok(texture_opt.clone()), Some(click));
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
                        ui.horizontal(|ui| {
                            for id in ids_to_load {
                                let try_load =
                                    || Ok(app_proxy.get_thumbnail(id, ctx, cancel.clone()));
                                image_view(
                                    ui,
                                    true,
                                    (THUMBNAIL_SIZE, THUMBNAIL_SIZE),
                                    try_load,
                                    Some(|| {}),
                                )
                            }
                        })
                    });

                let mut available_size = ui.available_size();
                available_size.y -= THUMBNAIL_SIZE - ui.style().spacing.item_spacing.y;
                for id in ids_to_load {
                    let id = (*id, (available_size.x as u32, available_size.y as u32));
                    app_proxy.get_image(&id, ctx, cancel.clone());
                    if *selected_id == id.0 {
                        let try_get = || Ok(app_proxy.get_image(&id, ctx, cancel.clone()));
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
