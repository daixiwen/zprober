//! Injection of code blocks into VHDL source files
//!
//! Zprober can inject blocks of code at several kinds of positions of a
//! VHDL file, all with a single [`inject`] call:
//!
//! - at the end of the ports list of the entity declaration
//! - at the end of the declarative region of the architecture(s), just
//!   before the `begin` keyword
//! - at the beginning of the statements region of the architecture(s),
//!   just after the `begin` keyword
//! - at the end of the port map of one or several (named) instantiations
//!
//! Additionally, the association lists of the generic and/or port maps of
//! one or several (named) instantiations can be replaced wholesale,
//! without the marker comments (see [`MapReplacement`]).
//!
//! Every injected block is wrapped in the `-- ZProber[[ DO NOT EDIT THE
//! LINES BELOW` / `-- ZProber]]` marker comments. When injecting into a
//! file that already contains such blocks, the existing blocks are
//! replaced by the new content (an existing block whose zone is not part
//! of the new injection is simply removed). Marker lines may carry
//! arbitrary trailing content, e.g. `-- ZProber[[ some note`, and are
//! matched case-insensitively.
//!
//! [`remove_injections`] strips every injected block (markers included)
//! from a source text, restoring the original code. It works on the plain
//! text and does not parse the VHDL.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use vhdl_lang::ast::{
    AnyDesignUnit, AnyPrimaryUnit, AnySecondaryUnit, AssociationElement, ConcurrentStatement,
    EntityDeclaration, InterfaceDeclaration, LabeledConcurrentStatement, ModeIndication,
};
use vhdl_lang::{NullDiagnostics, Position, Source, Token, TokenAccess, VHDLParser, VHDLStandard};

/// Comment introducing an injected code block
pub const START_MARKER: &str = "-- ZProber[[";

/// Trailing content of the start marker line
pub const WARNING_IN_MARKER: &str = "DO NOT EDIT THE LINES BELOW";

/// Comment terminating an injected code block
pub const END_MARKER: &str = "-- ZProber]]";

/// A map replacement requested for an instantiation (no markers involved):
/// either or both association lists are replaced wholesale, keeping the
/// surrounding syntax (the `generic map (` / `port map (` / `)` lines)
/// untouched
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapReplacement {
    /// Replacement for the association list of the generic map
    pub generic_map: Option<String>,
    /// Replacement for the association list of the port map
    pub port_map: Option<String>,
}

impl MapReplacement {
    /// Whether no map replacement is requested at all
    pub fn is_empty(&self) -> bool {
        self.generic_map.is_none() && self.port_map.is_none()
    }

    /// Replace the association list of the generic map
    pub fn with_generic_map(mut self, code: impl Into<String>) -> Self {
        self.generic_map = Some(code.into());
        self
    }

    /// Replace the association list of the port map
    pub fn with_port_map(mut self, code: impl Into<String>) -> Self {
        self.port_map = Some(code.into());
        self
    }
}

/// The code blocks to inject into a VHDL source, per injection zone
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InjectionPoints {
    /// Code appended at the end of the ports list of every entity
    /// declaration of the file
    pub entity_ports: Option<String>,
    /// Code appended at the end of the declarative region of every
    /// architecture of the file, just before `begin`
    pub architecture_declarations: Option<String>,
    /// Code appended at the beginning of the statements region of every
    /// architecture of the file, just after `begin`
    pub architecture_statements: Option<String>,
    /// Code appended at the end of the port map of the named
    /// instantiations, keyed by instantiation label (matched
    /// case-insensitively, as VHDL labels are)
    pub instantiation_ports: BTreeMap<String, String>,
    /// Map replacements requested for the named instantiations, keyed by
    /// instantiation label (matched case-insensitively, as VHDL labels
    /// are)
    pub instantiation_map_replacements: BTreeMap<String, MapReplacement>,
}

