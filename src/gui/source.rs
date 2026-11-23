use std::collections::BTreeMap;

use crate::probes::ProbeChannel;
use egui::Color32;
use egui::Sense;
use egui::text::{ByteIndex, ByteRange, CharIndex};
use egui::text_selection::text_cursor_state::byte_index_from_char_index;

use egui::vec2;
use vhdl_lang::Kind;

/// size of the monospace text for source (maybe a preference later?)
const MONOSPACE_SIZE: f32 = 14.0;

/// structure holding all the data required for showing and interacting
/// with VHDL source code
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceView {
    pub sections: Vec<Vec<SourceSection>>,
}

/// a section in a source code line
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSection {
    range: egui::text::ByteRange,
    kind: SourceSectionKind,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum SourceSectionKind {
    FixedText(ColorKind),
    InstanceName(String, String), // library, entity
    Signal(String),               // signal/port name, probe type
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ColorKind {
    Comment,
    Keyword,
    Operator,
    Name,
    Literal,
}

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
                comment: Color32::from_rgb(0x62, 0x72, 0xA4),
                keyword: Color32::from_rgb(0x8B, 0xE9, 0xFD),
                operator: Color32::from_rgb(0x50, 0xFA, 0x7B),
                name: Color32::from_rgb(0xF8, 0xF8, 0xF2),
                literal: Color32::from_rgb(0xFF, 0xB8, 0x6C),
                signal_not_probed: Color32::from_rgb(0xFF, 0x79, 0xC6),
                signal_probed_in: Color32::from_rgb(0xBD, 0x93, 0xF9),
                signal_probed_out: Color32::from_rgb(0xFF, 0x55, 0x55),
                signal_probed_ela: Color32::from_rgb(0xF1, 0xFA, 0x8C),
            }
        } else {
            CodeColors {
                background: visuals.window_fill,
                background_highlight: visuals.extreme_bg_color,
                comment: Color32::from_rgb(0x62, 0x72, 0xA4),
                keyword: Color32::from_rgb(0x8B, 0xE9, 0xFD),
                operator: Color32::from_rgb(0x50, 0xFA, 0x7B),
                name: Color32::from_rgb(0x00, 0x00, 0x00),
                literal: Color32::from_rgb(0xFF, 0xB8, 0x6C),
                signal_not_probed: Color32::from_rgb(0xFF, 0x79, 0xC6),
                signal_probed_in: Color32::from_rgb(0xBD, 0x93, 0xF9),
                signal_probed_out: Color32::from_rgb(0xFF, 0x55, 0x55),
                signal_probed_ela: Color32::from_rgb(0xF1, 0xFA, 0x8C),
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

/// add a source section to the list, merging it with the last element if they are of the same kind
fn add_source_section(line: &mut Vec<SourceSection>, section: SourceSection) {
    if line.is_empty() {
        line.push(section);
    } else {
        let last_section = line.last_mut().unwrap();

        if last_section.kind == section.kind {
            // the two sections are similar, merge them into one
            last_section.range = egui::text::ByteRange {
                start: last_section.range.start,
                end: section.range.end,
            };
        } else {
            line.push(section);
        }
    }
}

/// add_source_section with the range given in bytes and the kind
fn add_source_section_byte_range(
    line: &mut Vec<SourceSection>,
    start: ByteIndex,
    end: ByteIndex,
    kind: SourceSectionKind,
) {
    if end > start {
        add_source_section(
            line,
            SourceSection {
                range: egui::text::ByteRange { start, end },
                kind,
            },
        );
    }
}

/// add_source_section with the range given in characters and the kind
fn add_source_section_character_range(
    line: &mut Vec<SourceSection>,
    text: &str,
    start: u32,
    end: u32,
    kind: SourceSectionKind,
) {
    add_source_section_byte_range(
        line,
        byte_index_from_char_index(text, CharIndex(start as usize)),
        byte_index_from_char_index(text, CharIndex(end as usize)),
        kind,
    );
}

/// add_source_section with the remaining of a line from a character position
fn add_source_section_line_end(
    line: &mut Vec<SourceSection>,
    text: &str,
    start: u32,
    kind: SourceSectionKind,
) {
    add_source_section_byte_range(
        line,
        byte_index_from_char_index(text, CharIndex(start as usize)),
        ByteIndex(text.len() - 1), // remove 1 because all lines from vhdl_lang::source include an \n at the end
        kind,
    );
}

impl SourceView {
    /// create a source view from a parsed VHDL file
    pub fn new(
        entity_name: &str,
        source: &vhdl_lang::Source,
        design_file: &vhdl_lang::ast::DesignFile,
    ) -> Self {
        let mut sections: Vec<Vec<SourceSection>> = Vec::new();
        let signal_tokens_vec =
            crate::vhdl::signal_tokens::extract_tokens(entity_name, design_file);
        let mut signal_tokens = signal_tokens_vec.iter();
        let mut current_signal_token = signal_tokens.next();

        if source.contents().num_lines() != 0 {
            let end_of_file = source.contents().end();
            let mut current_position = vhdl_lang::Position::new(0, 0);
            let mut current_line_description: Vec<SourceSection> = Vec::new();
            let mut current_line_origin = source.contents().get_line(0).unwrap().to_owned();

            for (tokens_list, _design_unit) in design_file.clone().design_units {
                for token in tokens_list {
                    let token_start = token.pos.start();
                    if current_position < token_start {
                        // a part of the file not tokenized is either empty lines or comments
                        while current_position.line < token_start.line {
                            // need to finish the current line and start the next one
                            add_source_section_line_end(
                                &mut current_line_description,
                                &current_line_origin,
                                current_position.character,
                                SourceSectionKind::FixedText(ColorKind::Comment),
                            );

                            // move to next line
                            sections.push(current_line_description);
                            current_line_description = Vec::new();
                            current_position.line += 1;
                            current_position.character = 0;
                            current_line_origin = source
                                .contents()
                                .get_line(current_position.line as usize)
                                .unwrap_or_default()
                                .to_owned();
                        }
                        if current_position < token_start {
                            // part of the current line is not included with the token -> put is also as comment
                            add_source_section_character_range(
                                &mut current_line_description,
                                &current_line_origin,
                                current_position.character,
                                token_start.character,
                                SourceSectionKind::FixedText(ColorKind::Comment),
                            );
                            current_position.character = token_start.character;
                        }
                    }

                    // now add the token
                    let section_kind = if let Some(token_pos) = current_signal_token
                        && token_pos.pos_match(current_position)
                    {
                        let kind = SourceSectionKind::Signal(token_pos.name.clone());
                        current_signal_token = signal_tokens.next();
                        kind
                    } else {
                        SourceSectionKind::FixedText(match token.kind {
                            Kind::Architecture
                            | Kind::Entity
                            | Kind::Configuration
                            | Kind::Package
                            | Kind::Block
                            | Kind::Process
                            | Kind::Generate
                            | Kind::Postponed
                            | Kind::Library
                            | Kind::Label
                            | Kind::Use
                            | Kind::Context
                            | Kind::Body
                            | Kind::Component
                            | Kind::Is
                            | Kind::Return
                            | Kind::Null
                            | Kind::Of
                            | Kind::On
                            | Kind::Generic
                            | Kind::Map
                            | Kind::Default
                            | Kind::Port
                            | Kind::Attribute
                            | Kind::Begin
                            | Kind::If
                            | Kind::Loop
                            | Kind::While
                            | Kind::Case
                            | Kind::Else
                            | Kind::Elsif
                            | Kind::Then
                            | Kind::When
                            | Kind::With
                            | Kind::Select
                            | Kind::Next
                            | Kind::Exit
                            | Kind::For
                            | Kind::Force
                            | Kind::Release
                            | Kind::Assert
                            | Kind::Report
                            | Kind::Severity
                            | Kind::Wait
                            | Kind::After
                            | Kind::Transport
                            | Kind::Inertial
                            | Kind::Reject
                            | Kind::Unaffected
                            | Kind::Until
                            | Kind::End
                            | Kind::All
                            | Kind::Range
                            | Kind::Downto
                            | Kind::To
                            | Kind::In
                            | Kind::Out
                            | Kind::InOut
                            | Kind::Buffer
                            | Kind::Linkage
                            | Kind::Signal
                            | Kind::Constant
                            | Kind::Variable
                            | Kind::File
                            | Kind::Open
                            | Kind::Alias
                            | Kind::Shared
                            | Kind::Others
                            | Kind::Record
                            | Kind::Type
                            | Kind::Subtype
                            | Kind::Access
                            | Kind::Units
                            | Kind::New
                            | Kind::Array
                            | Kind::Protected
                            | Kind::Pure
                            | Kind::Impure
                            | Kind::Function
                            | Kind::Procedure
                            | Kind::Vunit
                            | Kind::Parameter
                            | Kind::Literal
                            | Kind::Bus
                            | Kind::Disconnect
                            | Kind::Group
                            | Kind::Guarded
                            | Kind::Register
                            | Kind::Assume
                            | Kind::AssumeGuarantee
                            | Kind::Cover
                            | Kind::Fairness
                            | Kind::Property
                            | Kind::Restrict
                            | Kind::RestrictGuarantee
                            | Kind::Sequence
                            | Kind::Strong
                            | Kind::Vmode
                            | Kind::Vprop
                            | Kind::Private
                            | Kind::View
                            | Kind::Vpkg => ColorKind::Keyword,

                            Kind::Abs
                            | Kind::Not
                            | Kind::Plus
                            | Kind::Minus
                            | Kind::QueQue
                            | Kind::And
                            | Kind::Or
                            | Kind::Nand
                            | Kind::Nor
                            | Kind::Xor
                            | Kind::Xnor
                            | Kind::SLL
                            | Kind::SRL
                            | Kind::SLA
                            | Kind::SRA
                            | Kind::ROL
                            | Kind::ROR
                            | Kind::Mod
                            | Kind::Rem
                            | Kind::EQ
                            | Kind::NE
                            | Kind::LT
                            | Kind::LTE
                            | Kind::GT
                            | Kind::GTE
                            | Kind::QueEQ
                            | Kind::QueNE
                            | Kind::QueLT
                            | Kind::QueLTE
                            | Kind::QueGT
                            | Kind::QueGTE
                            | Kind::Que
                            | Kind::Times
                            | Kind::Pow
                            | Kind::Div => ColorKind::Operator,

                            Kind::Identifier => ColorKind::Name,

                            Kind::AbstractLiteral
                            | Kind::StringLiteral
                            | Kind::BitString
                            | Kind::Character => ColorKind::Literal,

                            Kind::Tick
                            | Kind::LeftPar
                            | Kind::RightPar
                            | Kind::LeftSquare
                            | Kind::RightSquare
                            | Kind::SemiColon
                            | Kind::Colon
                            | Kind::Bar
                            | Kind::Dot
                            | Kind::BOX
                            | Kind::LtLt
                            | Kind::GtGt
                            | Kind::Circ
                            | Kind::CommAt
                            | Kind::Concat
                            | Kind::Comma
                            | Kind::ColonEq
                            | Kind::RightArrow
                            | Kind::GraveAccent => ColorKind::Operator,

                            Kind::Text => ColorKind::Comment,
                        })
                    };
                    let token_end = token.pos.end();
                    // if the token spans on several lines, store them
                    while current_position.line < token_end.line {
                        add_source_section_line_end(
                            &mut current_line_description,
                            &current_line_origin,
                            current_position.character,
                            section_kind.clone(),
                        );

                        sections.push(current_line_description);
                        current_line_description = Vec::new();
                        current_position.line += 1;
                        current_position.character = 0;
                        current_line_origin = source
                            .contents()
                            .get_line(current_position.line as usize)
                            .unwrap_or_default()
                            .to_owned();
                    }
                    // last line
                    add_source_section_character_range(
                        &mut current_line_description,
                        &current_line_origin,
                        current_position.character,
                        token_end.character,
                        section_kind,
                    );
                    current_position.character = token_end.character;
                }
            }

            // no more tokens... add the rest of the file as comment
            add_source_section_line_end(
                &mut current_line_description,
                &current_line_origin,
                current_position.character,
                SourceSectionKind::FixedText(ColorKind::Comment),
            );
            sections.push(current_line_description);
            current_line_description = Vec::new();
            current_position.line += 1;
            current_position.character = 0;
            while current_position < end_of_file {
                current_line_origin = source
                    .contents()
                    .get_line(current_position.line as usize)
                    .unwrap_or_default()
                    .to_owned();
                add_source_section_line_end(
                    &mut current_line_description,
                    &current_line_origin,
                    current_position.character,
                    SourceSectionKind::FixedText(ColorKind::Comment),
                );
                sections.push(current_line_description);
                current_line_description = Vec::new();
                current_position.line += 1;
                current_position.character = 0;
            }
        }
        SourceView { sections }
    }

    /// find the signal at the indicated cursor position
    pub fn signal_at_pos(&self, line_number: usize, position: CharIndex) -> Option<String> {
        if let Some(line) = self.sections.get(line_number) &&
            let Some(section) = line.iter().find(|s| s.range.contains(&ByteIndex(usize::from(position)))) &&
                let SourceSectionKind::Signal(name) = &section.kind {
                    Some(name.clone())
        } else {
            None
        }
    }

    /// generate a vector of layout jobs from the source view
    pub fn generate_job(
        &self,
        source: &vhdl_lang::Source,
        probe_definitions: &BTreeMap<String, Option<crate::probes::ProbeChannel>>,
        line_number: usize,
        num_lines_digits: usize,
        code_colors: &CodeColors,
    ) -> egui::text::LayoutJob {
        let line = format!(
            "{line_num:width$} {text}",
            line_num = line_number,
            width = num_lines_digits,
            text = source
                .contents()
                .get_line(line_number)
                .map(|s| {
                    let mut s = s.to_owned();
                    // remove the end-of-line at the end
                    s.pop();
                    s
                })
                .unwrap_or_default()
        );
        let line_length = line.len();

        fn create_section(
            range: &ByteRange,
            range_shift: usize,
            color_fg: egui::Color32,
            color_bg: egui::Color32,
            underline: bool,
        ) -> egui::text::LayoutSection {
            let underline_stroke = if underline {
                egui::Stroke::new(1.0, color_fg)
            } else {
                egui::Stroke::NONE
            };

            egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: ByteRange {
                    start: range.start + range_shift,
                    end: range.end + range_shift,
                },
                format: egui::text::TextFormat {
                    font_id: egui::FontId::monospace(MONOSPACE_SIZE),
                    extra_letter_spacing: 0.0,
                    line_height: None,
                    color: color_fg,
                    background: color_bg,
                    expand_bg: 1.0,
                    coords: Default::default(),
                    italics: false,
                    underline: underline_stroke,
                    strikethrough: egui::Stroke::NONE,
                    valign: egui::Align::Center,
                },
            }
        }
        let sections = match self.sections.get(line_number) {
            None => vec![create_section(
                &ByteRange {
                    start: ByteIndex(0),
                    end: ByteIndex(line_length - 1),
                },
                0,
                code_colors.comment,
                code_colors.background,
                false,
            )],
            Some(source_sections) => {
                let mut secs = vec![create_section(
                    &ByteRange {
                        start: ByteIndex(0),
                        end: ByteIndex(num_lines_digits + 1),
                    },
                    0,
                    code_colors.comment,
                    code_colors.background,
                    false,
                )];
                let mut other_secs: Vec<egui::text::LayoutSection> =
                    source_sections
                        .iter()
                        .map(|ss| {
                            let (color_fg, color_bg, underline) =
                                if let SourceSectionKind::Signal(name) = &ss.kind
                                    && let Some(optional_channel) = probe_definitions.get(name)
                                {
                                    (
                                        code_colors.colorize_probe(optional_channel),
                                        code_colors.background_highlight,
                                        true,
                                    )
                                } else {
                                    (
                                        code_colors.colorize_token(&ss.kind),
                                        code_colors.background,
                                        false,
                                    )
                                };
                            create_section(
                                &ss.range,
                                num_lines_digits + 1,
                                color_fg,
                                color_bg,
                                underline,
                            )
                        })
                        .collect();
                secs.append(&mut other_secs);
                secs
            }
        };
        egui::text::LayoutJob {
            text: line,
            sections,
            wrap: egui::text::TextWrapping {
                max_width: f32::MAX,
                max_rows: 1,
                break_anywhere: true,
                overflow_character: None,
            },
            first_row_min_height: 0.0,
            break_on_newline: true,
            halign: egui::Align::LEFT,
            justify: false,
            round_output_to_gui: true,
            keep_trailing_whitespace: true,
        }
    }
}

/// pane with the source file
pub fn panel(ui: &mut egui::Ui, context: &mut crate::app::ZProberAppContext) {
    let uiresponse = ui.vertical(|ui| {
        // top line with the path
        ui.horizontal(|ui| {
            if ui.button("<").clicked() {
                tracing::event!(tracing::Level::DEBUG, "should implement button");
            }
            ui.label(&context.selected_path)
        });

        // build the tree of probes (maybe cache it in the context later?)
        let probe_definitions: BTreeMap<String, Option<ProbeChannel>> = context
            .signals
            .iter()
            .map(|s| {
                let channel = context
                    .probes
                    .iter()
                    .find(|p| p.signal_name == s.name)
                    .map(|p| p.channel.clone());

                (s.name.clone(), channel)
            })
            .collect();

        // code view
        if let (Some(source), Some(source_view)) = (&context.source, &context.source_view) {
            // some constants to help with the scroll area show
            let font_id = egui::FontId::new(MONOSPACE_SIZE, egui::FontFamily::Monospace);
            let row_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font_id));
            let num_lines = source.contents().num_lines();
            let num_lines_digits = (num_lines as f32).log10().ceil() as usize;
            let code_colors = CodeColors::get(ui);

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show_rows(ui, row_height, num_lines, |ui, row_range| {
                    for i in row_range {
                        let (line_rect, response) = ui.allocate_exact_size(
                            vec2(ui.available_width(), row_height), Sense::click());
                        let galley = ui.painter().layout_job(source_view.generate_job(
                            source,
                            &probe_definitions,
                            i,
                            num_lines_digits,
                            &code_colors,
                        ));
                        let mouse_pos = if response.hovered() { galley.cursor_from_pos(response.hover_pos().unwrap_or_default()- line_rect.min).index} else { CharIndex(0)};
                        if mouse_pos > CharIndex(num_lines_digits) && let Some(signal) = source_view.signal_at_pos(i, mouse_pos - (num_lines_digits + 1)) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            if response.clicked() {
                                if let Some(probe) = probe_definitions.get(&signal) {
                                    context.source_view_signal_popup_name = Some(signal);
                                    context.source_view_signal_probe = probe.clone();
                                    context.source_view_signal_popup_position = response.hover_pos().unwrap_or_default();
                                }
                            }
                        }

                        ui.painter().galley(line_rect.min, galley, Color32::PLACEHOLDER);
                    }
                });
            
        }
    });

        // display the popup to change a probe
    let mut current_probe_selection = context.source_view_signal_probe.clone();
    let popup_signal_name = context.source_view_signal_popup_name.clone();

    if let Some(signal_name) = popup_signal_name {
        let mut open = true;
        egui::Popup::from_response(&uiresponse.response)
            .open_bool(&mut open)
            .at_position(context.source_view_signal_popup_position)
            .show(|ui| {
                let options = [
                    None,
                    Some(ProbeChannel::EioIn),
                    Some(ProbeChannel::EioOut),
                    Some(ProbeChannel::Ela(0)),
                ];
                for option in options {
                    ui.selectable_value(
                        &mut current_probe_selection,
                        option.clone(),
                        super::signals::probe_channel_to_text(&option, context),
                    );
                }

            });

        if current_probe_selection != context.source_view_signal_probe {
            // selection has been changed
            context.source_view_signal_popup_name = None;
            let channel_text = match current_probe_selection {
                None => "None".to_owned(),
                Some(channel) => channel.to_string(),
            };
            context.execute_tcl(&format!(
                "change_probe {} {} {}",
                context.selected_path, signal_name, channel_text
            ));
        }
        if !open {
            context.source_view_signal_popup_name = None;
        }
    }

}
