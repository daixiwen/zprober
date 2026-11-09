//! Resolution of the types of signals and ports.
//!
//! The [vhdl_lang] parser only provides the syntax of a type mark: a port
//! declared as `t_bus` may reference a subtype declared in a package, which
//! itself uses another package, and so on. This module builds a small model
//! of every design unit of a project (its type declarations, its context
//! clause and its ports/signals) and then resolves the type of each port and
//! signal recursively until a *terminal* type is reached:
//!
//! * a predefined / IEEE standard type (e.g. `std_logic`, `integer`), or a
//!   user-defined scalar (`range`/`physical`) type: terminal, only the name
//!   is kept,
//! * an enumeration: the list of its elements is kept,
//! * a record: the fields and their (recursively resolved) types are kept,
//! * an array or a subtype: resolved to what it points at (array ranges are
//!   ignored, so `std_logic_vector(7 downto 0)` is just an array of
//!   `std_logic`).
//!
//! Anything that cannot be resolved (declaration not found, unsupported
//! construct, cyclic definition) resolves to [`ResolvedType::Unknown`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::Path;

use vhdl_lang::ast::token_range::WithTokenSpan;
use vhdl_lang::ast::{
    AnyDesignUnit, AnyPrimaryUnit, AnySecondaryUnit, ContextItem, Declaration, DesignFile,
    Designator, EntityDeclaration, EnumerationLiteral, InterfaceDeclaration, InterfaceList,
    InterfaceType, ModeIndication, Name, PackageDeclaration, PackageInstantiation,
    SubtypeIndication, TypeDeclaration, TypeDefinition,
};

use crate::vhdl::hierarchy::DesignUnitKind;

/// The resolved type of a port or a signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedType {
    /// A predefined / IEEE standard type (e.g. `std_logic`, `integer`), or a
    /// user-defined scalar (`range`/`physical`) type: terminal, only the
    /// name is kept.
    Standard { name: String },
    /// An array; ranges are not kept and the element type is resolved
    /// recursively.
    Array { element: Box<ResolvedType> },
    /// An enumeration with the list of its elements (identifiers, or
    /// character literals such as `'x'`).
    Enum { name: String, elements: Vec<String> },
    /// A record with its fields; the type of each field is resolved
    /// recursively.
    Record {
        name: String,
        fields: Vec<RecordField>,
    },
    /// The type could not be resolved (unknown declaration, unsupported
    /// construct or cyclic definition). `raw` is the type mark as written.
    Unknown { raw: String },
}

/// A field of a [`ResolvedType::Record`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordField {
    pub name: String,
    pub resolved_type: ResolvedType,
}

impl fmt::Display for ResolvedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard { name } => write!(f, "{name}"),
            Self::Array { element } => write!(f, "array of {element}"),
            Self::Enum { name, elements } => write!(f, "{name} ({})", elements.join(", ")),
            Self::Record { name, fields } => {
                write!(f, "{name} record(")?;
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", field.name, field.resolved_type)?;
                }
                write!(f, ")")
            }
            Self::Unknown { raw } => write!(f, "unknown ({raw})"),
        }
    }
}

/// The kind of design unit a scope belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum UnitKind {
    Entity,
    Architecture,
    Package,
    /// `package foo is new bar ...`: an alias for the target package.
    PackageInstance,
    /// A `context ... is ... end context` declaration.
    Context,
}

/// Identifies a design unit: `(library, kind, name)`, all lowercase, since
/// VHDL identifiers are case-insensitive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScopeKey {
    library: String,
    kind: UnitKind,
    unit: String,
}

impl ScopeKey {
    fn new(library: &str, kind: UnitKind, unit: &str) -> Self {
        Self {
            library: library.to_lowercase(),
            kind,
            unit: unit.to_lowercase(),
        }
    }
}

/// A port or signal recorded for type resolution.
struct ObjectInfo {
    subtype: SubtypeIndication,
}

