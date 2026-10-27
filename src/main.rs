#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    let mut tiles = egui_tiles::Tiles::default();
    let mut settings = zprober::gui::panes::Settings::new();

    let tabs: Vec<egui_tiles::TileId> = vec![tiles.insert_pane(zprober::gui::panes::Pane::Settings), tiles.insert_pane(zprober::gui::panes::Pane::Text("toto".to_owned()))];
    let root: egui_tiles::TileId = tiles.insert_tab_tile(tabs);

    let mut tree = egui_tiles::Tree::new(egui::Id::unique("my_tree"), root, tiles);

    eframe::run_ui_native("ZProber", options, move |ui, _frame| {
        zprober::gui::menu::menu_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            zprober::gui::panes::tree_ui(ui, &mut tree, &mut settings);
        });
    })
}
