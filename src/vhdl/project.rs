//! VHDL project definition
//!
//! A project is a set of VHDL libraries, each containing a list of source files.
//! Projects can be built in two ways:
//! - From a `vhdl_ls.toml` configuration file (the native format used by
//!   [vhdl_ls](https://github.com/VHDL-LS/rust_hdl))
//! - Programmatically, by creating libraries and adding files to them

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A VHDL library, holding a name and a list of source files
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Library {
    /// Library name as declared in the VHDL code (e.g. `neorv32`)
    pub name: String,
    /// List of source files (absolute paths)
    pub files: Vec<PathBuf>,
}

impl Library {
    /// Create a new library with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            files: Vec::new(),
        }
    }

    /// Add a source file to the library
    pub fn add_file(&mut self, file: impl Into<PathBuf>) {
        self.files.push(file.into());
    }
}
/// VHDL standard version, mirroring the `standard` key in `vhdl_ls.toml`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VhdlStandard {
    Vhdl1993,
    #[default]
    Vhdl2008,
    Vhdl2019,
}

impl VhdlStandard {
    /// Convert to the corresponding `vhdl_lang::VHDLStandard`
    pub fn to_vhdl_lang(&self) -> vhdl_lang::VHDLStandard {
        match self {
            VhdlStandard::Vhdl1993 => vhdl_lang::VHDLStandard::VHDL1993,
            VhdlStandard::Vhdl2008 => vhdl_lang::VHDLStandard::VHDL2008,
            VhdlStandard::Vhdl2019 => vhdl_lang::VHDLStandard::VHDL2019,
        }
    }
}
/// A VHDL project, made of one or several libraries
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Project {
    /// Libraries, keyed by name
    libraries: BTreeMap<String, Library>,
    /// The root directory of the project, used to resolve relative paths
    root_dir: PathBuf,
    /// The VHDL standard to use for parsing
    standard: VhdlStandard,
}

impl Project {
    /// Create a new empty project, rooted at `root_dir`
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            libraries: BTreeMap::new(),
            root_dir: root_dir.into(),
            standard: VhdlStandard::default(),
        }
    }

    /// Build a project from a `vhdl_ls.toml` file.
    ///
    /// File paths in the configuration can be absolute, or relative to the
    /// parent directory of the `vhdl_ls.toml` file. Wildcard patterns (globs)
    /// are supported.
    pub fn from_vhdl_ls_toml(toml_file: impl AsRef<Path>) -> Result<Self, String> {
        let toml_file = toml_file.as_ref();
        let root_dir = toml_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let config = TomlConfig::from_file(toml_file)?;
        Self::from_toml_config(config, root_dir)
    }

    /// Build a project from a parsed `vhdl_ls.toml` configuration
    fn from_toml_config(config: TomlConfig, root_dir: PathBuf) -> Result<Self, String> {
        let standard = match config.standard.as_deref() {
            None => VhdlStandard::default(),
            Some("1993") => VhdlStandard::Vhdl1993,
            Some("2008") => VhdlStandard::Vhdl2008,
            Some("2019") => VhdlStandard::Vhdl2019,
            Some(other) => {
                return Err(format!(
                    "Invalid VHDL standard '{}' in configuration (expected '1993', '2008' or '2019')",
                    other
                ))
            }
        };

        let mut project = Self::new(root_dir);
        project.standard = standard;
        for (name, lib) in config.libraries {
            project.add_library(&name);
            for pattern in &lib.files {
                let full_pattern = project.resolve_path(pattern);
                let files = expand_pattern(&full_pattern);
                if files.is_empty() {
                    tracing::warn!(
                        "No files matched pattern '{}' for library '{}'",
                        pattern,
                        name
                    );
                }
                for file in files {
                    project.add_file(&name, file);
                }
            }
        }
        Ok(project)
    }

    /// Resolve a path relative to the project root directory.
    /// Returns the path unchanged if it is absolute.
    pub fn resolve_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root_dir.join(path)
        }
    }

    /// Set the VHDL standard to use for parsing
    pub fn set_standard(&mut self, standard: VhdlStandard) {
        self.standard = standard;
    }

    /// Get the VHDL standard used for parsing
    pub fn standard(&self) -> VhdlStandard {
        self.standard
    }

    /// Get the project root directory
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Create a new library in the project
    pub fn add_library(&mut self, name: &str) {
        self.libraries
            .entry(name.to_string())
            .or_insert_with(|| Library::new(name));
    }

    /// Add a source file to a library. Creates the library if it does not exist.
    pub fn add_file(&mut self, library: &str, file: impl Into<PathBuf>) {
        let file = file.into();
        let lib = self
            .libraries
            .entry(library.to_string())
            .or_insert_with(|| Library::new(library));
        lib.add_file(file);
    }

    /// Get the list of libraries in the project
    pub fn libraries(&self) -> impl Iterator<Item = &Library> {
        self.libraries.values()
    }

    /// Get a library by name
    pub fn library(&self, name: &str) -> Option<&Library> {
        self.libraries.get(name)
    }

    /// Get the total number of source files in the project
    pub fn num_files(&self) -> usize {
        self.libraries.values().map(|l| l.files.len()).sum()
    }
}

