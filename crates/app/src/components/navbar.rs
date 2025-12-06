use eframe::egui;

pub enum NavAction {
    Gallery,
    Faces,
    Import,
}

pub struct Navbar;

impl Navbar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<NavAction> {
        let mut action = None;

        ui.vertical(|ui| {
            ui.set_width(150.0);
            ui.add_space(10.0);

            if ui.button("Gallery").clicked() {
                action = Some(NavAction::Gallery);
            }

            ui.add_space(5.0);

            if ui.button("Faces").clicked() {
                action = Some(NavAction::Faces);
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                if ui.button("Import").clicked() {
                    action = Some(NavAction::Import);
                }
            });
        });

        action
    }
}
