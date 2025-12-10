use crate::components::thumbnail::thumbnail_view;
use crate::photo_library::PhotoLibraryProxy;
use eframe::egui;
use std::collections::HashMap;

pub struct GalleryView {
    pub columns: usize,
    desired_image_size: f32,
    texture_handles: HashMap<u32, egui::TextureHandle>,
}

impl GalleryView {
    pub fn new() -> Self {
        Self {
            columns: 2,
            desired_image_size: 100.0, // Desired image size in pixels
            texture_handles: HashMap::new(),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        photo_library: &mut PhotoLibraryProxy,
        mut on_photo_selected: impl FnMut(usize),
    ) {
        let available_width = ui.available_width();
        let spacing = ui.style().spacing.item_spacing.x;

        // Calculate how many columns fit with desired image size
        self.columns = ((available_width + spacing) / (self.desired_image_size + spacing))
            .floor()
            .max(1.0) as usize;

        // Calculate actual image size based on available width and number of columns
        let actual_image_size = ((available_width + spacing) / self.columns as f32 - spacing)
            .max(50.0) // Minimum size of 50px
            .min(500.0); // Maximum size of 500px

        let thumb_height = actual_image_size;
        let total_rows = (photo_library.get_number_of_images() + self.columns - 1) / self.columns;

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
                let mut end_index =
                    (end_row * self.columns).min(photo_library.get_number_of_images());

                if start_index > end_index {
                    let x = start_index;
                    start_index = end_index;
                    end_index = x;
                }

                let mut i = 1;
                while i <= photo_library.get_number_of_images() {
                    ui.horizontal(|ui| {
                        for _ in 0..self.columns {
                            let is_visible = start_index <= i && i <= end_index;
                            let photo_id = i as u32;
                            let texture_id = format!("thumbnail-{}", i);
                            
                            let try_get_image = || photo_library.try_get_thumbnail(photo_id);
                            let click_callback = || on_photo_selected(i as usize);
                            
                            // Get cached texture handle if available
                            let cached_handle = self.texture_handles.get(&photo_id).cloned();
                            
                            thumbnail_view(
                                ui,
                                ctx,
                                is_visible,
                                (actual_image_size, actual_image_size),
                                &texture_id,
                                try_get_image,
                                Some(click_callback),
                                cached_handle,
                                |handle| {
                                    // Store the texture handle when first loaded
                                    self.texture_handles.insert(photo_id, handle);
                                },
                            );
                            i += 1;
                        }
                    });
                }
            });
    }
}
