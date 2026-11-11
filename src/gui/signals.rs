use crate::probes::probe::ProbeChannel;
use egui_extras::{Column, TableBuilder};

/// convert an optional probe channel to a text to display
fn probe_channel_to_text(
    probe: &Option<ProbeChannel>,
    _context: &crate::app::ZProberAppContext,
) -> String {
    match probe {
        None => "not connected".to_owned(),
        Some(ProbeChannel::EioIn) => "EIO In".to_owned(),
        Some(ProbeChannel::EioOut) => "EIO Out".to_owned(),
        Some(ProbeChannel::Ela(_)) => "ELA".to_owned(),
    }
}

/// pane with the signals
pub fn panel(ui: &mut egui::Ui, context: &crate::app::ZProberAppContext) {
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

                body.row(18.0, |mut row| {
                    row.col(|ui| {
                        egui::ComboBox::from_id_salt(signal.name.clone())
                            .selected_text(probe_channel_to_text(&channel, context))
                            .show_ui(ui, |ui| {
                                let options = [
                                    None,
                                    Some(ProbeChannel::EioIn),
                                    Some(ProbeChannel::EioOut),
                                    Some(ProbeChannel::Ela(0)),
                                ];
                                for option in options {
                                    ui.selectable_value(
                                        &mut channel,
                                        option.clone(),
                                        probe_channel_to_text(&option, context),
                                    );
                                }
                            });
                    });
                    row.col(|ui| {
                        ui.label(&signal.name);
                    });
                    row.col(|ui| {
                        ui.label(signal.mode.to_string());
                    });
                    row.col(|ui| {
                        ui.label(&signal.signal_type);
                    });
                    row.col(|ui| {
                        ui.label(format!("{}", signal.size));
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
