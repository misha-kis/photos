use eframe::egui::{self, ColorImage, Vec2};
use image::DynamicImage;

pub fn thumbnail_view(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    is_visible: bool,
    size: (f32, f32),
    mut try_get: impl FnMut() -> Option<DynamicImage>,
    click_callback: Option<impl FnMut()>,
) {
    if is_visible && let Some(image) = try_get() {
        let rgba = image.into_rgba8();
        let tex = ctx.load_texture(
            "",
            ColorImage::from_rgba_unmultiplied(
                [rgba.width() as _, rgba.height() as _],
                rgba.as_raw(),
            ),
            Default::default(),
        );
        let image_size = egui::Vec2::new(size.0, size.1);
        let (rect, response) = ui.allocate_exact_size(image_size, egui::Sense::click());
        let _ = ui.put(rect, |ui: &mut egui::Ui| ui.image((tex.id(), image_size)));
        if response.clicked()
            && let Some(mut click_callback) = click_callback
        {
            click_callback()
        }
    } else {
        ui.allocate_space(Vec2::new(size.0, size.1));
    }
}
