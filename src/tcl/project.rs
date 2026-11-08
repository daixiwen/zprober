use crate::vhdl::{Hierarchy, QualifiedName, hierarchy};

/// load a project tcl function
use super::data::Data;
use molt::{ContextID, Exception, Interp, MoltResult, Value, check_args, molt_err, molt_ok};
use std::path::PathBuf;
use std::collections::BTreeMap;

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
    /// ghdl module
    pub ghdl_module : ghdl_sys::Module,
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

    //tcl_data.project_entries = 
    let entries : Result<Vec<ProjectEntry>, String> = top_levels
        .iter()
        .map(|top_level| {
            let design = tcl_data.vhdl_project.as_ref().unwrap().ghdl_synthesize(&top_level.parent.to_string())?;
            let top_entity = design.top_entity();
            Ok(ProjectEntry {
                path: String::new(),
                label: String::new(),
                library: top_level.parent.library.clone(),
                entity: top_level.parent.name.clone(),
                ghdl_module : top_entity,
                children: create_children_entries(hierarchy, &top_level.parent, "", &top_entity),
            })
        })
        .collect();
    
    tcl_data.project_entries = match entries {
        Ok(parsed_entries) => parsed_entries,
        Err(msg) => molt_err!("error while synthesizing project: {}", msg)?
    };

    tcl_data.send_new_hierarchy(tcl_data.project_entries.clone());
    molt_ok!()
}

fn create_children_entries(
    hierarchy: &Hierarchy,
    parent: &QualifiedName,
    parent_path: &str,
    ghdl_entity: &ghdl_sys::Module
) -> Vec<ProjectEntry> {

    tracing::event!(tracing::Level::DEBUG, "looking into childrens of {}", parent);

    let ghdl_instances : BTreeMap<String, ghdl_sys::Module> = ghdl_entity.instances()
        .iter()
        .map(|instance| 
            (
            instance.name(),
            instance.module()
        ))
        .collect();

    hierarchy
        .children_of(parent)
        .iter()
        .map(|instantiation| {
            // we need to convert from an Instantiation object from the vhdl module to something
            // useful for the GUI: a ProjectEntry
            let path = format!("{}.{}", parent_path, instantiation.label);
            let ghdl_label = instantiation.label.replace(".", "_");

            // we'll only keep the instance if we also find it in the synthesized project
            if let Some(child_module) = ghdl_instances.get(&ghdl_label) {
                tracing::event!(tracing::Level::DEBUG, "found ghdl instance for {}", ghdl_label);

                Some(            
                    ProjectEntry {
                        path: path.clone(),
                        label: instantiation.label.clone(),
                        library: instantiation.child.library.clone(),
                        entity: instantiation.child.name.clone(),
                        ghdl_module: child_module.to_owned(),
                        children: create_children_entries(hierarchy, &instantiation.child, &path, child_module),
                    }
                )
            } else {
                tracing::event!(tracing::Level::DEBUG, "did not find ghdl instance for {}", ghdl_label);
                None
            }
        }) // remove the "None" entries
        .filter(|instance| instance.is_some())
          // and take them out of the option object
        .map(|instance| instance.unwrap())
        .collect()
}

/// look for a project entry with a given path, looking through all children.
/// If everything is right, should return only one element
fn find_project_entry(entries: Vec<ProjectEntry>, instance_path: &str) -> Vec<ProjectEntry> {
    let found_entities: Vec<ProjectEntry> = entries
        .iter()
        .filter(|entry|
            entry.path == instance_path
        )
        .cloned()
        .collect();

    entries.iter().fold(found_entities, |entities, parent| {
        [
            entities,
            find_project_entry(parent.children.clone(), instance_path),
        ]
        .concat()
    })
}

/// select an instance in the project
fn cmd_select_instance(interp: &mut Interp, context_id: ContextID, argv: &[Value]) -> MoltResult {
    check_args(1, argv, 2, 2, "instance_path")?;
    let tcl_data: &mut Data = interp.context(context_id);
    let instance_path = argv[1].to_string();

    // find the path in our list
    let found_entities = find_project_entry(tcl_data.project_entries.clone(), &instance_path);

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
        let entry = found_entities.first().unwrap();
        let mut signals_and_ports = if let Some(hierarchy) = &tcl_data.vhdl_hierarchy {
            hierarchy.ports_and_signals(QualifiedName {
                library: entry.library.clone(),
                name: entry.entity.clone(),
            })
        } else {
            Vec::new()
        };

        // go through each signal and port and try to find them in the ghdl synthesis
        let ghdl_inputs = entry.ghdl_module.inputs();
        let ghdl_outputs = entry.ghdl_module.outputs();
        let ghdl_signals : Vec<ghdl_sys::Port> = Vec::default();
        for sig in signals_and_ports.iter_mut() {
            // try to find the signal in the ghdl synthesis
            let ghdl_list = match sig.mode {
                crate::vhdl::hierarchy::SignalMode::In => &ghdl_inputs,
                crate::vhdl::hierarchy::SignalMode::Out => &ghdl_outputs,
                crate::vhdl::hierarchy::SignalMode::InOut => &ghdl_outputs,
                _ => &ghdl_signals
            };
/*             if let Some(ghdl_signal) = ghdl_list.iter().find(|port| port.name == sig.name) {
                sig.size = ghdl_signal.width;
            }
*/
            let begin_pattern = format!("{}[",sig.name);
            sig.size = ghdl_list.iter().fold(0, | size, ghdl_sig | {
                size + if (ghdl_sig.name == sig.name) || (ghdl_sig.name.starts_with(&begin_pattern)) { ghdl_sig.width } else { 0 }
            });
        }
        tcl_data.send_selected_instance(instance_path, signals_and_ports);
        molt_ok!()
    }
}
