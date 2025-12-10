use eframe::egui::{self, ColorImage, Vec2};
use image::DynamicImage;

pub fn thumbnail_view(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    is_visible: bool,
    size: (f32, f32),
    texture_id: &str,
    mut try_get: impl FnMut() -> anyhow::Result<Option<DynamicImage>>,
    click_callback: Option<impl FnMut()>,
    cached_handle: Option<egui::TextureHandle>,
    mut on_texture_loaded: impl FnMut(egui::TextureHandle),
) {
    if is_visible {
        if let Some(tex) = cached_handle {
            // Use cached texture handle - no need to reload
            let image_size = egui::Vec2::new(size.0, size.1);
            let (rect, response) = ui.allocate_exact_size(image_size, egui::Sense::click());
            let _ = ui.put(rect, |ui: &mut egui::Ui| ui.image((tex.id(), image_size)));
            if response.clicked()
                && let Some(mut click_callback) = click_callback
            {
                click_callback()
            }
        } else {
            // No cached handle, try to load the image
            match try_get() {
                Ok(Some(image)) => {
                    // Load texture only once when image first becomes available
                    let rgba = image.into_rgba8();
                    let tex = ctx.load_texture(
                        texture_id,
                        ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as _, rgba.height() as _],
                            rgba.as_raw(),
                        ),
                        Default::default(),
                    );
                    // Cache the handle for future frames
                    on_texture_loaded(tex.clone());
                    
                    let image_size = egui::Vec2::new(size.0, size.1);
                    let (rect, response) = ui.allocate_exact_size(image_size, egui::Sense::click());
                    let _ = ui.put(rect, |ui: &mut egui::Ui| ui.image((tex.id(), image_size)));
                    if response.clicked()
                        && let Some(mut click_callback) = click_callback
                    {
                        click_callback()
                    }
                }
                Ok(None) => {
                    ui.allocate_space(Vec2::new(size.0, size.1));
                }
                Err(err) => {
                    ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                }
            }
        }
    } else {
        ui.allocate_space(Vec2::new(size.0, size.1));
    }
}
