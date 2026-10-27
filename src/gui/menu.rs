/// Window menu bar

pub fn menu_bar(ui: &mut egui::Ui) {
    egui::Panel::top("menu").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("ZProber");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
            });
        });
    });
}
