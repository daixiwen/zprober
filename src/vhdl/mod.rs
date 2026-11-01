/// VHDL project management and hierarchy analysis
pub mod project;

/// Instantiation hierarchy built from a VHDL project
pub mod hierarchy;

pub use hierarchy::{DesignUnit, DesignUnitKind, Hierarchy, Instantiation, QualifiedName};
pub use project::{Library, Project};
