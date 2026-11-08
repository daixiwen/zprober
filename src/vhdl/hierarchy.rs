//! Instantiation hierarchy built from a VHDL project
//!
//! The [vhdl_lang] crate provides parsing and analysis of individual files,
//! but it does not build an instantiation tree. This module walks the AST of
//! every design unit in a project, records which entity instantiates which,
//! and identifies the top-level entities.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use vhdl_lang::ast::{
    AnyDesignUnit, AnyPrimaryUnit, AnySecondaryUnit, ArchitectureBody, ConcurrentStatement,
    Designator, EntityDeclaration, GenerateBody, InstantiatedUnit, LabeledConcurrentStatement,
    Name,
};
use vhdl_lang::{NullDiagnostics, VHDLParser};

use crate::vhdl::project::Project;

/// A library-qualified name of a design unit, e.g. `lib.my_module`
///
/// VHDL identifiers are case-insensitive, so equality (and hashing) is
/// case-insensitive too.
#[derive(Debug, Clone)]
pub struct QualifiedName {
    /// The library the design unit belongs to
    pub library: String,
    /// The design unit name (e.g. the entity name)
    pub name: String,
}

impl QualifiedName {
    /// Create a new qualified name
    pub fn new(library: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            library: library.into(),
            name: name.into(),
        }
    }

    /// Case-insensitive key used for comparison, deduplication and lookup
    pub fn key(&self) -> (String, String) {
        (self.library.to_lowercase(), self.name.to_lowercase())
    }
}

impl PartialEq for QualifiedName {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for QualifiedName {}

impl std::hash::Hash for QualifiedName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.", self.library)?;
        write!(f, "{}", self.name)
    }
}

/// A design unit found in a VHDL source file (entity or architecture)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignUnit {
    /// The library the design unit belongs to
    pub library: String,
    /// The source file where the design unit was declared
    pub file: PathBuf,
    /// The kind of design unit
    pub kind: DesignUnitKind,
    /// The name of the design unit (entity name or architecture name)
    pub name: String,
    /// For architectures, the name of the entity they belong to
    pub entity: Option<String>,
    /// for entities, port list, for architectures, signals list
    pub signals: Vec<Signal>,
}

/// a port or a signal
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    /// the signal name
    pub name: String,
    /// mode (in, out, inout, buffer or signal)
    pub mode: SignalMode,
    /// the signal type
    pub signal_type: String,
    /// the signal size in bits
    pub size: u32,
}

#[derive(Debug, strum::Display, Clone, PartialEq, Eq)]
pub enum SignalMode {
    In, Out, InOut, Buffer, Signal, Unknown
}

impl DesignUnit {
    /// The library-qualified name of the primary unit this design unit
    /// belongs to.
    ///
    /// For entities, this is the entity's own qualified name; for
    /// architectures, it is the qualified name of the entity they belong to.
    pub fn qualified_name(&self) -> QualifiedName {
        QualifiedName::new(
            self.library.clone(),
            self.entity.clone().unwrap_or_else(|| self.name.clone()),
        )
    }
}

/// The kind of a design unit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignUnitKind {
    /// A primary unit (entity declaration)
    Entity,
    /// A secondary unit (architecture body)
    Architecture,
}

/// An instantiation of one entity by another entity's architecture
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instantiation {
    /// The entity that contains the instantiation (the parent in the tree)
    pub parent: QualifiedName,
    /// The instantiation label
    pub label: String,
    /// The entity that is being instantiated (the child in the tree)
    pub child: QualifiedName,
    /// The kind of instantiation (direct entity, component or configuration)
    pub kind: InstantiationKind,
}

/// The kind of instantiation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstantiationKind {
    /// Direct entity instantiation, e.g. `entity work.foo`
    Entity,
    /// Component instantiation, e.g. `component foo`
    Component,
    /// Configuration instantiation
    Configuration,
}

/// The instantiation hierarchy of a VHDL project
#[derive(Debug, Clone, Default)]
pub struct Hierarchy {
    /// All design units found in the project
    pub units: Vec<DesignUnit>,
    /// All instantiations found in the project
    pub instantiations: Vec<Instantiation>,
    /// All design units that are not instantiated by anyone: candidates for top-level
    pub top_level: Vec<QualifiedName>,
}

