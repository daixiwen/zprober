use crate::vhdl::{Hierarchy, QualifiedName, hierarchy};

/// load a project tcl function
use super::data::Data;
use molt::{ContextID, Exception, Interp, MoltResult, Value, check_args, molt_err, molt_ok};
use std::path::PathBuf;

/// Describes an entry in the project hierarchy
#[derive(Debug)]
pub struct ProjectEntry {
    /// label name
    pub label: String,
    /// library name
    pub library: String,
    /// entity name
    pub entity: String,
    /// list of instantiated entities
    pub children: Vec<ProjectEntry>,
}

/// string conversion
impl std::fmt::Display for ProjectEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.label.is_empty() {
            write!(f, "{}", self.entity)
        } else {
            write!(f, "{} ({})", self.label, self.entity)
        }
    }
}

/// install custom TCL commands from this file
pub fn install_tcl(interpreter: &mut molt::Interp, data_id: molt::ContextID) {
    interpreter.add_context_command("import_vhdl_ls", cmd_import_vhdl_ls, data_id);
}

/// new project from a vhdl_ls.toml file
fn cmd_import_vhdl_ls(interp: &mut Interp, context_id: ContextID, argv: &[Value]) -> MoltResult {
    let tracing_span = tracing::span!(tracing::Level::DEBUG, "tcl command import_vhdl_ls");
    let _enter = tracing_span.enter();

    check_args(1, argv, 2, 2, "filename")?;

    let tcl_data: &mut Data = interp.context(context_id);
    let filepath = PathBuf::from(&argv[1].to_string());

    let filename = filepath.clone().into_os_string().into_string().unwrap();

    if !filepath.exists() {
        molt_err!("file {} not found", filename)?;
    }

    tracing::event!(tracing::Level::DEBUG, filename, "opening vhdl_ls file");

    match crate::vhdl::Project::from_vhdl_ls_toml(&filepath) {
        Ok(project) => tcl_data.vhdl_project = Some(project),
        Err(msg) => {
            molt_err!("Error loading project {}: {}", filename, msg)?;
        }
    }

    tracing::event!(tracing::Level::DEBUG, filename, "creating vhdl hierarchy");
    match hierarchy::Hierarchy::build(tcl_data.vhdl_project.as_ref().unwrap()) {
        Ok(hierarchy) => tcl_data.vhdl_hierarchy = Some(hierarchy),
        Err(msg) => {
            molt_err!("Error creating project hierarchy for {}: {}", filename, msg)?;
        }
    }

    let hierarchy = tcl_data.vhdl_hierarchy.as_ref().unwrap();
    let top_levels = hierarchy.capz_instantiators(vec!["fcapz_ela_polarfire".to_owned()]);

    let num_top_levels = top_levels.len();
    tracing::event!(
        tracing::Level::INFO,
        filename,
        num_top_levels,
        "top levels found"
    );

    if num_top_levels == 0 {
        molt_err!(
            "No entity instantiating am fpgacapzero core has been found in project {}",
            filename
        )?;
    }

    let project_entries: Vec<ProjectEntry> = top_levels
        .iter()
        .map(|top_level| ProjectEntry {
            label: String::new(),
            library: top_level.parent.library.clone(),
            entity: top_level.parent.name.clone(),
            children: create_children_entries(hierarchy, &top_level.parent),
        })
        .collect();

    tcl_data.send_new_hierarchy(project_entries);
    molt_ok!()
}

fn create_children_entries(hierarchy: &Hierarchy, parent: &QualifiedName) -> Vec<ProjectEntry> {
    hierarchy
        .children_of(parent)
        .iter()
        .map(|instantiation| ProjectEntry {
            label: instantiation.label.clone(),
            library: instantiation.child.library.clone(),
            entity: instantiation.child.name.clone(),
            children: create_children_entries(hierarchy, &instantiation.child),
        })
        .collect()
}
