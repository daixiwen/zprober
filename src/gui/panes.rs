/// Tiles and panes handling
use egui_tracing::egui::Widget;

use crate::gui::console;

/// enumeration of all pane types
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum Pane {
    Settings,
    Tracing,
    Console,
    Hierarchy,
    Signals,
    Source,
    Probes,
}

/// display the complete tiles tree
/// returns a tcl command to execute if any has been
/// registered during the panes exploration
pub fn tree_ui(
    ui: &mut egui::Ui,
    tiles_tree: &mut egui_tiles::Tree<Pane>,
    context: &mut crate::app::ZProberAppContext,
) {
    let mut behavior = MyBehavior { context };
    tiles_tree.ui(&mut behavior, ui);
}

/// customization of tiles display
struct MyBehavior<'a> {
    context: &'a mut crate::app::ZProberAppContext,
}

impl<'a> egui_tiles::Behavior<Pane> for MyBehavior<'a> {
    /// rules applied before display
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            prune_empty_tabs: true,
            prune_empty_containers: true,
            prune_single_child_tabs: false,
            prune_single_child_containers: true,
            all_panes_must_have_tabs: true,
            join_nested_linear_containers: true,
            flatten_tabs_in_tabs: true,
        }
    }

    /// return a title for each pane
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::Settings => "Settings".into(),
            Pane::Tracing => "Tracing".into(),
            Pane::Console => "TCL".into(),
            Pane::Hierarchy => "Hierarchy".into(),
            Pane::Signals => "Signals".into(),
            Pane::Source => "Source".into(),
            Pane::Probes => "Probes".into(),
        }
    }

    /// display each pane
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        match pane {
            Pane::Settings => self.context.settings.ui(ui),
            Pane::Tracing => {
                egui_tracing::Logs::new(self.context.collector.clone()).ui(ui);
            }
            Pane::Console => {
                if let Some(cmd) = console::console_pane(ui, &mut self.context.console) {
                    self.context.execute_tcl_noprint(&cmd);
                };
            }
            Pane::Hierarchy => {
                egui::ScrollArea::both().show(ui, |ui| {
                    display_project_hierarchy(ui, &self.context.project_entries, self.context);
                });
            }
            Pane::Signals => {
                super::signals::panel(ui, self.context);
            }
            Pane::Source => {
                super::source::panel(ui, &mut self.context);
            }
            Pane::Probes => {
                super::probes::panel(ui, self.context);
            }
        }

        Default::default()
    }
}

pub fn create_panes() -> egui_tiles::Tree<Pane> {
    // create tiles tree
    let mut tiles = egui_tiles::Tiles::default();

    let hierarchy_pane = tiles.insert_pane(Pane::Hierarchy);
    let hierarchy_tab = tiles.insert_tab_tile(vec![hierarchy_pane]);

    let signal_pane = tiles.insert_pane(Pane::Signals);
    let signal_tab = tiles.insert_tab_tile(vec![signal_pane]);

    let left_column = tiles.insert_vertical_tile(vec![hierarchy_tab, signal_tab]);

    let main_view_tabs = vec![
        tiles.insert_pane(Pane::Source),
        tiles.insert_pane(Pane::Probes),
        tiles.insert_pane(Pane::Settings),
    ];
    let main_view = tiles.insert_tab_tile(main_view_tabs);

    let console_tabs = vec![
        tiles.insert_pane(Pane::Console),
        tiles.insert_pane(Pane::Tracing),
    ];
    let console_view = tiles.insert_tab_tile(console_tabs);

    let root_panes = vec![
        left_column,
        // column with main view
        tiles.insert_vertical_tile(vec![
            // main view
            main_view,
            // console
            console_view,
        ]),
    ];

    let root = tiles.insert_horizontal_tile(root_panes);
    egui_tiles::Tree::new(egui::Id::unique("my_tree"), root, tiles)
}

fn display_project_hierarchy(
    ui: &mut egui::Ui,
    hierarchy: &Vec<crate::tcl::project::ProjectEntry>,
    context: &crate::app::ZProberAppContext,
) {
    for entry in hierarchy {
        let mut selected = entry.path == context.selected_path;
        egui::CollapsingHeader::custom(
            ui,
            &entry.path,
            |ui| {
                if ui.toggle_value(&mut selected, entry.to_string()).clicked() {
                    context.execute_tcl(&format!("select_instance {}", entry.path));
                }
            },
            |ui| display_project_hierarchy(ui, &entry.children, context),
        );
    }
}

/// temporary example pane
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Settings {
    checked: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    pub fn new() -> Self {
        Self { checked: false }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.checked, "Checked");
    }
}