/// The declarations and context of one design unit.
#[derive(Default)]
struct UnitModel {
    /// Type declarations in the declarative part, keyed by lowercase name.
    decls: BTreeMap<String, TypeDeclaration>,
    /// For architectures: the declaring entity.
    parent: Option<ScopeKey>,
    /// For package instantiations: the instantiated package.
    alias: Option<ScopeKey>,
    /// The context clause of the design unit.
    context: Vec<ContextItem>,
    /// Ports and signals, one entry per identifier, in extraction order.
    objects: Vec<ObjectInfo>,
    /// Names made visible by the context clause (computed by
    /// [`TypeResolver::finalize`]), keyed by lowercase name.
    visible: BTreeMap<String, ScopeKey>,
}

/// Resolves the types of the ports and signals of a project.
///
/// Usage: call [`TypeResolver::add_design_file`] for every parsed design
/// file, then [`TypeResolver::finalize`], then
/// [`TypeResolver::resolve_unit`] for each design unit.
pub struct TypeResolver {
    models: HashMap<ScopeKey, UnitModel>,
    /// Known library names (the project's libraries plus the standard ones).
    libraries: HashSet<String>,
}

impl Default for TypeResolver {
    fn default() -> Self {
        Self::new()
    }
}
impl TypeResolver {
    pub fn new() -> Self {
        let mut libraries = HashSet::new();
        libraries.insert("std".to_string());
        libraries.insert("ieee".to_string());
        Self {
            models: HashMap::new(),
            libraries,
        }
    }

    /// Collect the type declarations, context clause and ports/signals of
    /// every design unit of a parsed design file.
    pub fn add_design_file(&mut self, library: &str, _file: &Path, design_file: &DesignFile) {
        self.libraries.insert(library.to_lowercase());
        for (_, design_unit) in &design_file.design_units {
            match design_unit {
                AnyDesignUnit::Primary(AnyPrimaryUnit::Entity(entity)) => {
                    let EntityDeclaration {
                        context_clause,
                        ident,
                        port_clause,
                        decl,
                        ..
                    } = entity;
                    let scope = ScopeKey::new(library, UnitKind::Entity, &name_of(ident));
                    let model = self.model(scope);
                    model.context = context_clause.clone();
                    collect_type_decls(decl, &mut model.decls);
                    collect_port_objects(port_clause, &mut model.objects);
                }

                AnyDesignUnit::Secondary(AnySecondaryUnit::Architecture(arch)) => {
                    let scope =
                        ScopeKey::new(library, UnitKind::Architecture, &name_of(&arch.ident));
                    let model = self.model(scope);
                    model.parent = Some(ScopeKey::new(
                        library,
                        UnitKind::Entity,
                        &name_of(&arch.entity_name.item),
                    ));
                    model.context = arch.context_clause.clone();
                    collect_type_decls(&arch.decl, &mut model.decls);
                    collect_signal_objects(&arch.decl, &mut model.objects);
                }

                AnyDesignUnit::Primary(AnyPrimaryUnit::Package(package)) => {
                    let PackageDeclaration {
                        context_clause,
                        ident,
                        decl,
                        ..
                    } = package;
                    let scope = ScopeKey::new(library, UnitKind::Package, &name_of(ident));
                    let model = self.model(scope);
                    model.context = context_clause.clone();
                    collect_type_decls(decl, &mut model.decls);
                }

                AnyDesignUnit::Primary(AnyPrimaryUnit::PackageInstance(instantiation)) => {
                    let PackageInstantiation {
                        ident,
                        package_name,
                        ..
                    } = instantiation;
                    let scope = ScopeKey::new(library, UnitKind::PackageInstance, &name_of(ident));
                    let target = self.package_scope(&package_name.item, library);
                    let model = self.model(scope);
                    model.alias = target;
                }

                AnyDesignUnit::Primary(AnyPrimaryUnit::Context(context)) => {
                    let scope = ScopeKey::new(library, UnitKind::Context, &name_of(&context.ident));
                    let model = self.model(scope);
                    model.context = context.items.clone();
                }

                // Package bodies (their declarations are private) and
                // configuration declarations are not needed for type
                // resolution.
                _ => {}
            }
        }
    }

