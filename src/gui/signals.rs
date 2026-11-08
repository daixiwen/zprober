use egui_extras::{Column, TableBuilder};

/// pane with the signals
pub fn panel(ui: &mut egui::Ui, context: &crate::app::ZProberAppContext) {
    let table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto())
        .column(Column::auto())
        //        .column(Column::remainder())
        .column(Column::auto());

    table
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Name");
            });
            header.col(|ui| {
                ui.strong("type");
            });
            header.col(|ui| {
                ui.strong("size");
            });
        })
        .body(|mut body| {
            for signal in &context.signals {
                body.row(18.0, |mut row| {
                    row.col(|ui| {
                        ui.label(&signal.name);
                    });
                    row.col(|ui| {
                        ui.label(&signal.signal_type);
                    });
                    row.col(|ui| {
                        ui.label(format!("{}", signal.size));
                    });
                })
            }
        });
}
