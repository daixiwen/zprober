use crate::gui::panes;
use egui_tracing::tracing_subscriber::layer::SubscriberExt;
use egui_tracing::tracing_subscriber::util::SubscriberInitExt;

/// the application root class
pub struct ZProberApp {
    pub collector: egui_tracing::EventCollector,
    pub context: ZProberAppContext,
}

/// the context: part of the application that TCL can act on, and that is
/// persistent between app runs
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ZProberAppContext {
    pub settings: panes::Settings,
    pub tiles_tree: egui_tiles::Tree<panes::Pane>,
}

/// value for the context is no save is found on disk
impl Default for ZProberAppContext {
    fn default() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let tabs: Vec<egui_tiles::TileId> = vec![
            tiles.insert_pane(panes::Pane::Settings),
            tiles.insert_pane(panes::Pane::Text("toto".to_owned())),
            tiles.insert_pane(panes::Pane::Tracing),
        ];
        let root: egui_tiles::TileId = tiles.insert_tab_tile(tabs);

        let tree = egui_tiles::Tree::new(egui::Id::unique("my_tree"), root, tiles);
        ZProberAppContext {
            settings: panes::Settings::new(),
            tiles_tree: tree,
        }
    }
}

impl ZProberApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // create the application
        let collector = egui_tracing::EventCollector::default();

        // try to get the previous context from persistent storage
        let context = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        // enable the tracing subscriber to write all traces in the pane
        egui_tracing::tracing_subscriber::registry()
            .with(collector.clone())
            .init();

        ZProberApp { collector, context }
    }
}

impl eframe::App for ZProberApp {
    /// called when quitting: write the context to persistent storage
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.context);
    }

    /// draw the GUI
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::gui::menu::menu_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            crate::gui::panes::tree_ui(
                ui,
                &mut self.context.tiles_tree,
                &mut self.context.settings,
                &self.collector,
            );
        });
    }
}
