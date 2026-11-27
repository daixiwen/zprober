use egui_extras::{Column, TableBody, TableBuilder};

use crate::{
    app::ZProberAppContext,
    probes::{Entity, ProbeChannel},
};

/// convert an optional probe channel to a text to display
pub fn probe_channel_to_text(
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

/// builds the contents of a combobox or context menu with a list or probe channels
pub fn probe_menu_contents(
    current_probe_selection: &mut Option<ProbeChannel>,
    context: &crate::app::ZProberAppContext,
) -> impl FnMut(&mut egui::Ui) -> () {
    |ui| {
        let options = [
            None,
            Some(ProbeChannel::EioIn),
            Some(ProbeChannel::EioOut),
            Some(ProbeChannel::Ela(0)),
        ];
        for option in options {
            ui.selectable_value(
                current_probe_selection,
                option.clone(),
                probe_channel_to_text(&option, context),
            );
        }
    }
}

/// probes list panel
pub fn panel(ui: &mut egui::Ui, context: &crate::app::ZProberAppContext) {
    let colors = super::colors::CodeColors::get(ui);

    let table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::auto())
        //        .column(Column::remainder())
        .column(Column::auto())
        .column(Column::auto());

    table
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Probe");
            });
            header.col(|ui| {
                ui.strong("Path");
            });
            header.col(|ui| {
                ui.strong("Name");
            });
        })
        .body(|mut body| {
            add_probes(&mut body, &context.project_probes, "", context, &colors);
        });
}

fn add_probes(
    body: &mut TableBody,
    module: &Entity,
    path: &str,
    context: &ZProberAppContext,
    colors: &super::colors::CodeColors,
) {
    let current_path = format!("{}.{}", path, module.label);
    for probe in &module.probes {
        let previous_channel = probe.channel.clone();
        let mut channel = Some(previous_channel.clone());

        let color = colors.colorize_probe(&channel);

        body.row(18.0, |mut row| {
            row.col(|ui| {
                egui::ComboBox::from_id_salt(&probe.signal_name)
                    .selected_text(super::probes::probe_channel_to_text(&channel, context))
                    .show_ui(
                        ui,
                        super::probes::probe_menu_contents(&mut channel, context),
                    );
            });
            row.col(|ui| {
                ui.colored_label(color, &current_path);
            });
            row.col(|ui| {
                ui.colored_label(color, &probe.signal_name);
            });
        });

        if channel != Some(previous_channel) {
            let channel_text = match channel {
                None => "None".to_owned(),
                Some(channel) => channel.to_string(),
            };
            context.execute_tcl(&format!(
                "change_probe {} {} {}",
                current_path, probe.signal_name, channel_text
            ));
        }
    }
    for child in &module.children {
        add_probes(body, child, &current_path, context, colors);
    }
}