impl Hierarchy {
    /// Build the instantiation hierarchy of a VHDL project.
    ///
    /// Parses every source file in every library of the project using
    /// `vhdl_lang::VHDLParser`, and records which entity instantiates which.
    pub fn build(project: &Project) -> Result<Self, String> {
        let parser = VHDLParser::new(project.standard());
        let mut units = Vec::new();
        let mut instantiations = Vec::new();

        for library in project.libraries() {
            for file in &library.files {
                let mut diagnostics = NullDiagnostics;
                match parser.parse_design_file(file, &mut diagnostics) {
                    Ok((source, design_file)) => {
                        let _ = source; // source is used to resolve the file name
                        process_design_file(
                            &library.name,
                            file,
                            &design_file,
                            &mut units,
                            &mut instantiations,
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read '{}': {}", file.display(), e);
                    }
                }
            }
        }

        let top_level = Self::compute_top_level(&units, &instantiations);
        Ok(Self {
            units,
            instantiations,
            top_level,
        })
    }

    /// Find the entities that are not instantiated by anyone else.
    ///
    /// This is the definition of the top-level entity: it is defined in the
    /// project, but no other entity instantiates it. Entities are compared by
    /// their library-qualified name, so two entities with the same name in
    /// different libraries are kept apart.
    fn compute_top_level(
        units: &[DesignUnit],
        instantiations: &[Instantiation],
    ) -> Vec<QualifiedName> {
        // All entities defined in the project, keyed by their qualified name
        let entities: BTreeMap<(String, String), QualifiedName> = units
            .iter()
            .filter(|u| u.kind == DesignUnitKind::Entity)
            .map(|u| (u.qualified_name().key(), u.qualified_name()))
            .collect();

        // Qualified names of entities instantiated by someone else
        let instantiated: BTreeSet<(String, String)> =
            instantiations.iter().map(|i| i.child.key()).collect();

        // Top-level candidates: entities that are defined but not instantiated
        entities
            .into_iter()
            .filter(|(key, _)| !instantiated.contains(key))
            .map(|(_, name)| name)
            .collect()
    }

    /// Get the children of an entity (the entities it instantiates)
    pub fn children_of(&self, entity: &QualifiedName) -> Vec<&Instantiation> {
        let key = entity.key();
        self.instantiations
            .iter()
            .filter(|i| i.parent.key() == key)
            .collect()
    }

    /// Get the parents of an entity (the entities that instantiate it)
    pub fn parents_of(&self, entity: &QualifiedName) -> Vec<&Instantiation> {
        let key = entity.key();
        self.instantiations
            .iter()
            .filter(|i| i.child.key() == key)
            .collect()
    }

    /// get a list of ports and signals in an entity
    pub fn ports_and_signals(&self, entity: QualifiedName) -> Vec<Signal> {
        self.units
            .iter()
            .filter(|u| u.qualified_name() == entity)
            .fold(Vec::new(), |signals, unit| {
                [signals, unit.signals.clone()].concat()
            })
    }

    /// get a list of entities who instantiate a fpgacapzero core, from a list of names
    pub fn capz_instantiators(&self, cores_names: Vec<String>) -> Vec<&Instantiation> {
        self.instantiations
            .iter()
            .filter(|i| cores_names.contains(&i.child.name))
            .collect()
    }
}

/// Process a parsed design file: extract entities, architectures and instantiations
fn process_design_file(
    library: &str,
    file: &Path,
    design_file: &vhdl_lang::ast::DesignFile,
    units: &mut Vec<DesignUnit>,
    instantiations: &mut Vec<Instantiation>,
) {
    for (_, design_unit) in &design_file.design_units {
        match design_unit {
            AnyDesignUnit::Primary(primary) => {
                if let AnyPrimaryUnit::Entity(EntityDeclaration {
                    ident, port_clause, ..
                }) = primary
                {
                    units.push(DesignUnit {
                        library: library.to_string(),
                        file: file.to_path_buf(),
                        kind: DesignUnitKind::Entity,
                        name: ident.tree.item.to_string(),
                        entity: None,
                        signals: find_ports(port_clause),
                    });
                }
            }

            AnyDesignUnit::Secondary(secondary) => {
                if let AnySecondaryUnit::Architecture(ArchitectureBody {
                    ident,
                    entity_name,
                    statements,
                    decl,
                    ..
                }) = secondary
                {
                    let entity_name = entity_name.item.to_string();
                    units.push(DesignUnit {
                        library: library.to_string(),
                        file: file.to_path_buf(),
                        kind: DesignUnitKind::Architecture,
                        name: ident.tree.item.to_string(),
                        entity: Some(entity_name.clone()),
                        signals: find_signals(decl),
                    });
                    let mut walker = InstantiationWalker {
                        library,
                        entity: &entity_name,
                        instantiations,
                    };
                    walker.walk_statements(statements, "");
                }
            }
        }
    }
}

/// find the ports from a list
fn find_ports(port_clause: &Option<vhdl_lang::ast::InterfaceList>) -> Vec<Signal> {
    let mut ports: Vec<Signal> = Vec::new();

    if let Some(interfaces) = port_clause {
        for item in &interfaces.items {
            if let vhdl_lang::ast::InterfaceDeclaration::Object(object) = item
                && object.list_type == vhdl_lang::ast::InterfaceType::Port
                && let vhdl_lang::ast::ModeIndication::Simple(simple) = &object.mode
            {

                let mode = match &simple.mode {
                    Some(port_mode) => match port_mode.item {
                        vhdl_lang::ast::Mode::In => SignalMode::In,
                        vhdl_lang::ast::Mode::Out => SignalMode::Out,
                        vhdl_lang::ast::Mode::InOut => SignalMode::InOut,
                        vhdl_lang::ast::Mode::Buffer => SignalMode::Buffer,
                        _ => SignalMode::Unknown
                    },
                    _ => SignalMode::Unknown
                };
                let signal_type = simple.subtype_indication.to_string();
                for name in &object.idents {
                    ports.push(Signal {
                        name: name.to_string(),
                        signal_type: signal_type.clone(),
                        mode: mode.to_owned(),
                        size: 0,
                    });
                }
            }
        }
    }
    ports
}

fn find_signals(
    declarations: &Vec<vhdl_lang::ast::token_range::WithTokenSpan<vhdl_lang::ast::Declaration>>,
) -> Vec<Signal> {
    let mut signals: Vec<Signal> = Vec::new();

    for declaration in declarations {
        if let vhdl_lang::ast::Declaration::Object(object) = &declaration.item {
            let signal_type = object.subtype_indication.to_string();
            for name in &object.idents {
                signals.push(Signal {
                    name: name.to_string(),
                    signal_type: signal_type.clone(),
                    mode: SignalMode::Signal,
                    size: 0,
                });
            }
        }
    }
    signals
}

/// A walker that recursively traverses concurrent statements to find instantiations
struct InstantiationWalker<'a> {
    /// The library of the file being parsed
    library: &'a str,
    /// The name of the entity whose architecture is being walked
    entity: &'a str,
    /// Where to record the instantiations found
    instantiations: &'a mut Vec<Instantiation>,
}

