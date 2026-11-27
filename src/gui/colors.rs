use egui::Color32;

use crate::{
    gui::source::{ColorKind, SourceSectionKind},
    probes::ProbeChannel,
};

pub struct CodeColors {
    pub background: Color32,
    pub background_highlight: Color32,
    pub comment: Color32,
    pub keyword: Color32,
    pub operator: Color32,
    pub name: Color32,
    pub literal: Color32,
    pub signal_not_probed: Color32,
    pub signal_probed_in: Color32,
    pub signal_probed_out: Color32,
    pub signal_probed_ela: Color32,
}

impl CodeColors {
    /// create a color list from the theme (light or dark)
    pub fn get(ui: &egui::Ui) -> Self {
        let visuals = ui.visuals();

        if visuals.dark_mode {
            CodeColors {
                background: visuals.window_fill,
                background_highlight: visuals.extreme_bg_color,
                comment: Color32::from_rgb(0x8B, 0x94, 0x9E),
                keyword: Color32::from_rgb(0x79, 0xC0, 0xFF),
                operator: Color32::from_rgb(0x56, 0xD4, 0xDD),
                name: Color32::from_rgb(0x7E, 0xE7, 0x87),
                literal: Color32::from_rgb(0xD2, 0xA8, 0xFF),
                signal_not_probed: Color32::from_rgb(0xFF, 0x55, 0x55),
                signal_probed_in: Color32::from_rgb(0xFF, 0xB8, 0x6C),
                signal_probed_out: Color32::from_rgb(0xF1, 0xFA, 0x8C),
                signal_probed_ela: Color32::from_rgb(0xFF, 0x79, 0xC6),
            }
        } else {
            CodeColors {
                background: visuals.window_fill,
                background_highlight: visuals.extreme_bg_color,
                comment: Color32::from_rgb(0x6E, 0x77, 0x81),
                keyword: Color32::from_rgb(0x09, 0x69, 0xDA),
                operator: Color32::from_rgb(0x0E, 0x7C, 0x86),
                name: Color32::from_rgb(0x11, 0x63, 0x29),
                literal: Color32::from_rgb(0x82, 0x50, 0xDF),
                signal_not_probed: Color32::from_rgb(0xB4, 0x23, 0x18),
                signal_probed_in: Color32::from_rgb(0xBC, 0x5C, 0x00),
                signal_probed_out: Color32::from_rgb(0x6E, 0x64, 0x00),
                signal_probed_ela: Color32::from_rgb(0xC2, 0x25, 0xC2),
            }
        }
    }

    /// return the correct color for a token based on its kind
    pub fn colorize_token(&self, kind: &SourceSectionKind) -> Color32 {
        match kind {
            SourceSectionKind::FixedText(ColorKind::Comment) => self.comment,
            SourceSectionKind::FixedText(ColorKind::Keyword) => self.keyword,
            SourceSectionKind::FixedText(ColorKind::Literal) => self.literal,
            SourceSectionKind::FixedText(ColorKind::Name) => self.name,
            SourceSectionKind::FixedText(ColorKind::Operator) => self.operator,
            SourceSectionKind::Signal(_) => self.name,
            SourceSectionKind::InstanceName(_, _) => self.name,
        }
    }

    /// return the correct color for a probed/prob_able signal
    pub fn colorize_probe(&self, probe: &Option<ProbeChannel>) -> Color32 {
        match probe {
            None => self.signal_not_probed,
            Some(ProbeChannel::EioIn) => self.signal_probed_in,
            Some(ProbeChannel::EioOut) => self.signal_probed_out,
            Some(ProbeChannel::Ela(_)) => self.signal_probed_ela,
        }
    }
}
