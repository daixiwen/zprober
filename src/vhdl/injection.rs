//! Injection of code blocks into VHDL source files
//!
//! Zprober can inject blocks of code at three kinds of positions of a VHDL
//! file, all with a single [`inject`] call:
//!
//! - at the end of the ports list of the entity declaration
//! - at the end of the declarative region of the architecture(s), just
//!   before the `begin` keyword
//! - at the end of the port map of one or several (named) instantiations
//!
//! Every injected block is wrapped in the `-- ZProber[[` / `-- ZProber]]`
//! marker comments. When injecting into a file that already contains such
//! blocks, the existing blocks are replaced by the new content (an existing
//! block whose zone is not part of the new injection is simply removed).
//! Marker lines may carry arbitrary trailing content, e.g.
//! `-- ZProber[[ some note`, and are matched case-insensitively.
//!
//! [`remove_injections`] strips every injected block (markers included)
//! from a source text, restoring the original code. It works on the plain
//! text and does not parse the VHDL.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use vhdl_lang::ast::{
    AnyDesignUnit, AnyPrimaryUnit, AnySecondaryUnit, ConcurrentStatement, EntityDeclaration,
    InterfaceDeclaration, LabeledConcurrentStatement, ModeIndication,
};
use vhdl_lang::{NullDiagnostics, Position, Source, Token, TokenAccess, VHDLParser, VHDLStandard};

/// Comment introducing an injected code block
pub const START_MARKER: &str = "-- ZProber[[";
pub const WARNING_IN_MARKER: &str = "DO NOT EDIT THE LINES BELOW";

/// Comment terminating an injected code block
pub const END_MARKER: &str = "-- ZProber]]";

/// The code blocks to inject into a VHDL source, per injection zone
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InjectionPoints {
    /// Code appended at the end of the ports list of every entity
    /// declaration of the file
    pub entity_ports: Option<String>,
    /// Code appended at the end of the declarative region of every
    /// architecture of the file, just before `begin`
    pub architecture_declarations: Option<String>,
    /// Code appended at the end of the port map of the named
    /// instantiations, keyed by instantiation label (matched
    /// case-insensitively, as VHDL labels are)
    pub instantiation_ports: BTreeMap<String, String>,
}

impl InjectionPoints {
    /// Whether no injection is requested at all
    pub fn is_empty(&self) -> bool {
        self.entity_ports.is_none()
            && self.architecture_declarations.is_none()
            && self.instantiation_ports.is_empty()
    }

    /// Request an injection at the end of the entity ports list
    pub fn with_entity_ports(mut self, code: impl Into<String>) -> Self {
        self.entity_ports = Some(code.into());
        self
    }

    /// Request an injection at the end of the architecture declarative region
    pub fn with_architecture_declarations(mut self, code: impl Into<String>) -> Self {
        self.architecture_declarations = Some(code.into());
        self
    }

    /// Request an injection at the end of the port map of an instantiation,
    /// identified by its label
    pub fn with_instantiation_ports(
        mut self,
        label: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        self.instantiation_ports.insert(label.into(), code.into());
        self
    }
}

/// Inject code blocks into a VHDL source text.
///
/// `points` describes which zones to inject. Blocks already present in the
/// source are first removed, so existing injections are replaced (or
/// dropped when their zone is not part of `points`).
///
/// The source is parsed with `vhdl_lang` (VHDL-2008) to locate the
/// injection points.
pub fn inject(source: &str, points: &InjectionPoints) -> Result<String, String> {
    // Replacing an injection means removing the previous blocks first
    let clean = remove_injections(source)?;

    if points.is_empty() {
        return Ok(clean);
    }

    let parser = VHDLParser::new(VHDLStandard::default());
    let parsed_source = Source::inline(Path::new("zprober_injection.vhd"), &clean);
    let mut diagnostics = NullDiagnostics;
    let design_file = parser.parse_design_source(&parsed_source, &mut diagnostics);

    let mut insertions: Vec<Insertion> = Vec::new();
    let mut entity_injected = false;
    let mut architecture_injected = false;
    let mut labels_found: BTreeSet<String> = BTreeSet::new();

    for (tokens, unit) in &design_file.design_units {
        match unit {
            AnyDesignUnit::Primary(AnyPrimaryUnit::Entity(entity)) => {
                if let Some(code) = &points.entity_ports {
                    let end = entity_port_list_end(entity, tokens)?;
                    insertions.push(Insertion::after_token(&clean, end, code)?);
                    entity_injected = true;
                }
            }
            AnyDesignUnit::Secondary(AnySecondaryUnit::Architecture(architecture)) => {
                if let Some(code) = &points.architecture_declarations {
                    let begin_line = tokens.get_pos(architecture.begin_token).range.start.line;
                    insertions.push(Insertion::at_line_start(&clean, begin_line, code)?);
                    architecture_injected = true;
                }
                walk_statements(
                    &clean,
                    tokens,
                    &architecture.statements,
                    points,
                    &mut insertions,
                    &mut labels_found,
                )?;
            }
            _ => {} // Packages, contexts and configurations: no injection zone
        }
    }

    if points.entity_ports.is_some() && !entity_injected {
        return Err("no entity declaration found in the file".to_string());
    }
    if points.architecture_declarations.is_some() && !architecture_injected {
        return Err("no architecture declaration found in the file".to_string());
    }
    for label in points.instantiation_ports.keys() {
        if !labels_found
            .iter()
            .any(|found| found.eq_ignore_ascii_case(label))
        {
            return Err(format!("instantiation '{}' not found in the file", label));
        }
    }

    apply_insertions(clean, insertions)
}