impl<'a> InstantiationWalker<'a> {
    fn walk_statements(&mut self, statements: &[LabeledConcurrentStatement], label_prefix: &str) {
        for statement in statements {
            let label = if let Some(symbol) = &statement.label.tree {
                format!("{}{}", label_prefix, symbol.item)
            } else {
                label_prefix.to_owned()
            };
            self.walk_statement(&statement.statement.item, label);
        }
    }

    fn walk_statement(&mut self, statement: &ConcurrentStatement, label: String) {
        match statement {
            ConcurrentStatement::Instance(inst) => {
                self.walk_instantiation(&inst.unit, label);
            }
            ConcurrentStatement::Block(block) => {
                self.walk_statements(&block.statements, "");
            }
            ConcurrentStatement::ForGenerate(gen_stmt) => {
                self.walk_generate_body(&gen_stmt.body, &label);
            }
            ConcurrentStatement::IfGenerate(gen_stmt) => {
                for cond in &gen_stmt.conds.conditionals {
                    self.walk_generate_body(&cond.item, &label);
                }
                if let Some((else_body, _)) = &gen_stmt.conds.else_item {
                    self.walk_generate_body(else_body, &label);
                }
            }
            ConcurrentStatement::CaseGenerate(gen_stmt) => {
                for alt in &gen_stmt.sels.alternatives {
                    self.walk_generate_body(&alt.item, &label);
                }
            }
            _ => {} // Other statements do not contain instantiations
        }
    }

    fn walk_generate_body(&mut self, body: &GenerateBody, label: &str) {
        self.walk_statements(&body.statements, &format!("{}.", label));
    }

    fn walk_instantiation(&mut self, unit: &InstantiatedUnit, label: String) {
        let (child, kind) = match unit {
            InstantiatedUnit::Component(name) => {
                // A component is bound by default binding to an entity with
                // the same name, resolved in the current library (`work`)
                match extract_name_from_name(&name.item) {
                    Some(name) => (
                        QualifiedName::new(self.library, name),
                        InstantiationKind::Component,
                    ),
                    None => return,
                }
            }
            InstantiatedUnit::Entity(name, _) => {
                // A direct entity instantiation carries its own library
                // qualification, e.g. `entity lib2.my_module` or `entity work.foo`
                match extract_qualified_name(&name.item, self.library) {
                    Some(qualified) => (qualified, InstantiationKind::Entity),
                    None => return,
                }
            }
            InstantiatedUnit::Configuration(name) => {
                match extract_qualified_name(&name.item, self.library) {
                    Some(qualified) => (qualified, InstantiationKind::Configuration),
                    None => return,
                }
            }
        };
        let parent = QualifiedName::new(self.library, self.entity);
        self.instantiations.push(Instantiation {
            parent,
            label,
            child,
            kind,
        });
    }
}

