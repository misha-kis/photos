use crate::photo_library::PhotoLibraryProxy;
use crate::thumb_size::ThumbSize;
use eframe::egui::{self, ColorImage};
use egui::Vec2;

pub struct GalleryView {
    pub columns: usize,
    pub thumb_size: ThumbSize,
}

impl GalleryView {
    pub fn new(thumb_size: ThumbSize) -> Self {
        Self {
            columns: 2,
            thumb_size,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        photo_library: &mut PhotoLibraryProxy,
        mut on_photo_selected: impl FnMut(usize),
    ) {
        self.columns = (ui.clip_rect().width()
            / (self.thumb_size as u32 as f32 + ui.style().spacing.item_spacing.x)
                .max(0.0)) as usize;

        let thumb_height = self.thumb_size as u32 as f32;
        let total_rows = (photo_library.get_number_of_images() + self.columns - 1)
            / self.columns;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let clip_rect = ui.clip_rect();
                let scroll_y = ui.min_rect().top();
                let visible_start = ((clip_rect.top() - scroll_y)
                    / (thumb_height + ui.style().spacing.item_spacing.y))
                    .floor() as isize;
                let visible_end = ((clip_rect.bottom() - scroll_y)
                    / (thumb_height + ui.style().spacing.item_spacing.y))
                    .ceil() as isize;

                let margin = 1;
                let start_row = (visible_start - margin).max(0) as usize;
                let end_row = ((visible_end + margin) as usize).min(total_rows);

                let mut start_index = start_row * self.columns;
                let mut end_index = (end_row * self.columns)
                    .min(photo_library.get_number_of_images());

                if start_index > end_index {
                    let x = start_index;
                    start_index = end_index;
                    end_index = x;
                }

                let mut i = 1;
                while i <= photo_library.get_number_of_images() {
                    ui.horizontal(|ui| {
                        for _ in 0..self.columns {
                            if start_index <= i
                                && i < end_index
                                && let Some(image) =
                                    photo_library.try_get_thumbnail(i as u32)
                            {
                                let rgba = image.into_rgba8();
                                let tex = ctx.load_texture(
                                    format!("thumb-{}", i),
                                    ColorImage::from_rgba_unmultiplied(
                                        [rgba.width() as _, rgba.height() as _],
                                        rgba.as_raw(),
                                    ),
                                    Default::default(),
                                );
                                if ui
                                    .add(egui::Button::image(&tex).frame(false))
                                    .clicked()
                                {
                                    on_photo_selected(i as usize);
                                }
                            } else {
                                ui.allocate_space(Vec2::new(
                                    self.thumb_size as u32 as f32,
                                    self.thumb_size as u32 as f32,
                                ));
                            }
                            i += 1;
                        }
                    });
                }
            });
    }
}