/// Remove every injected code block (markers included) from a VHDL source
/// text.
///
/// This works on the plain text and does not parse the VHDL: a block is
/// the sequence of lines starting at a line prefixed with `-- Zprober[[`
/// and ending at the next line prefixed with `-- ZProber]]` (both matched
/// case-insensitively, ignoring leading whitespace and tolerating any
/// trailing content on the marker lines).
pub fn remove_injections(source: &str) -> Result<String, String> {
    let start_prefix = START_MARKER.to_lowercase();
    let end_prefix = END_MARKER.to_lowercase();

    let mut kept: Vec<&str> = Vec::new();
    let mut in_block = false;
    for line in source.lines() {
        let prefix = line.trim_start().to_lowercase();
        if in_block {
            if prefix.starts_with(&end_prefix) {
                in_block = false;
            }
            // Lines inside a block (including both marker lines) are dropped
        } else if prefix.starts_with(&start_prefix) {
            in_block = true;
        } else {
            kept.push(line);
        }
    }
    if in_block {
        return Err(format!(
            "unterminated injection block: missing '{}' marker",
            END_MARKER
        ));
    }

    // Rebuild the text, preserving the line ending style and the presence
    // of a trailing newline
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut result = kept.join(newline);
    if source.ends_with('\n') {
        result.push_str(newline);
    }
    Ok(result)
}

/// Render an injected block: the code wrapped in the marker comments, each
/// marker on its own line.
///
/// `trailing_newline` controls whether the block ends with a newline: a
/// block inserted at the start of a line does (the following line is kept
/// intact), a block inserted in the middle of a line does not (the rest of
/// the original line already starts with one).
fn block_text(code: &str, trailing_newline: bool) -> String {
    let code = code.trim_matches(|c: char| c == '\n' || c == '\r');
    if trailing_newline {
        format!(
            "{} {}\n{}\n{}\n",
            START_MARKER, WARNING_IN_MARKER, code, END_MARKER
        )
    } else {
        format!(
            "{} {}\n{}\n{}",
            START_MARKER, WARNING_IN_MARKER, code, END_MARKER
        )
    }
}

/// A pending insertion of an injected block into the source text
struct Insertion {
    /// Byte offset in the (block-stripped) source
    offset: usize,
    /// The text to insert at that offset
    text: String,
}

impl Insertion {
    /// Insertion just after the token ending at `position`: the block is
    /// written on the following lines, the rest of the current line (e.g.
    /// the closing `);`) is preserved
    fn after_token(source: &str, position: Position, code: &str) -> Result<Insertion, String> {
        let offset = position_to_offset(source, position).ok_or_else(|| {
            format!(
                "position {}:{} is outside of the source",
                position.line + 1,
                position.character
            )
        })?;
        Ok(Insertion {
            offset,
            text: format!("\n{}", block_text(code, false)),
        })
    }

    /// Insertion at the very beginning of `line`: the block is written on
    /// the preceding lines
    fn at_line_start(source: &str, line: u32, code: &str) -> Result<Insertion, String> {
        let offset = position_to_offset(source, Position::new(line, 0))
            .ok_or_else(|| format!("line {} is outside of the source", line + 1))?;
        Ok(Insertion {
            offset,
            text: block_text(code, true),
        })
    }
}

