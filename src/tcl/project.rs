use crate::vhdl::{Hierarchy, QualifiedName, hierarchy};

/// load a project tcl function
use super::data::Data;
use molt::{ContextID, Exception, Interp, MoltResult, Value, check_args, molt_err, molt_ok};
use std::path::PathBuf;

/// Describes an entry in the project hierarchy
#[derive(Debug, Clone)]
pub struct ProjectEntry {
    /// entry path
    pub path: String,
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
    interpreter.add_context_command("select_instance", cmd_select_instance, data_id);
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

    tcl_data.project_entries = top_levels
        .iter()
        .map(|top_level| ProjectEntry {
            path: String::new(),
            label: String::new(),
            library: top_level.parent.library.clone(),
            entity: top_level.parent.name.clone(),
            children: create_children_entries(hierarchy, &top_level.parent, ""),
        })
        .collect();

    tcl_data.send_new_hierarchy(tcl_data.project_entries.clone());
    molt_ok!()
}

fn create_children_entries(
    hierarchy: &Hierarchy,
    parent: &QualifiedName,
    parent_path: &str,
) -> Vec<ProjectEntry> {
    hierarchy
        .children_of(parent)
        .iter()
        .map(|instantiation| {
            let path = format!("{}.{}", parent_path, instantiation.label);
            ProjectEntry {
                path: path.clone(),
                label: instantiation.label.clone(),
                library: instantiation.child.library.clone(),
                entity: instantiation.child.name.clone(),
                children: create_children_entries(hierarchy, &instantiation.child, &path),
            }
        })
        .collect()
}

/// look for a project entry with a given path, looking through all children
/// If everything is right, should return only one element
fn find_project_entry(entries: Vec<ProjectEntry>, instance_path: &str) -> Vec<ProjectEntry> {
    let found_entities: Vec<ProjectEntry> = entries
        .iter()
        .filter(|entry| {
            tracing::event!(tracing::Level::DEBUG, "looking into {}", entry.path);
            entry.path == instance_path})
        .map(|entry| entry.clone())
        .collect();

    entries.iter().fold(found_entities, | entities, parent| [ entities, find_project_entry(parent.children.clone(), instance_path)].concat())
}

/// select an instance in the project
fn cmd_select_instance(interp: &mut Interp, context_id: ContextID, argv: &[Value]) -> MoltResult {
    check_args(1, argv, 2, 2, "instance_path")?;
    let tcl_data: &mut Data = interp.context(context_id);
    let instance_path = argv[1].to_string();

    // find the path in our list
    let found_entities=  find_project_entry(tcl_data.project_entries.clone(),&instance_path);

    if found_entities.is_empty() {
        molt_err!(format!("entity {instance_path} not found"))
    } else {
        if found_entities.len() > 1 {
            tracing::event!(
                tracing::Level::ERROR,
                "found {} different entries with path {}",
                found_entities.len(),
                instance_path
            );
        }
        let signals_and_ports = if let Some(hierarchy) = &tcl_data.vhdl_hierarchy {
            let entry = found_entities.first().unwrap();
            hierarchy.ports_and_signals(QualifiedName {
                library: entry.library.clone(),
                name: entry.entity.clone(),
            })
        } else {
            Vec::new()
        };
        tcl_data.send_selected_instance(instance_path, signals_and_ports);
        molt_ok!()
    }
}
