#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use egui_tracing::tracing_subscriber::layer::SubscriberExt;
use egui_tracing::tracing_subscriber::util::SubscriberInitExt;

fn main() -> Result<(), eframe::Error> {

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    let mut tiles = egui_tiles::Tiles::default();
    let mut settings = zprober::gui::panes::Settings::new();

    let tabs: Vec<egui_tiles::TileId> = vec![
        tiles.insert_pane(zprober::gui::panes::Pane::Settings),
        tiles.insert_pane(zprober::gui::panes::Pane::Text("toto".to_owned())),
        tiles.insert_pane(zprober::gui::panes::Pane::Tracing),
    ];
    let root: egui_tiles::TileId = tiles.insert_tab_tile(tabs);

    let mut tree = egui_tiles::Tree::new(egui::Id::unique("my_tree"), root, tiles);

    let collector = egui_tracing::EventCollector::default();
    egui_tracing::tracing_subscriber::registry()
        .with(collector.clone())
        .init();

    eframe::run_ui_native("ZProber", options, move |ui, _frame| {
        zprober::gui::menu::menu_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            zprober::gui::panes::tree_ui(ui, &mut tree, &mut settings, &collector);
        });
    })
}
