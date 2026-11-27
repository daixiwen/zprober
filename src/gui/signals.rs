use egui_extras::{Column, TableBuilder};

/// pane with the signals
pub fn panel(ui: &mut egui::Ui, context: &crate::app::ZProberAppContext) {
    let colors = super::colors::CodeColors::get(ui);

    let table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto())
        .column(Column::auto())
        .column(Column::auto())
        .column(Column::auto())
        //        .column(Column::remainder())
        .column(Column::auto());

    table
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Probe");
            });
            header.col(|ui| {
                ui.strong("Name");
            });
            header.col(|ui| {
                ui.strong("Mode");
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
                let probe = context.probes.iter().find(|p| p.signal_name == signal.name);

                let previous_channel = probe.map(|p| p.channel.clone());
                let mut channel = previous_channel.clone();

                let color = colors.colorize_probe(&channel);

                body.row(18.0, |mut row| {
                    row.col(|ui| {
                        egui::ComboBox::from_id_salt(&signal.name)
                            .selected_text(super::probes::probe_channel_to_text(&channel, context))
                            .show_ui(
                                ui,
                                super::probes::probe_menu_contents(&mut channel, context),
                            );
                    });
                    row.col(|ui| {
                        ui.colored_label(color, &signal.name);
                    });
                    row.col(|ui| {
                        ui.colored_label(color, signal.mode.to_string());
                    });
                    row.col(|ui| {
                        ui.colored_label(color, &signal.signal_type);
                    });
                    row.col(|ui| {
                        ui.colored_label(color, format!("{}", signal.size));
                    });
                });

                if channel != previous_channel {
                    let channel_text = match channel {
                        None => "None".to_owned(),
                        Some(channel) => channel.to_string(),
                    };
                    context.execute_tcl(&format!(
                        "change_probe {} {} {}",
                        context.selected_path, signal.name, channel_text
                    ));
                }
            }
        });
}