    /// Compute the visibility maps of every collected design unit. Must be
    /// called once after all design files were added, before any resolution.
    pub fn finalize(&mut self) {
        let scopes: Vec<ScopeKey> = self.models.keys().cloned().collect();
        for scope in scopes {
            let mut visible = BTreeMap::new();
            let mut stack = Vec::new();
            self.expand_visible(scope.clone(), &mut stack, &mut visible);
            if let Some(model) = self.models.get_mut(&scope) {
                model.visible = visible;
            }
        }
    }

    /// Resolve the type of every recorded port/signal of a design unit.
    ///
    /// The returned vector is aligned with the `Signal`s that
    /// `Hierarchy::build` extracts for the same design unit.
    pub fn resolve_unit(
        &self,
        library: &str,
        kind: DesignUnitKind,
        name: &str,
    ) -> Vec<ResolvedType> {
        let kind = match kind {
            DesignUnitKind::Entity => UnitKind::Entity,
            DesignUnitKind::Architecture => UnitKind::Architecture,
        };
        let scope = ScopeKey::new(library, kind, name);
        let Some(model) = self.models.get(&scope) else {
            return Vec::new();
        };
        let mut result = Vec::with_capacity(model.objects.len());
        for object in &model.objects {
            let mut visited = HashSet::new();
            result.push(self.resolve_subtype(&object.subtype, &scope, &mut visited));
        }
        result
    }

    fn model(&mut self, scope: ScopeKey) -> &mut UnitModel {
        self.models.entry(scope).or_default()
    }

    /// Expand the context clause of `scope` into `visible`; `stack` guards
    /// against recursive context declarations.
    fn expand_visible(
        &self,
        scope: ScopeKey,
        stack: &mut Vec<ScopeKey>,
        visible: &mut BTreeMap<String, ScopeKey>,
    ) {
        if stack.contains(&scope) {
            return;
        }
        stack.push(scope.clone());
        if let Some(model) = self.models.get(&scope) {
            for item in &model.context {
                match item {
                    ContextItem::Use(use_clause) => {
                        for name in &use_clause.name_list {
                            if let Some(parts) = name_parts(&name.item) {
                                self.apply_use(&parts, &scope, visible);
                            }
                        }
                    }
                    ContextItem::Library(_) => {
                        // Library clauses only enable `lib.unit.name`
                        // selected names, which are resolved on demand.
                    }
                    ContextItem::Context(context_ref) => {
                        for name in &context_ref.name_list {
                            if let Some(parts) = name_parts(&name.item)
                                && parts.len() == 2
                            {
                                let library = resolve_library_name(&parts[0], &scope.library);
                                self.expand_visible(
                                    ScopeKey::new(&library, UnitKind::Context, &parts[1]),
                                    stack,
                                    visible,
                                );
                            }
                        }
                    }
                }
            }
        }
        stack.pop();
    }