/// Convert a (zero-based line, zero-based utf-16 column) position into a
/// byte offset in `source`
fn position_to_offset(source: &str, position: Position) -> Option<usize> {
    let mut line_start = 0usize;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index as u32 == position.line {
            let mut utf16 = 0u32;
            for (offset, character) in line.char_indices() {
                if utf16 >= position.character {
                    return Some(line_start + offset);
                }
                utf16 += character.len_utf16() as u32;
            }
            return Some(line_start + line.len());
        }
        line_start += line.len();
    }
    None
}

/// Position of the end of the last port of an entity's port declaration
fn entity_port_list_end(entity: &EntityDeclaration, tokens: &[Token]) -> Result<Position, String> {
    let name = entity.ident.tree.item.name_utf8();
    let Some(list) = &entity.port_clause else {
        return Err(format!("entity '{}' has no port declaration", name));
    };
    let Some(last) = list.items.last() else {
        return Err(format!("port declaration of entity '{}' is empty", name));
    };
    let InterfaceDeclaration::Object(object) = last else {
        return Err(format!(
            "the last port of entity '{}' is not an object declaration",
            name
        ));
    };
    let ModeIndication::Simple(simple) = &object.mode else {
        return Err(format!(
            "the last port of entity '{}' has no simple mode indication",
            name
        ));
    };

    // The end of the last port is the end of its subtype indication
    // (constraint included) or of its default value expression,
    // whichever comes last
    let mut end = tokens
        .get_pos(simple.subtype_indication.type_mark.span.end_token)
        .range
        .end;
    if let Some(constraint) = &simple.subtype_indication.constraint {
        let constraint_end = tokens.get_pos(constraint.span.end_token).range.end;
        if constraint_end > end {
            end = constraint_end;
        }
    }
    if let Some(expression) = &simple.expression {
        let expression_end = tokens.get_pos(expression.span.end_token).range.end;
        if expression_end > end {
            end = expression_end;
        }
    }
    Ok(end)
}

/// Recursively walk concurrent statements, recording a port map injection
/// for every instantiation whose label is listed in `points`.
///
/// `labels_found` collects the (lowercased) labels that were matched, so
/// the caller can detect requested labels that are missing from the file.
fn walk_statements(
    source: &str,
    tokens: &[Token],
    statements: &[LabeledConcurrentStatement],
    points: &InjectionPoints,
    insertions: &mut Vec<Insertion>,
    labels_found: &mut BTreeSet<String>,
) -> Result<(), String> {
    for statement in statements {
        if let Some(ident) = &statement.label.tree {
            let label = ident.item.name_utf8();
            if let Some((_, code)) = points
                .instantiation_ports
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&label))
            {
                let ConcurrentStatement::Instance(inst) = &statement.statement.item else {
                    return Err(format!(
                        "'{}' does not label a component instantiation",
                        label
                    ));
                };
                let Some(port_map) = &inst.port_map else {
                    return Err(format!("instantiation '{}' has no port map", label));
                };
                let Some(last) = port_map.list.items.last() else {
                    return Err(format!("port map of instantiation '{}' is empty", label));
                };
                let position = tokens.get_pos(last.actual.span.end_token).range.end;
                insertions.push(Insertion::after_token(source, position, code)?);
                labels_found.insert(label.to_lowercase());
            }
        }
        match &statement.statement.item {
            ConcurrentStatement::Block(block) => {
                walk_statements(
                    source,
                    tokens,
                    &block.statements,
                    points,
                    insertions,
                    labels_found,
                )?;
            }
            ConcurrentStatement::ForGenerate(gen_stmt) => {
                walk_statements(
                    source,
                    tokens,
                    &gen_stmt.body.statements,
                    points,
                    insertions,
                    labels_found,
                )?;
            }
            ConcurrentStatement::IfGenerate(gen_stmt) => {
                for conditional in &gen_stmt.conds.conditionals {
                    walk_statements(
                        source,
                        tokens,
                        &conditional.item.statements,
                        points,
                        insertions,
                        labels_found,
                    )?;
                }
                if let Some((else_body, _)) = &gen_stmt.conds.else_item {
                    walk_statements(
                        source,
                        tokens,
                        &else_body.statements,
                        points,
                        insertions,
                        labels_found,
                    )?;
                }
            }
            ConcurrentStatement::CaseGenerate(gen_stmt) => {
                for alternative in &gen_stmt.sels.alternatives {
                    walk_statements(
                        source,
                        tokens,
                        &alternative.item.statements,
                        points,
                        insertions,
                        labels_found,
                    )?;
                }
            }
            _ => {} // Other statements do not contain instantiations
        }
    }
    Ok(())
}

