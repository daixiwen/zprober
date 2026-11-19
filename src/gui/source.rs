use egui::text::{ByteIndex, ByteRange, CharIndex};
use egui::text_selection::text_cursor_state::byte_index_from_char_index;

use vhdl_lang::Kind;

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
    Signal(String),               // signal/port name
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ColorKind {
    Comment,
    Keyword,
    Operator,
    Name,
    Literal,
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
        ByteIndex(text.len()-1),  // remove 1 because all lines from vhdl_lang::source include an \n at the end
        kind,
    );
}

impl SourceView {
    /// create a source view from a parsed VHDL file
    pub fn new(source: &vhdl_lang::Source, design_file: &vhdl_lang::ast::DesignFile) -> Self {
        let mut sections: Vec<Vec<SourceSection>> = Vec::new();
        let signal_tokens_vec = crate::vhdl::signal_tokens::extract_tokens(&design_file);
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
                    let section_kind = if let Some(token_pos) = current_signal_token && token_pos.pos_match(current_position) {
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

    /// generate a vector of layout jobs from the source view
    pub fn generate_job(&self, source: &vhdl_lang::Source, line_number: usize) -> egui::text::LayoutJob {
        let line = source.contents().get_line(line_number).map(|s| {
            let mut s = s.to_owned();
            // remove the end-of-line at the end
            s.pop();
            s
        }).unwrap_or_default();
        let line_length = line.len();

        fn create_section(range: &ByteRange, color: egui::Color32) -> egui::text::LayoutSection {
            egui::text::LayoutSection{
                leading_space: 0.0,
                byte_range: range.to_owned(),
                format:egui::text::TextFormat{
                        font_id: egui::FontId::monospace(12.0),
                        extra_letter_spacing: 0.0,
                        line_height: None,
                        color: color,
                        background: egui::Color32::BLACK,
                        expand_bg: 1.0,
                        coords: Default::default(),
                        italics: false,
                        underline: egui::Stroke::NONE,
                        strikethrough: egui::Stroke::NONE,
                        valign: egui::Align::Center,
                }
            }            
        }
        let sections = match self.sections.get(line_number) {
            None => vec![create_section(&ByteRange{
                start: ByteIndex(0),
                end: ByteIndex(line_length-1)
            }, egui::Color32::BLUE)],
            Some(source_sections) => {
                source_sections.iter().map(|ss|
                    create_section(&ss.range, match ss.kind {
                        SourceSectionKind::FixedText(ColorKind::Comment) => egui::Color32::GRAY,
                        SourceSectionKind::FixedText(ColorKind::Keyword) => egui::Color32::RED,
                        SourceSectionKind::FixedText(ColorKind::Literal) => egui::Color32::BLUE,
                        SourceSectionKind::FixedText(ColorKind::Name) => egui::Color32::GREEN,
                        SourceSectionKind::FixedText(ColorKind::Operator) => egui::Color32::YELLOW,
                        _ => egui::Color32::WHITE
                    }
                    )).collect()
                }
        };
        egui::text::LayoutJob {
            text: line,
            sections: sections,
            wrap: egui::text::TextWrapping{
                max_width: f32::MAX,
                max_rows: 1,
                break_anywhere: true,
                overflow_character: None
            },
            first_row_min_height: 0.0,
            break_on_newline: true,
            halign: egui::Align::LEFT,
            justify: false,
            round_output_to_gui: true,
            keep_trailing_whitespace: true
        }
    }
}

/// pane with the source file
pub fn panel(ui: &mut egui::Ui, context: &crate::app::ZProberAppContext) {
    ui.vertical(|ui| {
        // top line with the path
        ui.horizontal(|ui| {
            if ui.button("<").clicked() {
                tracing::event!(tracing::Level::DEBUG, "should implement button");
            }
            ui.label(&context.selected_path)
        });

        // code view
        if let (Some(source), Some(source_view)) = (&context.source, &context.source_view) {
            egui::ScrollArea::both().auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible).show(ui, |ui| {
                ui.vertical(|ui| {
                    for i in 0..source.contents().num_lines() {
                        ui.label(source_view.generate_job(source, i));
                    }
                });
            });
        }
    });
}
