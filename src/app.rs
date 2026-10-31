use crate::gui::{self, panes};
use egui_tracing::tracing_subscriber::layer::SubscriberExt;
use egui_tracing::tracing_subscriber::util::SubscriberInitExt;
use tracing::{Level, event};

/// the application root class
pub struct ZProberApp {
    tcl_interpreter: molt::Interp,
    context_id: molt::ContextID,
    tiles_tree: egui_tiles::Tree<panes::Pane>,
}

/// the application context both persistent and non persistent
/// data, that the TCL can act on
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ZProberContext {
    pub settings: panes::Settings,
    pub console: gui::console::ConsoleWindow,

    #[serde(skip)]
    pub collector: egui_tracing::EventCollector,

    #[serde(skip)]
    tcl_command: Option<String>
}

/// Structure for context I/O
#[derive(serde::Serialize, serde::Deserialize)]
struct ZProberContextIO {
    context_structure: ZProberContext,
    tiles_tree: egui_tiles::Tree<panes::Pane>,
}

/// value for the context if no save is found on disk
impl Default for ZProberContext {
    fn default() -> Self {

        // create console
        let console = gui::console::ConsoleBuilder::new()
            .prompt(">> ")
            .history_size(20)
            .build();
        ZProberContext {
            settings: panes::Settings::new(),
            console,
            collector : Default::default(),
            tcl_command: None
        }
    }
}

impl Default for ZProberContextIO {
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

        ZProberContextIO {
            tiles_tree,
            context_structure: Default::default(),
        }
    }
}

// other methods for the context
impl ZProberContext {
    // register a TCL command to execute
    pub fn register_tcl_command(&mut self, command: &str) {
        self.tcl_command = Some(command.into());
    }

    // if a command has been registered, return it
    pub fn get_tcl_command(&mut self) -> Option<String> {
        self.tcl_command.take()
    }
}

impl ZProberApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // create the application

        // try to get the previous context from persistent storage
        let context_io: ZProberContextIO = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        // enable the tracing subscriber to write all traces in the pane
        egui_tracing::tracing_subscriber::registry()
            .with(context_io.context_structure.collector.clone())
            .init();

        // create the TCL interpeter
        let mut tcl_interpreter = molt::Interp::new();
        let context_id = tcl_interpreter.save_context(context_io.context_structure);
        ZProberApp {
            tcl_interpreter,
            context_id,
            tiles_tree: context_io.tiles_tree
        }
    }

    pub fn get_context(&mut self) -> &mut ZProberContext {
        self.tcl_interpreter.context(self.context_id)
    }
}

impl eframe::App for ZProberApp {
    /// called when quitting: write the context to persistent storage
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let context_io = ZProberContextIO {
            context_structure : self.get_context().clone(),
            tiles_tree : self.tiles_tree.clone()
        };
        eframe::set_value(storage, eframe::APP_KEY, &context_io);
    }

    /// draw the GUI
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::gui::menu::menu_bar(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(tcl_command) = crate::gui::panes::tree_ui(ui, &mut self.tiles_tree, self.tcl_interpreter.context(self.context_id)) {
                // we have a TCL command to execute

                event!(Level::INFO, "evaluating TCL \"{tcl_command}\"");
                let tcl_result = self.tcl_interpreter.eval(&tcl_command);
                match tcl_result {
                    Ok(value) => {
                        let console = &mut self.get_context().console;
                        event!(Level::DEBUG, "TCL result: {:?}", value);
                        console.write(&value.to_string());
                        console.prompt();
                    },
                    Err(exception) => { 
                        let console = &mut self.get_context().console;
                        if exception.is_error() {
                            event!(Level::ERROR, "TCL command \"{tcl_command}\" returned {exception:?}");
                            console.write(&format!("error: {}", exception.value()));
                        } else {
                            console.write(&exception.value().to_string());                 
                        }
                        console.prompt();
                    }
                }
            }
        });
    }
}
