/// Tiles and panes handling
use egui_tracing::egui::Widget;

use crate::gui::console;

/// enumeration of all pane types
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum Pane {
    Settings,
    Text(String),
    Tracing,
    Console,
}

/// display the complete tiles tree
/// returns a tcl command to execute if any has been
/// registered during the panes exploration
pub fn tree_ui(
    ui: &mut egui::Ui,
    tiles_tree: &mut egui_tiles::Tree<Pane>,
    context: &mut crate::app::ZProberAppContext,
) {
    let mut behavior = MyBehavior {
        context: context,
    };
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
            Pane::Text(text) => text.clone().into(),
            Pane::Tracing => "Tracing".into(),
            Pane::Console => "TCL".into(),
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
            Pane::Text(text) => {
                ui.text_edit_singleline(text);
            }
            Pane::Tracing => {
                egui_tracing::Logs::new(self.context.collector.clone()).ui(ui);
            }
            Pane::Console => {
                if let Some(cmd) = console::console_pane(ui, &mut self.context.console) {
                    self.context.execute_tcl(&cmd);
                };
            }
        }

        Default::default()
    }
}

/// temporary example pane
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Settings {
    checked: bool,
}

impl Settings {
    pub fn new() -> Self {
        Self { checked: false }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.checked, "Checked");
    }
}
