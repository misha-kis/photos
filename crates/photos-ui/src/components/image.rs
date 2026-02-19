use eframe::egui::{self, Vec2};

fn get_correct_size((img_w, img_h): (f32, f32), (area_w, area_h): (f32, f32)) -> (f32, f32) {
    if img_w == 0.0 || img_h == 0.0 {
        return (0.0, 0.0);
    }

    let width_ratio = area_w / img_w;
    let height_ratio = area_h / img_h;

    let scale = width_ratio.min(height_ratio);

    (img_w * scale, img_h * scale)
}

pub fn image_view(
    ui: &mut egui::Ui,
    is_visible: bool,
    size: (f32, f32),
    mut try_get: impl FnMut() -> Option<egui::TextureHandle>,
    click_callback: Option<impl FnMut()>,
) {
    if is_visible {
        match try_get() {
            Some(tex) => {
                let image_size = tex.size();
                let image_size =
                    get_correct_size((image_size[0] as f32, image_size[1] as f32), size);
                let (rect, response) = ui.allocate_exact_size(size.into(), egui::Sense::click());
                let _ = ui.put(rect, |ui: &mut egui::Ui| {
                    ui.image((tex.id(), image_size.into()))
                });
                if response.clicked()
                    && let Some(mut click_callback) = click_callback
                {
                    click_callback()
                }
            }
            None => {
                ui.allocate_space(Vec2::new(size.0, size.1));
            }
        }
    } else {
        ui.allocate_space(Vec2::new(size.0, size.1));
    }
}
