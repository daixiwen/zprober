#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_ui_native("ZProber", options, move |ui, _frame| {
        zprober::gui::menu::menu_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Hello World!");
        });
    })
}