/// Extract a simple identifier from a `Name`, if it is a plain
/// `Name::Designator` holding an identifier.
///
/// Returns `None` for names that are not simple identifiers (e.g. selected,
/// indexed or attribute names).
fn extract_name_from_name(name: &Name) -> Option<String> {
    match name {
        Name::Designator(designator) => extract_name_from_designator(&designator.item),
        _ => None,
    }
}

/// Extract the identifier of a `Designator`, if it is a simple identifier.
fn extract_name_from_designator(designator: &Designator) -> Option<String> {
    match designator {
        Designator::Identifier(symbol) => Some(symbol.name_utf8()),
        _ => None,
    }
}

/// Extract the library-qualified name of an instantiated unit from a `Name`.
///
/// - `entity lib2.my_module`  -> `QualifiedName::new("lib2", "my_module")`
/// - `entity work.my_module`  -> `QualifiedName::new(default_library, "my_module")`
///   (`work` refers to the library of the file being parsed)
/// - `entity my_module`       -> `QualifiedName::new(default_library, "my_module")`
///   (no library part, resolved to the current library)
///
/// Returns `None` for names that are not simple or selected identifiers.
fn extract_qualified_name(name: &Name, default_library: &str) -> Option<QualifiedName> {
    match name {
        Name::Selected(prefix, suffix) => {
            let library = match extract_name_from_name(&prefix.item) {
                Some(lib) if lib.eq_ignore_ascii_case("work") => default_library.to_string(),
                Some(lib) => lib,
                None => default_library.to_string(),
            };
            extract_name_from_designator(&suffix.item.item)
                .map(|name| QualifiedName::new(library, name))
        }
        _ => extract_name_from_name(name).map(|name| QualifiedName::new(default_library, name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vhdl::project::Project;
    use std::path::PathBuf;

    fn example_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example")
    }

    #[test]
    fn test_build_hierarchy() {
        let example_dir = example_dir();
        let toml_file = example_dir.join("vhdl_ls.toml");
        assert!(
            toml_file.exists(),
            "Test requires the example project at {}",
            toml_file.display()
        );
        let project = Project::from_vhdl_ls_toml(&toml_file).unwrap();
        let hierarchy = Hierarchy::build(&project).unwrap();
        assert!(hierarchy.units.len() > 100, "Expected many design units");
        assert!(
            !hierarchy.instantiations.is_empty(),
            "Expected instantiations to be found"
        );
        assert!(
            !hierarchy.top_level.is_empty(),
            "Expected top-level entities"
        );

        assert_eq!(
            hierarchy.top_level,
            vec![QualifiedName::new("neorv32", "neorv32_top")]
        );
    }

    #[test]
    fn test_multilib_hierarchy() {
        let example_dir = example_dir().join("multilib");
        let toml_file = example_dir.join("vhdl_ls.toml");
        assert!(
            toml_file.exists(),
            "Test requires the example project at {}",
            toml_file.display()
        );
        let project = Project::from_vhdl_ls_toml(&toml_file).unwrap();
        let hierarchy = Hierarchy::build(&project).unwrap();
        assert_eq!(hierarchy.units.len(), 6);
        assert_eq!(hierarchy.instantiations.len(), 2);

        // `lib2.my_module` is instantiated by `lib1.my_module`, so the only
        // top-level entity is `lib1.my_module` (and not `lib2.my_module`)
        assert_eq!(
            hierarchy.top_level,
            vec![QualifiedName::new("lib1", "my_module")]
        );

        // The two instantiations in `lib1.my_module`
        let children = hierarchy.children_of(&QualifiedName::new("lib1", "my_module"));
        let mut children: Vec<_> = children.iter().map(|i| (i.child.key(), i.kind)).collect();
        children.sort();
        assert_eq!(
            children,
            vec![
                (
                    ("lib1".into(), "a_submodule".into()),
                    InstantiationKind::Component
                ),
                (
                    ("lib2".into(), "my_module".into()),
                    InstantiationKind::Entity
                ),
            ]
        );

        // `lib2.my_module` is instantiated, `lib1.my_module` is not
        assert_eq!(
            hierarchy
                .parents_of(&QualifiedName::new("lib2", "my_module"))
                .len(),
            1
        );
        assert!(
            hierarchy
                .parents_of(&QualifiedName::new("lib1", "my_module"))
                .is_empty()
        );
    }

    #[test]
    fn test_qualified_name_case_insensitive() {
        assert_eq!(
            QualifiedName::new("lib1", "my_module"),
            QualifiedName::new("LIB1", "MY_MODULE")
        );
        assert_ne!(
            QualifiedName::new("lib1", "my_module"),
            QualifiedName::new("lib2", "my_module")
        );
        assert_eq!(
            QualifiedName::new("lib1", "my_module").to_string(),
            "lib1.my_module"
        );
    }
}