    /// Apply a single use clause to the visibility map.
    ///
    /// * `lib.pkg.all` (or `work.pkg.all`): all declarations of the package
    ///   become visible,
    /// * `lib.pkg.name`: the named item becomes visible,
    /// * `lib.pkg`: the package name itself becomes visible (no types),
    /// * `pkg.name`: a package directly visible in the current library.
    fn apply_use(
        &self,
        parts: &[String],
        scope: &ScopeKey,
        visible: &mut BTreeMap<String, ScopeKey>,
    ) {
        match parts {
            [library, package, item] => {
                let library = resolve_library_name(library, &scope.library);
                let target = self.follow_alias(ScopeKey::new(&library, UnitKind::Package, package));
                if item.eq_ignore_ascii_case("all") {
                    if let Some(model) = self.models.get(&target) {
                        for name in model.decls.keys() {
                            visible.insert(name.clone(), target.clone());
                        }
                    }
                } else {
                    visible.insert(item.to_lowercase(), target);
                }
            }
            [first, second]
                // `lib.pkg` (package name visibility only) or `pkg.name`
                // (a package of the current library made visible by a
                // previous library clause).
                if !self.is_library(first) => {
                    let target =
                        self.follow_alias(ScopeKey::new(&scope.library, UnitKind::Package, first));
                    visible.insert(second.to_lowercase(), target);
                }
            _ => {}
        }
    }

    /// Follow package instantiation aliases (bounded, to survive aliases of
    /// aliases and (erroneous) alias cycles).
    fn follow_alias(&self, mut scope: ScopeKey) -> ScopeKey {
        for _ in 0..16 {
            let Some(alias) = self
                .models
                .get(&scope)
                .and_then(|model| model.alias.clone())
            else {
                return scope;
            };
            scope = alias;
        }
        scope
    }

    fn is_library(&self, name: &str) -> bool {
        self.libraries.contains(&name.to_lowercase())
    }

    /// Whether the given ieee package is used by `scope` (or the entity its
    /// architecture belongs to).
    fn uses_package(&self, scope: &ScopeKey, library: &str, unit: &str) -> bool {
        let target = ScopeKey::new(library, UnitKind::Package, unit);
        let mut scopes = vec![scope.clone()];
        if let Some(parent) = self.models.get(scope).and_then(|m| m.parent.clone()) {
            scopes.push(parent);
        }
        scopes.iter().any(|s| {
            self.models
                .get(s)
                .is_some_and(|model| model.visible.values().any(|v| *v == target))
        })
    }

    /// Find the target package of a package name (e.g. from a package
    /// instantiation declaration), handling the `work` alias.
    fn package_scope(&self, name: &Name, current_library: &str) -> Option<ScopeKey> {
        let parts = name_parts(name)?;
        match parts.len() {
            1 => Some(ScopeKey::new(current_library, UnitKind::Package, &parts[0])),
            2 => {
                let library = resolve_library_name(&parts[0], current_library);
                Some(ScopeKey::new(&library, UnitKind::Package, &parts[1]))
            }
            _ => None,
        }
    }

    // -- resolution ---------------------------------------------------------

    /// Resolve a subtype indication in the context of `scope`.
    fn resolve_subtype(
        &self,
        subtype: &SubtypeIndication,
        scope: &ScopeKey,
        visited: &mut HashSet<(ScopeKey, String)>,
    ) -> ResolvedType {
        let raw = subtype.type_mark.item.to_string();
        let Some(parts) = name_parts(&subtype.type_mark.item) else {
            return ResolvedType::Unknown { raw };
        };

        // A declaration of the project (in the local scope, an enclosing
        // scope or a used package)?
        if let Some((decl_scope, decl_name)) = self.lookup(scope, &parts) {
            let key = (decl_scope.clone(), decl_name.clone());
            if !visited.insert(key) {
                // Cyclic type definition (e.g. two access types pointing at
                // each other): stop the recursion here.
                return ResolvedType::Unknown { raw };
            }
            let decl = self
                .models
                .get(&decl_scope)
                .and_then(|model| model.decls.get(&decl_name));
            let result = match decl {
                Some(decl) => self.resolve_decl(decl, &decl_scope, visited),
                None => ResolvedType::Unknown { raw },
            };
            visited.remove(&(decl_scope, decl_name));
            return result;
        }

        // Predefined types of std.standard / ieee packages (looked up last,
        // so user declarations take precedence).
        if parts.len() == 1 {
            if is_predefined_scalar(&parts[0]) {
                return ResolvedType::Standard { name: raw.clone() };
            }
            if let Some(element) = self.predefined_array_element(&parts[0], scope) {
                return ResolvedType::Array {
                    element: Box::new(ResolvedType::Standard {
                        name: element.to_string(),
                    }),
                };
            }
        }

        ResolvedType::Unknown { raw }
    }

