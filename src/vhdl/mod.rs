/// VHDL project management and hierarchy analysis
pub mod project;

/// Injection of code blocks into VHDL files
pub mod injection;

/// Instantiation hierarchy built from a VHDL project
pub mod hierarchy;

pub use hierarchy::{
    DesignUnit, DesignUnitKind, Hierarchy, Instantiation, QualifiedName, Signal, SignalMode,
};
pub use injection::{InjectionPoints, inject, remove_injections};
pub use project::{Library, Project};
