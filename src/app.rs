use crate::gui::{self, panes};
use egui_tracing::tracing_subscriber::layer::SubscriberExt;
use egui_tracing::tracing_subscriber::util::SubscriberInitExt;

/// the application root class
pub struct ZProberApp {
    pub volatile_context: ZProberVolatileContext,
    pub saved_context: ZProberSavedContext,
}

/// the saved context: part of the application that TCL can
/// act on, and that is persistent between app runs
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ZProberSavedContext {
    pub tiles_tree: egui_tiles::Tree<panes::Pane>,
    pub settings: panes::Settings,
    pub console: gui::console::ConsoleWindow,
}

/// the volatile context: part of the application that TCL can
/// act on, and that is destroyed when the app is closed
#[derive(Default)]
pub struct ZProberVolatileContext {
    pub collector: egui_tracing::EventCollector,
}

/// value for the context if no save is found on disk
impl Default for ZProberSavedContext {
    fn default() -> Self {
        // create tiles tree
        let mut tiles = egui_tiles::Tiles::default();

        let tabs: Vec<egui_tiles::TileId> = vec![
            tiles.insert_pane(panes::Pane::Settings),
            tiles.insert_pane(panes::Pane::Text("toto".to_owned())),
            tiles.insert_pane(panes::Pane::Tracing),
            tiles.insert_pane(panes::Pane::Console),
        ];
        let root: egui_tiles::TileId = tiles.insert_tab_tile(tabs);

        let tree = egui_tiles::Tree::new(egui::Id::unique("my_tree"), root, tiles);

        // create console
        let console = gui::console::ConsoleBuilder::new()
            .prompt(">> ")
            .history_size(20)
            .build();
        ZProberSavedContext {
            settings: panes::Settings::new(),
            tiles_tree: tree,
            console,
        }
    }
}

impl ZProberApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // create the application

        // try to get the previous context from persistent storage
        let saved_context = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        // create the volatile context
        let volatile_context = ZProberVolatileContext::default();

        // enable the tracing subscriber to write all traces in the pane
        egui_tracing::tracing_subscriber::registry()
            .with(volatile_context.collector.clone())
            .init();

        ZProberApp {
            volatile_context,
            saved_context,
        }
    }
}

impl eframe::App for ZProberApp {
    /// called when quitting: write the context to persistent storage
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.saved_context);
    }

    /// draw the GUI
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::gui::menu::menu_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            crate::gui::panes::tree_ui(ui, &mut self.volatile_context, &mut self.saved_context);
        });
    }
}