    /// Find the declaration of a type mark in `scope`, following the
    /// visibility rules (with the usual simplifications):
    ///
    /// 1. the declarations of the unit itself,
    /// 2. the declarations of the declaring entity (for architectures),
    /// 3. the names made visible by the unit's context clause,
    /// 4. the names made visible by the declaring entity's context clause,
    ///
    /// For selected names (`pkg.type`, `lib.pkg.type`), the declaration is
    /// looked up directly in the designated package.
    fn lookup(&self, scope: &ScopeKey, parts: &[String]) -> Option<(ScopeKey, String)> {
        let type_name = parts.last()?.to_lowercase();
        match parts.len() {
            1 => {
                let model = self.models.get(scope)?;
                if model.decls.contains_key(&type_name) {
                    return Some((scope.clone(), type_name));
                }
                if let Some(parent) = &model.parent
                    && self
                        .models
                        .get(parent)
                        .is_some_and(|m| m.decls.contains_key(&type_name))
                {
                    return Some((parent.clone(), type_name));
                }
                if let Some(target) = model.visible.get(&type_name) {
                    return Some((target.clone(), type_name));
                }
                if let Some(parent) = &model.parent
                    && let Some(target) = self
                        .models
                        .get(parent)
                        .and_then(|m| m.visible.get(&type_name))
                {
                    return Some((target.clone(), type_name));
                }
                None
            }

            2 => {
                // `pkg.type`: a package of the current library
                let package =
                    self.follow_alias(ScopeKey::new(&scope.library, UnitKind::Package, &parts[0]));
                if self
                    .models
                    .get(&package)
                    .is_some_and(|m| m.decls.contains_key(&type_name))
                {
                    Some((package, type_name))
                } else {
                    None
                }
            }
            3 => {
                // `lib.pkg.type` (or `work.pkg.type`)
                let library = resolve_library_name(&parts[0], &scope.library);
                if library == "std" || library == "ieee" {
                    // Standard packages: only the predefined table is known.
                    return None;
                }
                let package =
                    self.follow_alias(ScopeKey::new(&library, UnitKind::Package, &parts[1]));
                if self
                    .models
                    .get(&package)
                    .is_some_and(|m| m.decls.contains_key(&type_name))
                {
                    Some((package, type_name))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Resolve a type declaration into a terminal type.
    fn resolve_decl(
        &self,
        decl: &TypeDeclaration,
        scope: &ScopeKey,
        visited: &mut HashSet<(ScopeKey, String)>,
    ) -> ResolvedType {
        let name = name_of(&decl.ident);
        match &decl.def {
            TypeDefinition::Enumeration(literals) => {
                let elements = literals
                    .iter()
                    .map(|literal| match &literal.tree.item {
                        EnumerationLiteral::Identifier(symbol) => symbol.name_utf8(),
                        EnumerationLiteral::Character(c) => {
                            format!("'{}'", *c as char)
                        }
                    })
                    .collect();
                ResolvedType::Enum { name, elements }
            }
            // Scalar types are terminal (their ranges are not needed).
            TypeDefinition::Numeric(_) | TypeDefinition::Physical(_) => {
                ResolvedType::Standard { name }
            }
            TypeDefinition::Array(_, _, element) => ResolvedType::Array {
                element: Box::new(self.resolve_subtype(element, scope, visited)),
            },
            TypeDefinition::Record(elements) => {
                let mut fields = Vec::new();
                for element in elements {
                    // One element declaration can declare several fields
                    // (`a, b : std_logic`).
                    for ident in &element.idents {
                        fields.push(RecordField {
                            name: name_of(ident),
                            resolved_type: self.resolve_subtype(&element.subtype, scope, visited),
                        });
                    }
                }
                ResolvedType::Record { name, fields }
            }
            // A subtype resolves to what it points at (constraints are
            // ignored).
            TypeDefinition::Subtype(subtype) => self.resolve_subtype(subtype, scope, visited),
            // An access type resolves to its designated type; cycles are
            // caught by `visited`.
            TypeDefinition::Access(subtype) => self.resolve_subtype(subtype, scope, visited),
            // File and protected types are not needed for signal typing.
            _ => ResolvedType::Unknown { raw: name },
        }
    }

    /// The element type of a predefined array type, if `name` is one.
    fn predefined_array_element(&self, name: &str, scope: &ScopeKey) -> Option<&'static str> {
        let element = match name {
            "bit_vector" => "bit",
            "string" => "character",
            "boolean_vector" => "boolean",
            "integer_vector" => "integer",
            "real_vector" => "real",
            "time_vector" => "time",
            "std_logic_vector" => "std_logic",
            "std_ulogic_vector" => "std_ulogic",
            // numeric_std defines `unsigned`/`signed` as arrays of
            // std_logic; numeric_bit as arrays of bit.
            "unsigned" | "signed" => {
                if self.uses_package(scope, "ieee", "numeric_bit")
                    && !self.uses_package(scope, "ieee", "numeric_std")
                {
                    "bit"
                } else {
                    "std_logic"
                }
            }
            _ => return None,
        };
        Some(element)
    }
}

/// Predefined scalar types of `std.standard` and `ieee.std_logic_1164`.
///
/// Note: known IEEE names are resolved even if their package is not used;
/// declaring a port of an unused type is a design error anyway.
const PREDEFINED_SCALARS: &[&str] = &[
    "boolean",
    "bit",
    "character",
    "severity_level",
    "integer",
    "natural",
    "positive",
    "real",
    "time",
    "delay_length",
    "universal_integer",
    "universal_real",
    "std_logic",
    "std_ulogic",
];

fn is_predefined_scalar(name: &str) -> bool {
    PREDEFINED_SCALARS.contains(&name)
}

// -- extraction helpers -----------------------------------------------------

fn name_of<T: std::fmt::Display>(item: &T) -> String {
    item.to_string()
}

/// Collect the type declarations of a declarative part.
fn collect_type_decls(
    declarations: &[WithTokenSpan<Declaration>],
    out: &mut BTreeMap<String, TypeDeclaration>,
) {
    for declaration in declarations {
        if let Declaration::Type(type_decl) = &declaration.item {
            out.insert(name_of(&type_decl.ident).to_lowercase(), type_decl.clone());
        }
    }
}

/// Record the ports of a port clause, one entry per identifier (matching the
/// extraction order of `hierarchy::find_ports`).
fn collect_port_objects(port_clause: &Option<InterfaceList>, out: &mut Vec<ObjectInfo>) {
    if let Some(interfaces) = port_clause {
        for item in &interfaces.items {
            if let InterfaceDeclaration::Object(object) = item
                && object.list_type == InterfaceType::Port
                && let ModeIndication::Simple(simple) = &object.mode
            {
                for _ in &object.idents {
                    out.push(ObjectInfo {
                        subtype: simple.subtype_indication.clone(),
                    });
                }
            }
        }
    }
}

/// Record the objects (signals, constants, ...) of a declarative part, one
/// entry per identifier (matching the extraction order of
/// `hierarchy::find_signals`).
fn collect_signal_objects(declarations: &[WithTokenSpan<Declaration>], out: &mut Vec<ObjectInfo>) {
    for declaration in declarations {
        if let Declaration::Object(object) = &declaration.item {
            for _ in &object.idents {
                out.push(ObjectInfo {
                    subtype: object.subtype_indication.clone(),
                });
            }
        }
    }
}

/// Flatten a (selected) name into its identifier parts, e.g.
/// `work.pkg.all` -> `["work", "pkg", "all"]`.
///
/// Returns `None` for names that are not made of simple identifiers
/// (attributes, slices, calls, ...).
fn name_parts(name: &Name) -> Option<Vec<String>> {
    match name {
        Name::Designator(designator) => designator_name(&designator.item).map(|n| vec![n]),
        Name::Selected(prefix, suffix) => {
            let mut parts = name_parts(&prefix.item)?;
            parts.push(designator_name(&suffix.item.item)?);
            Some(parts)
        }
        Name::SelectedAll(prefix) => {
            let mut parts = name_parts(&prefix.item)?;
            parts.push("all".to_string());
            Some(parts)
        }
        _ => None,
    }
}

fn designator_name(designator: &Designator) -> Option<String> {
    match designator {
        Designator::Identifier(symbol) => Some(symbol.name_utf8()),
        _ => None,
    }
}

/// Resolve a library name of a selected name: `work` refers to the library
/// of the design unit being resolved.
fn resolve_library_name(name: &str, current: &str) -> String {
    if name.eq_ignore_ascii_case("work") {
        current.to_string()
    } else {
        name.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use vhdl_lang::{NullDiagnostics, Source, VHDLParser, VHDLStandard};

    fn test_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("example")
            .join("types_test")
    }

    fn parse(vhdl_file_name: &str) -> DesignFile {
        let parser = VHDLParser::new(VHDLStandard::VHDL2008);
        let source = Source::from_latin1_file(&test_dir().join(vhdl_file_name))
            .expect("error parsing file {vhdl_file_name}");
        parser.parse_design_source(&source, &mut NullDiagnostics)
    }

    fn build(vhdl_file_name: &str, library: &str) -> TypeResolver {
        let design_file = parse(vhdl_file_name);
        let mut resolver = TypeResolver::new();
        resolver.add_design_file(library, Path::new("inline_test.vhd"), &design_file);
        resolver.finalize();
        resolver
    }

    fn std(name: &str) -> ResolvedType {
        ResolvedType::Standard {
            name: name.to_string(),
        }
    }

    fn array_of(element: ResolvedType) -> ResolvedType {
        ResolvedType::Array {
            element: Box::new(element),
        }
    }

    #[test]
    fn test_predefined_types() {
        let resolver = build("test_predefined_types.vhd", "work");
        let types = resolver.resolve_unit("work", DesignUnitKind::Entity, "top");
        assert_eq!(types[0], std("std_logic"));
        assert_eq!(types[1], array_of(std("std_logic")));
        assert_eq!(types[2], std("integer"));
        assert_eq!(types[3], array_of(std("std_logic")));
        assert_eq!(types[4], std("boolean"));
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_entity_local_types() {
        let resolver = build("test_entity_local_types.vhd", "work");
        let types = resolver.resolve_unit("work", DesignUnitKind::Entity, "top");
        // The ports are declared in this order: rec (t_rec), arr (t_arr),
        // state (t_state).
        assert_eq!(
            types[0],
            ResolvedType::Record {
                name: "t_rec".to_string(),
                fields: vec![
                    RecordField {
                        name: "a".to_string(),
                        resolved_type: std("std_logic"),
                    },
                    RecordField {
                        name: "b".to_string(),
                        // the enum t_state, resolved as types[2] below
                        resolved_type: types[2].clone(),
                    },
                ]
            }
        );
        // t_arr is an array of the record t_rec
        assert_eq!(types[1], array_of(types[0].clone()));
        assert_eq!(
            types[2],
            ResolvedType::Enum {
                name: "t_state".to_string(),
                elements: vec![
                    "idle".to_string(),
                    "running".to_string(),
                    "done".to_string(),
                    "'x'".to_string()
                ]
            }
        );
    }

    #[test]
    fn test_package_types_and_use_clauses() {
        let resolver = build("test_package_types_and_use_clauses.vhd", "work");
        let types = resolver.resolve_unit("work", DesignUnitKind::Entity, "top");
        let expected_record = ResolvedType::Record {
            name: "t_rec".to_string(),
            fields: vec![
                RecordField {
                    name: "state".to_string(),
                    resolved_type: ResolvedType::Enum {
                        name: "t_state".to_string(),
                        elements: vec!["s0".to_string(), "s1".to_string(), "s2".to_string()],
                    },
                },
                RecordField {
                    name: "data".to_string(),
                    resolved_type: array_of(std("std_logic")),
                },
            ],
        };
        // `t_sub` is a subtype of `t_rec`
        assert_eq!(types[0], expected_record);
        // selected name `pkg_b.t_rec`
        assert_eq!(types[1], expected_record);
        // fully qualified name `work.pkg_a.t_state`
        assert_eq!(
            types[2],
            ResolvedType::Enum {
                name: "t_state".to_string(),
                elements: vec!["s0".to_string(), "s1".to_string(), "s2".to_string()],
            }
        );
    }

    #[test]
    fn test_architecture_signals() {
        let resolver = build("test_architecture_signals.vhd", "work");
        let types = resolver.resolve_unit("work", DesignUnitKind::Architecture, "rtl");
        // `s1` uses a local record whose field is an array of the type
        // declared in the entity
        assert_eq!(
            types[0],
            ResolvedType::Record {
                name: "t_local".to_string(),
                fields: vec![RecordField {
                    name: "f".to_string(),
                    resolved_type: array_of(std("std_logic")),
                }],
            }
        );
        assert_eq!(types[1], array_of(std("std_logic")));
    }

    #[test]
    fn test_cross_library_selected_name() {
        let mut resolver = TypeResolver::new();
        resolver.add_design_file(
            "l1",
            Path::new("pkg.vhd"),
            &parse("test_cross_library_selected_name_l1.vhd"),
        );
        resolver.add_design_file(
            "l2",
            Path::new("top.vhd"),
            &parse("test_cross_library_selected_name_l2.vhd"),
        );
        resolver.finalize();
        let types = resolver.resolve_unit("l2", DesignUnitKind::Entity, "top");
        assert_eq!(
            types[0],
            ResolvedType::Enum {
                name: "t_color".to_string(),
                elements: vec!["red".to_string(), "green".to_string(), "blue".to_string()],
            }
        );
    }

    #[test]
    fn test_context_declaration() {
        let resolver = build("test_context_declaration.vhd", "work");
        let types = resolver.resolve_unit("work", DesignUnitKind::Entity, "top");
        assert_eq!(
            types[0],
            ResolvedType::Enum {
                name: "t_state".to_string(),
                elements: vec!["a".to_string(), "b".to_string()],
            }
        );
    }

    #[test]
    fn test_unknown_and_cycles() {
        let resolver = build("test_unknown_and_cycles.vhd", "work");
        let types = resolver.resolve_unit("work", DesignUnitKind::Entity, "top");
        // cyclic access types cannot be resolved
        assert_eq!(
            types[0],
            ResolvedType::Unknown {
                raw: "t_a".to_string()
            }
        );
        // the record itself is resolved, but its unknown field is not
        assert_eq!(
            types[1],
            ResolvedType::Record {
                name: "t_rec".to_string(),
                fields: vec![RecordField {
                    name: "f".to_string(),
                    resolved_type: ResolvedType::Unknown {
                        raw: "t_missing".to_string()
                    },
                }],
            }
        );
    }

    #[test]
    fn test_display() {
        let resolver = build("test_display.vhd", "work");
        let types = resolver.resolve_unit("work", DesignUnitKind::Entity, "top");
        assert_eq!(types[0].to_string(), "array of std_logic");
        assert_eq!(types[1].to_string(), "t_state (low, high)");
    }
}
