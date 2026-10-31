use egui_tracing::tracing_subscriber::layer::SubscriberExt;
use egui_tracing::tracing_subscriber::util::SubscriberInitExt;
use tracing::{Level, event};

use crate::gui::{self, panes};
use crate::tcl::interpreter::MsgFromTcl;

/// the application root class, both persistent and non persistent
/// data for the GUI
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ZProberApp {
    pub tiles_tree: egui_tiles::Tree<panes::Pane>,
    pub context: ZProberAppContext
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ZProberAppContext {
    pub settings: panes::Settings,
    pub console: gui::console::ConsoleWindow,

    #[serde(skip)]
    pub collector: egui_tracing::EventCollector,

    #[serde(skip)]
    tcl: crate::tcl::interpreter::Interpreter
}

impl ZProberAppContext {
    pub fn execute_tcl(&self, cmd: &str) {
        self.tcl.execute(cmd);
    }
}

/// value for the context if no save is found on disk
impl Default for ZProberAppContext {
    fn default() -> Self {

        // create console
        let console = gui::console::ConsoleBuilder::new()
            .prompt(">> ")
            .history_size(20)
            .build();

        ZProberAppContext {
            settings: panes::Settings::new(),
            console,
            collector : Default::default(),
            tcl: Default::default()
        }
    }
}

impl Default for ZProberApp {
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

        let tiles_tree = egui_tiles::Tree::new(egui::Id::unique("my_tree"), root, tiles);

        ZProberApp {
            tiles_tree,
            context : Default::default(),
        }
    }
}

impl ZProberApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // create the application

        // try to get the previous context from persistent storage
        let app: ZProberApp = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        // enable the tracing subscriber to write all traces in the pane
        egui_tracing::tracing_subscriber::registry()
            .with(app.context.collector.clone())
            .init();

        app
    }
}

impl eframe::App for ZProberApp {
    /// called when quitting and regularly: write the context to persistent storage
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self);
    }

    /// called on exit
    fn on_exit(&mut self) {
        // stop the TCL thread
        self.context.tcl.quit();
    }

    /// draw the GUI
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {

        // check messages from the TCL thread
        if let Some(msg) = self.context.tcl.receive() {
            event!(Level::DEBUG, ?msg, "received message from TCL");
            match msg {
                MsgFromTcl::Write(msg) => {
                    self.context.console.write(&msg);
                },
                MsgFromTcl::CommandOk(result) => {
                    self.context.console.write(&result);
                    self.context.console.prompt();
                },
                MsgFromTcl::CommandError(result) => {
                    self.context.console.write(&format!("ERROR: {result}"));
                    self.context.console.prompt();
                }
            }
        }

        // GUI
        crate::gui::menu::menu_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            crate::gui::panes::tree_ui(ui, &mut self.tiles_tree, &mut self.context);
        });
    }
}
