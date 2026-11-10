use serde::{Deserialize, Serialize};

/// an entity object is a mirror of a tcl::project::ProjectEntry, but holding data
/// the data about probes. It is stored in a separate structure from ProjectEntry
/// because this is saved with the project file and kept independent, so that we
/// can compare each time the VHDL files are parsed again, and keep objects if we
/// have some defined probes in entities that don't exist in the VHDL project any
/// more
#[derive(Default, Serialize, Deserialize)]
pub struct Entity {
    /// entity name
    pub label: String,
    /// probes list
    pub probes: Vec<super::probe::Probe>,
    /// children entities
    pub children: Vec<Entity>,
}

impl Entity {
    /// find an entity in the hierarchy from the given path, returns None if entity is not found
    pub fn find_path(&self, path: &str) -> Option<&Entity> {
        self.find_path_vec(path.split(".").collect())
    }

    /// find_path, but with a vector input, called recursively
    fn find_path_vec(&self, path: Vec<&str>) -> Option<&Entity> {
        if path.is_empty() {
            Some(self)
        } else {
            // look for immediate children
            match self.children.iter().find(|e| e.label == path[0]) {
                None => None,
                Some(child) => child.find_path_vec(path[1..].to_vec()),
            }
        }
    }

    /// find an entity in the hierarchy from the given path, creating the hierarchy if required
    pub fn find_path_mut(&mut self, path: &str) -> &mut Entity {
        self.find_path_mut_vec(path.split(".").collect())
    }

    /// find_path_mut, but with a vector input, called recursively
    fn find_path_mut_vec(&mut self, path: Vec<&str>) -> &mut Entity {
        if path.is_empty() {
            self
        } else {
            let child_name = path[0];
            let remaining_path = path[1..].to_vec();

            let child = match self.children.iter().find(|e| e.label == child_name) {
                None => self.children.push_mut(Entity {
                    label: child_name.to_owned(),
                    probes: Default::default(),
                    children: Default::default(),
                }),
                Some(_) =>
                // we have to search again, this time as mutable
                {
                    self.children
                        .iter_mut()
                        .find(|e| e.label == child_name)
                        .unwrap()
                }
            };

            child.find_path_mut_vec(remaining_path)
        }
    }
}
