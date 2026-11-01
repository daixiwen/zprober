//! Instantiation hierarchy built from a VHDL project
//!
//! The [vhdl_lang] crate provides parsing and analysis of individual files,
//! but it does not build an instantiation tree. This module walks the AST of
//! every design unit in a project, records which entity instantiates which,
//! and identifies the top-level entities.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use vhdl_lang::ast::{
    AnyDesignUnit, AnyPrimaryUnit, AnySecondaryUnit, ArchitectureBody, ConcurrentStatement,
    Designator, EntityDeclaration, GenerateBody, InstantiatedUnit, LabeledConcurrentStatement,
    Name,
};
use vhdl_lang::{NullDiagnostics, VHDLParser};

use crate::vhdl::project::Project;

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
    pub parent: String,
    /// The entity that is being instantiated (the child in the tree)
    pub child: String,
    /// The source file of the parent
    pub parent_file: PathBuf,
    /// The kind of instantiation (direct entity, component or configuration)
    pub kind: InstantiationKind,
}

/// The kind of instantiation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub top_level: Vec<String>,
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
    /// project, but no other entity instantiates it.
    fn compute_top_level(units: &[DesignUnit], instantiations: &[Instantiation]) -> Vec<String> {
        // All entity names defined in the project
        let entities: BTreeSet<String> = units
            .iter()
            .filter(|u| u.kind == DesignUnitKind::Entity)
            .map(|u| u.name.to_lowercase())
            .collect();

        // Names of entities instantiated by someone else
        let instantiated: BTreeSet<String> = instantiations
            .iter()
            .map(|i| i.child.to_lowercase())
            .collect();

        // Top-level candidates: entities that are defined but not instantiated
        entities
            .iter()
            .filter(|name| !instantiated.contains(*name))
            .cloned()
            .collect()
    }

    /// Get the children of an entity (the entities it instantiates)
    pub fn children_of(&self, entity: &str) -> Vec<&Instantiation> {
        let lower = entity.to_lowercase();
        self.instantiations
            .iter()
            .filter(|i| i.parent.to_lowercase() == lower)
            .collect()
    }

    /// Get the parents of an entity (the entities that instantiate it)
    pub fn parents_of(&self, entity: &str) -> Vec<&Instantiation> {
        let lower = entity.to_lowercase();
        self.instantiations
            .iter()
            .filter(|i| i.child.to_lowercase() == lower)
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
            AnyDesignUnit::Primary(primary) => match primary {
                AnyPrimaryUnit::Entity(EntityDeclaration { ident, .. }) => {
                    units.push(DesignUnit {
                        library: library.to_string(),
                        file: file.to_path_buf(),
                        kind: DesignUnitKind::Entity,
                        name: ident.tree.item.to_string(),
                        entity: None,
                    });
                }
                _ => {} // Configurations, packages, contexts: not relevant for the tree
            },
            AnyDesignUnit::Secondary(secondary) => match secondary {
                AnySecondaryUnit::Architecture(ArchitectureBody {
                    ident,
                    entity_name,
                    statements,
                    ..
                }) => {
                    let entity_name = entity_name.item.to_string();
                    units.push(DesignUnit {
                        library: library.to_string(),
                        file: file.to_path_buf(),
                        kind: DesignUnitKind::Architecture,
                        name: ident.tree.item.to_string(),
                        entity: Some(entity_name.clone()),
                    });
                    let mut walker = InstantiationWalker {
                        file,
                        entity: &entity_name,
                        instantiations,
                    };
                    walker.walk_statements(statements);
                }
                _ => {} // Package bodies: not relevant for the tree
            },
        }
    }
}

/// A walker that recursively traverses concurrent statements to find instantiations
struct InstantiationWalker<'a> {
    file: &'a Path,
    entity: &'a str,
    instantiations: &'a mut Vec<Instantiation>,
}

impl<'a> InstantiationWalker<'a> {
    fn walk_statements(&mut self, statements: &[LabeledConcurrentStatement]) {
        for statement in statements {
            self.walk_statement(&statement.statement.item);
        }
    }

    fn walk_statement(&mut self, statement: &ConcurrentStatement) {
        match statement {
            ConcurrentStatement::Instance(inst) => {
                self.walk_instantiation(&inst.unit);
            }
            ConcurrentStatement::Block(block) => {
                self.walk_statements(&block.statements);
            }
            ConcurrentStatement::ForGenerate(gen_stmt) => {
                self.walk_generate_body(&gen_stmt.body);
            }
            ConcurrentStatement::IfGenerate(gen_stmt) => {
                for cond in &gen_stmt.conds.conditionals {
                    self.walk_generate_body(&cond.item);
                }
                if let Some((else_body, _)) = &gen_stmt.conds.else_item {
                    self.walk_generate_body(else_body);
                }
            }
            ConcurrentStatement::CaseGenerate(gen_stmt) => {
                for alt in &gen_stmt.sels.alternatives {
                    self.walk_generate_body(&alt.item);
                }
            }
            _ => {} // Other statements do not contain instantiations
        }
    }

    fn walk_generate_body(&mut self, body: &GenerateBody) {
        self.walk_statements(&body.statements);
    }

    fn walk_instantiation(&mut self, unit: &InstantiatedUnit) {
        match unit {
            InstantiatedUnit::Component(name) => {
                if let Some(name) = extract_name_from_name(&name.item) {
                    self.instantiations.push(Instantiation {
                        parent: self.entity.to_string(),
                        child: name,
                        parent_file: self.file.to_path_buf(),
                        kind: InstantiationKind::Component,
                    });
                }
            }
            InstantiatedUnit::Entity(name, _) => {
                if let Some(name) = extract_name_from_name(&name.item) {
                    self.instantiations.push(Instantiation {
                        parent: self.entity.to_string(),
                        child: name,
                        parent_file: self.file.to_path_buf(),
                        kind: InstantiationKind::Entity,
                    });
                }
            }
            InstantiatedUnit::Configuration(name) => {
                if let Some(name) = extract_name_from_name(&name.item) {
                    self.instantiations.push(Instantiation {
                        parent: self.entity.to_string(),
                        child: name,
                        parent_file: self.file.to_path_buf(),
                        kind: InstantiationKind::Configuration,
                    });
                }
            }
        }
    }
}

/// Extract the string representation of a `Name` from the AST.
///
/// Returns `None` for names that are not simple identifiers (e.g. indexed
/// names or attribute names).
fn extract_name_from_name(name: &Name) -> Option<String> {
    match name {
        Name::Designator(designator) => {
            match &designator.item {
                Designator::Identifier(symbol) => Some(symbol.name_utf8()),
                _ => None,
            }
        }
        Name::Selected(prefix, suffix) => {
            // For a selected name like `lib.entity_name`, the suffix is the entity name
            let _ = prefix;
            match &suffix.item.item {
                Designator::Identifier(symbol) => Some(symbol.name_utf8()),
                _ => None,
            }
        }
        _ => None,
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
        assert!(!hierarchy.top_level.is_empty(), "Expected top-level entities");

        assert_eq!(hierarchy.top_level, vec!["neorv32_top"]);
    }
}