impl InjectionPoints {
    /// Whether no injection is requested at all
    pub fn is_empty(&self) -> bool {
        self.entity_ports.is_none()
            && self.architecture_declarations.is_none()
            && self.architecture_statements.is_none()
            && self.instantiation_ports.is_empty()
            && self.instantiation_map_replacements.is_empty()
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

    /// Request an injection at the beginning of the architecture statements
    pub fn with_architecture_statements(mut self, code: impl Into<String>) -> Self {
        self.architecture_statements = Some(code.into());
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

    /// Request a wholesale replacement of the generic and/or port maps of
    /// an instantiation, identified by its label
    pub fn with_instantiation_map_replacement(
        mut self,
        label: impl Into<String>,
        replacement: MapReplacement,
    ) -> Self {
        self.instantiation_map_replacements
            .insert(label.into(), replacement);
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

    let mut edits: Vec<Edit> = Vec::new();
    let mut entity_handled = false;
    let mut architecture_handled = false;
    let mut labels_found: BTreeSet<String> = BTreeSet::new();

    for (tokens, unit) in &design_file.design_units {
        match unit {
            AnyDesignUnit::Primary(AnyPrimaryUnit::Entity(entity)) => {
                if let Some(code) = &points.entity_ports {
                    let end = entity_port_list_end(entity, tokens)?;
                    edits.push(Edit::after_token(&clean, end, code)?);
                    entity_handled = true;
                }
            }
            AnyDesignUnit::Secondary(AnySecondaryUnit::Architecture(architecture)) => {
                let begin_line = tokens.get_pos(architecture.begin_token).range.start.line;
                if let Some(code) = &points.architecture_declarations {
                    edits.push(Edit::at_line_start(&clean, begin_line, code)?);
                    architecture_handled = true;
                }
                if let Some(code) = &points.architecture_statements {
                    edits.push(Edit::after_line(&clean, begin_line, code)?);
                    architecture_handled = true;
                }
                walk_statements(
                    &clean,
                    tokens,
                    &architecture.statements,
                    points,
                    &mut edits,
                    &mut labels_found,
                )?;
            }
            _ => {} // Packages, contexts and configurations: no injection zone
        }
    }

    if points.entity_ports.is_some() && !entity_handled {
        return Err("no entity declaration found in the file".to_string());
    }
    if (points.architecture_declarations.is_some() || points.architecture_statements.is_some())
        && !architecture_handled
    {
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
    for label in points.instantiation_map_replacements.keys() {
        if !labels_found
            .iter()
            .any(|found| found.eq_ignore_ascii_case(label))
        {
            return Err(format!("instantiation '{}' not found in the file", label));
        }
    }

    apply_edits(clean, edits)
}

/// Remove every injected code block (markers included) from a VHDL source
/// text.
///
/// This works on the plain text and does not parse the VHDL: a block is
/// the sequence of lines starting at a line prefixed with `-- ZProber[[`
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

/// A pending edit of the (block-stripped) source text: an insertion when
/// `start == end`, or a replacement of the range `start..end` otherwise
/// (used for the marker-free map replacements)
struct Edit {
    /// Byte offset in the (block-stripped) source where the edit starts
    start: usize,
    /// Byte offset where the edit ends (equal to `start` for an insertion)
    end: usize,
    /// The text to write at that offset
    text: String,
}

impl Edit {
    /// Insertion just after the token ending at `position`: the block is
    /// written on the following lines, the rest of the current line (e.g.
    /// the closing `);`) is preserved
    fn after_token(source: &str, position: Position, code: &str) -> Result<Edit, String> {
        let offset = position_to_offset(source, position).ok_or_else(|| {
            format!(
                "position {}:{} is outside of the source",
                position.line + 1,
                position.character
            )
        })?;
        Ok(Edit {
            start: offset,
            end: offset,
            text: format!("\n{}", block_text(code, false)),
        })
    }

    /// Insertion at the very beginning of `line`: the block is written on
    /// the preceding lines
    fn at_line_start(source: &str, line: u32, code: &str) -> Result<Edit, String> {
        let offset = position_to_offset(source, Position::new(line, 0))
            .ok_or_else(|| format!("line {} is outside of the source", line + 1))?;
        Ok(Edit {
            start: offset,
            end: offset,
            text: block_text(code, true),
        })
    }

    /// Insertion at the very beginning of the line following `line`: the
    /// block is written on the lines separating the current line and the
    /// following one
    fn after_line(source: &str, line: u32, code: &str) -> Result<Edit, String> {
        let offset = position_to_offset(source, Position::new(line + 1, 0))
            .ok_or_else(|| format!("line {} is outside of the source", line + 2))?;
        Ok(Edit {
            start: offset,
            end: offset,
            text: block_text(code, true),
        })
    }

    /// Replacement of the range between two positions (no markers)
    fn replace_range(
        source: &str,
        start: Position,
        end: Position,
        code: &str,
    ) -> Result<Edit, String> {
        let start_offset = position_to_offset(source, start).ok_or_else(|| {
            format!(
                "position {}:{} is outside of the source",
                start.line + 1,
                start.character
            )
        })?;
        let end_offset = position_to_offset(source, end).ok_or_else(|| {
            format!(
                "position {}:{} is outside of the source",
                end.line + 1,
                end.character
            )
        })?;
        Ok(Edit {
            start: start_offset,
            end: end_offset,
            text: code.to_string(),
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
/// and/or map replacement for every instantiation whose label is listed in
/// `points`.
///
/// `labels_found` collects the (lowercased) labels that were matched, so
/// the caller can detect requested labels that are missing from the file.
fn walk_statements(
    source: &str,
    tokens: &[Token],
    statements: &[LabeledConcurrentStatement],
    points: &InjectionPoints,
    edits: &mut Vec<Edit>,
    labels_found: &mut BTreeSet<String>,
) -> Result<(), String> {
    for statement in statements {
        if let Some(ident) = &statement.label.tree {
            let label = ident.item.name_utf8();
            let is_port_requested = points
                .instantiation_ports
                .keys()
                .any(|name| name.eq_ignore_ascii_case(&label));
            let is_map_requested = points
                .instantiation_map_replacements
                .keys()
                .any(|name| name.eq_ignore_ascii_case(&label));
            if is_port_requested || is_map_requested {
                let ConcurrentStatement::Instance(inst) = &statement.statement.item else {
                    return Err(format!(
                        "'{}' does not label a component instantiation",
                        label
                    ));
                };
                if is_port_requested {
                    let code = points
                        .instantiation_ports
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(&label))
                        .map(|(_, code)| code)
                        .unwrap();
                    let Some(port_map) = &inst.port_map else {
                        return Err(format!("instantiation '{}' has no port map", label));
                    };
                    let Some(last) = port_map.list.items.last() else {
                        return Err(format!("port map of instantiation '{}' is empty", label));
                    };
                    let position = tokens.get_pos(last.actual.span.end_token).range.end;
                    edits.push(Edit::after_token(source, position, code)?);
                }
                if is_map_requested {
                    let replacement = points
                        .instantiation_map_replacements
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(&label))
                        .map(|(_, replacement)| replacement)
                        .unwrap();
                    if let Some(code) = &replacement.generic_map {
                        let Some(map) = &inst.generic_map else {
                            return Err(format!("instantiation '{}' has no generic map", label));
                        };
                        edits.push(Edit::replace_range(
                            source,
                            map_list_start(&map.list, tokens)?,
                            map_list_end(&map.list, tokens)?,
                            code,
                        )?);
                    }
                    if let Some(code) = &replacement.port_map {
                        let Some(map) = &inst.port_map else {
                            return Err(format!("instantiation '{}' has no port map", label));
                        };
                        edits.push(Edit::replace_range(
                            source,
                            map_list_start(&map.list, tokens)?,
                            map_list_end(&map.list, tokens)?,
                            code,
                        )?);
                    }
                }
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
                    edits,
                    labels_found,
                )?;
            }
            ConcurrentStatement::ForGenerate(gen_stmt) => {
                walk_statements(
                    source,
                    tokens,
                    &gen_stmt.body.statements,
                    points,
                    edits,
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
                        edits,
                        labels_found,
                    )?;
                }
                if let Some((else_body, _)) = &gen_stmt.conds.else_item {
                    walk_statements(
                        source,
                        tokens,
                        &else_body.statements,
                        points,
                        edits,
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
                        edits,
                        labels_found,
                    )?;
                }
            }
            _ => {} // Other statements do not contain instantiations
        }
    }
    Ok(())
}

/// Position of the start of the first association element of a map list
fn map_list_start(
    list: &vhdl_lang::ast::SeparatedList<AssociationElement>,
    tokens: &[Token],
) -> Result<Position, String> {
    let Some(first) = list.items.first() else {
        return Err("the map list is empty".to_string());
    };
    let start = if let Some(formal) = &first.formal {
        formal.span.start_token
    } else {
        first.actual.span.start_token
    };
    Ok(tokens.get_pos(start).range.start)
}

/// Position of the end of the last association element of a map list
fn map_list_end(
    list: &vhdl_lang::ast::SeparatedList<AssociationElement>,
    tokens: &[Token],
) -> Result<Position, String> {
    let Some(last) = list.items.last() else {
        return Err("the map list is empty".to_string());
    };
    Ok(tokens.get_pos(last.actual.span.end_token).range.end)
}

/// Apply the pending edits to the source text
fn apply_edits(mut source: String, mut edits: Vec<Edit>) -> Result<String, String> {
    // Apply from the end of the file backwards, so that the offsets of
    // the pending edits remain valid
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    if let Some(duplicate) = edits
        .windows(2)
        .find(|pair| pair[1].start == pair[0].start || pair[1].end > pair[0].start)
    {
        return Err(format!(
            "two edits overlapping at offset {}",
            duplicate[0].start
        ));
    }
    for edit in edits {
        source.replace_range(edit.start..edit.end, &edit.text);
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

    /// instructions_1.md: inject the three zones into original.vhd
    #[test]
    fn test_inject_original() {
        let source = read("original.vhd");
        let points = InjectionPoints::default()
            .with_entity_ports(";\n    debug : out std_logic_vector(7 downto 0)")
            .with_architecture_declarations(
                "    signal extra_signal : std_logic_vector(31 downto 0);",
            )
            .with_architecture_statements("    extra_signal <= data_internal;")
            .with_instantiation_map_replacement(
                "i_component_0",
                MapReplacement::default()
                    .with_generic_map("const_a => 3,\n            const_b => 4")
                    .with_port_map(
                        "clk => clk,\n            reset => reset,\n            data_in => data_in,\n            data_out => extra_signal",
                    ),
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
            .with_architecture_statements("    extra_signal <= data_internal;")
            .with_instantiation_map_replacement(
                "I_Component_0",
                MapReplacement::default()
                    .with_generic_map("const_a => 3,\n            const_b => 4")
                    .with_port_map(
                        "clk => clk,\n            reset => reset,\n            data_in => data_in,\n            data_out => extra_signal",
                    ),
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
            .with_architecture_statements("    extra_signal <= data_internal;")
            .with_instantiation_map_replacement(
                "i_component_0",
                MapReplacement::default()
                    .with_generic_map("const_a => 3,\n            const_b => 4")
                    .with_port_map(
                        "clk => clk,\n            reset => reset,\n            data_in => data_in,\n            data_out => extra_signal",
                    ),
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
    fn test_unknown_map_replacement_is_rejected() {
        let source = read("original.vhd");
        let points = InjectionPoints::default().with_instantiation_map_replacement(
            "no_such_label",
            MapReplacement::default().with_port_map(","),
        );
        assert!(inject(&source, &points).is_err());
    }

    #[test]
    fn test_unterminated_block_is_rejected() {
        let source = "-- Zprober[[\nfoo\n";
        assert!(remove_injections(source).is_err());
        assert!(inject(source, &InjectionPoints::default()).is_err());
    }
}