/// Apply the pending insertions to the source text
fn apply_insertions(mut source: String, mut insertions: Vec<Insertion>) -> Result<String, String> {
    // Insert from the end of the file backwards, so that the offsets of
    // the pending insertions remain valid
    insertions.sort_by_key(|insertion| std::cmp::Reverse(insertion.offset));
    if let Some(duplicate) = insertions
        .windows(2)
        .find(|pair| pair[0].offset == pair[1].offset)
    {
        return Err(format!(
            "two injections at the same position (offset {})",
            duplicate[0].offset
        ));
    }
    for insertion in insertions {
        source.insert_str(insertion.offset, &insertion.text);
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("example")
            .join("injection_test")
    }

    fn read(file: &str) -> String {
        std::fs::read_to_string(test_dir().join(file))
            .unwrap_or_else(|e| panic!("failed to read {}: {}", file, e))
    }

    /// Inject the three zones into original.vhd
    #[test]
    fn test_inject_original() {
        let source = read("original.vhd");
        let points = InjectionPoints::default()
            .with_entity_ports(";\n    debug : out std_logic_vector(7 downto 0)")
            .with_architecture_declarations(
                "    signal extra_signal : std_logic_vector(31 downto 0);",
            )
            .with_instantiation_ports("i_component_1", ",\n    debug => debug");
        let result = inject(&source, &points).unwrap();
        assert_eq!(result, read("modified_1_out.vhd"));
    }

    /// re-injecting into a file that already contains blocks replaces
    /// them (the marker lines may carry trailing content)
    #[test]
    fn test_inject_replaces_existing_blocks() {
        let source = read("modified_2_in.vhd");
        let points = InjectionPoints::default()
            .with_entity_ports(";\n    new_debug : out std_logic_vector(15 downto 0)")
            .with_architecture_declarations(
                "    signal new_extra_signal : std_logic_vector(15 downto 0);",
            )
            .with_instantiation_ports("i_component_1", ",\n    new_debug => new_debug");
        let result = inject(&source, &points).unwrap();
        assert_eq!(result, read("modified_2_out.vhd"));
    }

    /// Removing the blocks of modified_3_in.vhd restores original.vhd
    #[test]
    fn test_remove_injections() {
        let source = read("modified_3_in.vhd");
        let result = remove_injections(&source).unwrap();
        assert_eq!(result, read("original.vhd"));
    }

    /// Instantiation labels are matched case-insensitively, like any VHDL
    /// identifier
    #[test]
    fn test_instantiation_label_case_insensitive() {
        let source = read("original.vhd");
        let points = InjectionPoints::default()
            .with_entity_ports(";\n    debug : out std_logic_vector(7 downto 0)")
            .with_architecture_declarations(
                "    signal extra_signal : std_logic_vector(31 downto 0);",
            )
            .with_instantiation_ports("I_Component_1", ",\n    debug => debug");
        let result = inject(&source, &points).unwrap();
        assert_eq!(result, read("modified_1_out.vhd"));
    }

    /// Injecting the same content twice is a no-op the second time
    #[test]
    fn test_inject_is_idempotent() {
        let source = read("original.vhd");
        let points = InjectionPoints::default()
            .with_entity_ports(";\n    debug : out std_logic_vector(7 downto 0)")
            .with_architecture_declarations(
                "    signal extra_signal : std_logic_vector(31 downto 0);",
            )
            .with_instantiation_ports("i_component_1", ",\n    debug => debug");
        let once = inject(&source, &points).unwrap();
        let twice = inject(&once, &points).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn test_unknown_instantiation_is_rejected() {
        let source = read("original.vhd");
        let points = InjectionPoints::default().with_instantiation_ports("no_such_label", ",");
        assert!(inject(&source, &points).is_err());
    }

    #[test]
    fn test_unterminated_block_is_rejected() {
        let source = "-- Zprober[[\nfoo\n";
        assert!(remove_injections(source).is_err());
        assert!(inject(source, &InjectionPoints::default()).is_err());
    }
}