/// Internal representation of the `vhdl_ls.toml` configuration file
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlConfig {
    #[serde(default)]
    standard: Option<String>,
    #[serde(default)]
    libraries: BTreeMap<String, TomlLibrary>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlLibrary {
    #[serde(default)]
    files: Vec<String>,
}

impl TomlConfig {
    /// Load a `vhdl_ls.toml` configuration from a file
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
        Self::from_str(&contents)
    }

    /// Parse a `vhdl_ls.toml` configuration from a string
    pub fn from_str(contents: &str) -> Result<Self, String> {
        toml::from_str(contents).map_err(|e| format!("Failed to parse TOML: {}", e))
    }
}
/// Expand a glob pattern into a list of matching files.
///
/// This is used to support the wildcard patterns found in `vhdl_ls.toml`
/// files, e.g. `'src/*.vhd'`.
pub fn expand_pattern(pattern: &Path) -> Vec<PathBuf> {
    let pattern_str = pattern.to_string_lossy();
    match glob::glob(&pattern_str) {
        Ok(paths) => paths
            .filter_map(|p| p.ok())
            .filter(|p| p.is_file())
            .collect(),
        Err(e) => {
            tracing::warn!("Invalid glob pattern '{}': {}", pattern_str, e);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn example_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("example/neorv32-main/rtl")
    }

    #[test]
    fn test_parse_vhdl_ls_toml_string() {
        let config = TomlConfig::from_str(
            r#"
standard = "2008"
[libraries]
lib1.files = ['src/lib1/*.vhd']
lib2.files = ['src/lib2/*.vhd', 'src/common/*.vhd']
"#,
        )
        .unwrap();
        assert_eq!(config.standard.as_deref(), Some("2008"));
        assert_eq!(config.libraries.len(), 2);
        assert_eq!(config.libraries["lib1"].files.len(), 1);
        assert_eq!(config.libraries["lib2"].files.len(), 2);
    }

    #[test]
    fn test_invalid_standard_rejected() {
        let config = TomlConfig::from_str("standard = \"bad\"").unwrap();
        let result = Project::from_toml_config(config, PathBuf::from("."));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_example_project() {
        let example_dir = example_dir();
        let toml_file = example_dir.join("vhdl_ls.toml");
        assert!(
            toml_file.exists(),
            "Test requires the example project at {}",
            toml_file.display()
        );
        let project = Project::from_vhdl_ls_toml(&toml_file).unwrap();
        assert_eq!(project.libraries().count(), 1);
        let lib = project.library("neorv32").unwrap();
        assert!(lib.files.len() > 40, "Expected ~61 files in neorv32 library");
        assert!(project.num_files() > 40);
        assert_eq!(project.standard(), VhdlStandard::Vhdl2008);
    }

    #[test]
    fn test_expand_pattern() {
        let pattern = example_dir().join("core/*.vhd");
        let files = expand_pattern(&pattern);
        assert!(files.len() > 40);
        assert!(files.iter().all(|f| f.extension().unwrap() == "vhd"));
    }

    #[test]
    fn test_manual_project() {
        let mut project = Project::new(".");
        project.add_library("lib1");
        project.add_library("lib2");
        project.add_file("lib2", "file2.vhd");
        assert_eq!(project.libraries().count(), 2);
        assert_eq!(project.library("lib1").unwrap().files.len(), 0);
        assert_eq!(project.library("lib2").unwrap().files.len(), 1);
    }
}

