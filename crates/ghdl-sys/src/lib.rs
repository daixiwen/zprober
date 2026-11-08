//! Raw bindings and a small safe API for the GHDL shared library (`libghdl`).
//!
//! The `ghdl` git submodule (under this crate) is configured and compiled by
//! `build.rs` (`./configure --enable-libghdl && make all.libghdl && make libs`),
//! producing `ghdl/lib/libghdl-<version>.so` plus the VHDL standard libraries
//! in `ghdl/lib/ghdl/`.
//!
//! # Session lifecycle
//!
//! GHDL keeps all its state in process-global variables, and some
//! initializations cannot run twice, so the API is tied to a [`Ghdl`] session
//! object enforcing the correct order:
//!
//! 1. [`Ghdl::init`] - library initialization (must be called first, exactly
//!    once per process).
//! 2. [`Ghdl::import`] - import (analyze) VHDL files into one or several
//!    libraries, like `ghdl -i`. Libraries are saved in the work directory
//!    (`<lib>-obj<std>.cf` files), and stay in memory.
//! 3. [`Ghdl::compile`] - analyze the imported units (like `ghdl -m`),
//!    needed before synthesis.
//! 4. [`Ghdl::synth`] - synthesize a design (like `ghdl --synth`), starting
//!    either from the in-memory libraries or from libraries previously saved
//!    on disk. May be called several times. Files can only be passed to the
//!    *first* synthesis of a process (one-shot mode).
//! 5. Browse the resulting netlist through [`Design`], [`Module`],
//!    [`Instance`] and [`Net`].
//!
//! The names of the netlist modules are GHDL-generated, in the form
//! `entity_Barchitecture[_Llibrary][_generic values...][_sha1]`; see
//! [`Module::parse_name`].
//!
//! All functions must be called from the same thread (GHDL is not
//! thread-safe); `Ghdl` is `!Send`/`!Sync` to enforce this.

use std::cell::Cell;
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Raw FFI
// ---------------------------------------------------------------------------

/// All GHDL exported types are 32-bit identifiers; 0 means "none".
/// See `ghdl/src/synth/include/synth.h` for the reference ABI.
mod ffi {
    use std::ffi::c_char;

    pub type Module = u32;
    pub type Instance = u32;
    pub type Net = u32;
    pub type Input = u32;
    pub type Sname = u32;
    pub type NameId = u32;
    /// VHDL AST node (a 32-bit index into the node table in GHDL 5.x).
    pub type Iir = u32;

    extern "C" {
        //  Initialization.  The sequence is the one used by pyGHDL:
        //  libghdl_init() must be the very first call into the library.
        pub fn libghdl_init();
        pub fn libghdl__set_hooks_for_analysis();
        pub fn libghdl__set_exec_prefix(prefix: *const c_char, len: u32);
        //  Extra initialization before synthesis.
        pub fn ghdlsynth__init_for_ghdl_synth();

        //  Error reporting (Errorout.Nbr_Errors).
        pub static errorout__nbr_errors: u32;

        //  Driver options and analysis (src/vhdl/libghdl/libghdl.ads).
        pub fn libghdl__set_option(opt: *const c_char, len: u32) -> i32;
        pub fn libghdl__analyze_init_status() -> i32;
        //  Full semantic analysis of one file (ghdl -a semantics).
        #[allow(dead_code)]
        pub fn libghdl__analyze_file(file: *const c_char, len: u32) -> Iir;
        //  Parse-only import (ghdl -i semantics).
        pub fn name_table__get_identifier_with_len(s: *const u8, len: u32) -> NameId;
        pub fn vhdl__sem_lib__load_file_name(id: NameId) -> Iir;
        pub fn vhdl__nodes__get_first_design_unit(f: Iir) -> Iir;
        pub fn vhdl__nodes__get_chain(u: Iir) -> Iir;
        pub fn vhdl__nodes__set_chain(u: Iir, c: Iir);
        pub fn libraries__add_design_unit_into_library(u: Iir);
        pub fn libraries__save_work_library();
        pub fn libraries__load_work_library(empty: bool);

        //  Synthesis (src/vhdl/libghdl/libghdl_synth.ads).
        //  ARGV is the same command line as `ghdl synth ...`.
        pub fn ghdlsynth__ghdl_synth(init: i32, argc: i32, argv: *const *const c_char) -> Module;

        //  Compilation of parsed units (ghdl -m semantics).
        pub static mut flags__flag_elaborate_with_outdated: bool;
        pub fn libraries__get_libraries_chain() -> Iir;
        pub fn vhdl__nodes__get_identifier(lib: Iir) -> NameId;
        //  Libraries.Work_Library is a variable (the current work library).
        pub static libraries__work_library: Iir;
        pub fn vhdl__nodes__get_design_file_chain(lib: Iir) -> Iir;
        pub fn vhdl__nodes__get_date(u: Iir) -> u32;
        //  Load (analyze) a design unit; handles the unit date state, so
        //  units are analyzed exactly once even when referenced several
        //  times.  The unit is passed again as location.
        pub fn vhdl__sem_lib__load_design_unit(u: Iir, loc: Iir);

        //  Netlist browsing (src/synth/include/synth.h).
        pub fn netlists__get_id(m: Module) -> u32;
        pub fn netlists__get_module_name(m: Module) -> Sname;
        pub fn netlists__get_first_sub_module(m: Module) -> Module;
        pub fn netlists__get_next_sub_module(m: Module) -> Module;
        pub fn netlists__get_nbr_inputs(m: Module) -> u32;
        pub fn netlists__get_nbr_outputs(m: Module) -> u32;
        pub fn netlists__get_nbr_params(m: Module) -> u32;
        pub fn netlists__get_first_instance(m: Module) -> Instance;
        pub fn netlists__get_next_instance(i: Instance) -> Instance;
        pub fn netlists__get_instance_name(i: Instance) -> Sname;
        pub fn netlists__get_module(i: Instance) -> Module;
        pub fn netlists__get_self_instance(m: Module) -> Instance;
        pub fn netlists__get_width(n: Net) -> u32;
        pub fn netlists__get_input(i: Instance, idx: u32) -> Input;
        pub fn netlists__get_output(i: Instance, idx: u32) -> Net;
        pub fn netlists__get_driver(inp: Input) -> Net;
        pub fn netlists__get_net_parent(n: Net) -> Instance;
        pub fn netlists__get_first_sink(n: Net) -> Input;
        pub fn netlists__get_next_sink(i: Input) -> Input;
        pub fn netlists__get_input_parent(i: Input) -> Instance;
        pub fn netlists__get_param_uns32(i: Instance, idx: u32) -> u32;
        pub fn netlists__get_sname_kind(n: Sname) -> u32;
        pub fn netlists__get_sname_prefix(n: Sname) -> Sname;
        pub fn netlists__get_sname_suffix(n: Sname) -> NameId;
        pub fn netlists__get_sname_version(n: Sname) -> u32;
        pub fn netlists__utils__get_input_name(m: Module, idx: u32) -> Sname;
        pub fn netlists__utils__get_output_name(m: Module, idx: u32) -> Sname;
        pub fn netlists__utils__get_input_width(m: Module, idx: u32) -> u32;
        pub fn netlists__utils__get_output_width(m: Module, idx: u32) -> u32;
        pub fn netlists__utils__get_inout_flag(m: Module, idx: u32) -> u32;
        pub fn netlists__utils__get_param_name(m: Module, idx: u32) -> Sname;
        pub fn netlists__utils__get_param_type(m: Module, idx: u32) -> u32;

        //  Name table.
        pub fn name_table__get_address(id: NameId) -> *const c_char;
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GhdlError {
    pub msg: String,
}

impl std::fmt::Display for GhdlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for GhdlError {}

impl From<std::ffi::NulError> for GhdlError {
    fn from(e: std::ffi::NulError) -> Self {
        err(format!("string contains interior NUL byte: {e}"))
    }
}

fn err(msg: impl Into<String>) -> GhdlError {
    GhdlError { msg: msg.into() }
}

fn cstring(s: &str) -> Result<CString, std::ffi::NulError> {
    CString::new(s)
}

// ---------------------------------------------------------------------------
// Name utilities
// ---------------------------------------------------------------------------

fn name_id_to_string(id: ffi::NameId) -> String {
    if id == 0 {
        return String::new();
    }
    unsafe {
        CStr::from_ptr(ffi::name_table__get_address(id))
            .to_string_lossy()
            .into_owned()
    }
}

/// Sname kinds (must match `Sname_Kind` in `src/synth/netlists.ads`).
mod sname_kind {
    pub const UNIQUE: u32 = 0;
    pub const SYSTEM: u32 = 1;
    pub const USER: u32 = 2;
    pub const FIELD: u32 = 3;
    pub const VERSION: u32 = 4;
}

/// Convert a hierarchical `Sname` to its flat string representation
/// (same convention as the netlist printers: prefixes joined with `_`).
///
/// Note: only `Sname_User`, `Sname_Field` and `Sname_Version` names have a
/// prefix.  For `Sname_System` names the prefix field holds the reserved
/// `System_Sname` marker, and for `Sname_Unique` it is absent, so the
/// prefix must not be read for those kinds.
fn sname_to_string(n: ffi::Sname) -> String {
    if n == 0 {
        return "*none*".to_string();
    }
    let kind = unsafe { ffi::netlists__get_sname_kind(n) };
    //  Prefix is only meaningful for these kinds.
    let prefix = match kind {
        sname_kind::USER | sname_kind::FIELD | sname_kind::VERSION => unsafe {
            ffi::netlists__get_sname_prefix(n)
        },
        _ => 0,
    };
    let s = match kind {
        sname_kind::USER | sname_kind::SYSTEM => {
            name_id_to_string(unsafe { ffi::netlists__get_sname_suffix(n) })
        }
        sname_kind::FIELD => format!(
            "[{}]",
            name_id_to_string(unsafe { ffi::netlists__get_sname_suffix(n) })
        ),
        sname_kind::VERSION => format!("n{}", unsafe { ffi::netlists__get_sname_version(n) }),
        sname_kind::UNIQUE => format!("n{}", unsafe { ffi::netlists__get_sname_version(n) }),
        _ => format!("*kind{kind}*"),
    };
    if prefix != 0 {
        let pfx = sname_to_string(prefix);
        if kind == sname_kind::FIELD {
            format!("{pfx}{s}")
        } else if kind == sname_kind::VERSION {
            format!("{pfx}n{}", s.trim_start_matches('n'))
        } else {
            format!("{pfx}_{s}")
        }
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum State {
    /// Library initialized, no VHDL loaded yet.
    Fresh,
    /// `import()` was used: libraries are loaded in memory.
    LibrariesLoaded,
    /// At least one synthesis was performed.
    SynthReady,
}

/// A GHDL session. Only one session can exist per process, and all its
/// methods must be called from the thread that created it.
pub struct Ghdl {
    state: Cell<State>,
}

impl Ghdl {
    /// Initialize the library. `prefix` must point to a tree containing the
    /// compiled VHDL standard libraries (i.e. `<prefix>/lib/ghdl/std/...`).
    /// The build script provides the submodule directory through the
    /// `GHDL_SYS_PREFIX` environment variable, which is the default.
    pub fn init(prefix: Option<&Path>) -> Result<Ghdl, GhdlError> {
        let prefix = match prefix {
            Some(p) => p.to_path_buf(),
            None => PathBuf::from(
                //  Set at compile time by build.rs (cargo:rustc-env).
                option_env!("GHDL_SYS_PREFIX")
                    .ok_or_else(|| err("GHDL_SYS_PREFIX not set and no prefix given"))?,
            ),
        };

        //  On Windows, make the DLL and its MinGW runtime dependencies
        //  loadable before the first FFI call.
        #[cfg(target_os = "windows")]
        unsafe {
            let lib_dir = prefix.join("lib");
            let path = std::env::var("PATH").unwrap_or_default();
            std::env::set_var("PATH", format!("{};{path}", lib_dir.display()));
        }

        let prefix_c = cstring(prefix.to_str().ok_or_else(|| err("prefix is not UTF-8"))?)?;
        unsafe {
            ffi::libghdl_init();
            ffi::libghdl__set_hooks_for_analysis();
            ffi::libghdl__set_exec_prefix(prefix_c.as_ptr(), prefix_c.as_bytes().len() as u32);
            ffi::ghdlsynth__init_for_ghdl_synth();
        }
        Ok(Ghdl {
            state: Cell::new(State::Fresh),
        })
    }

    fn nbr_errors() -> u32 {
        unsafe { ffi::errorout__nbr_errors }
    }

    fn check_errors(&self, what: &str) -> Result<(), GhdlError> {
        let n = Self::nbr_errors();
        if n > 0 {
            Err(err(format!(
                "{what}: {n} error(s) (see stderr for GHDL messages)"
            )))
        } else {
            Ok(())
        }
    }

    fn set_option(&self, opt: &str) -> Result<(), GhdlError> {
        let c = cstring(opt)?;
        let r = unsafe { ffi::libghdl__set_option(c.as_ptr(), c.as_bytes().len() as u32) };
        if r != 0 {
            Err(err(format!("invalid option '{opt}'")))
        } else {
            Ok(())
        }
    }

    /// Import VHDL files into libraries, like `ghdl -i`: files are parsed
    /// and their design units are registered into the library, without
    /// semantic analysis (so files can be imported in any order).  The
    /// libraries are saved as `<workdir>/<name>-obj<ver>.cf` and stay in
    /// memory.  May be called several times (library setup is performed
    /// only once, on the first call).  Re-importing a library replaces its
    /// content.
    pub fn import(&self, opts: &ImportOptions) -> Result<(), GhdlError> {
        let st = self.state.get();
        assert!(st != State::SynthReady, "cannot import after synth()");

        self.set_option(&format!("--std={}", opts.std))?;
        self.set_option(&format!("--workdir={}", opts.workdir.display()))?;
        for p in &opts.library_paths {
            self.set_option(&format!("-P{}", p.display()))?;
        }

        if st == State::Fresh {
            //  Loads the std library (must happen only once).
            let r = unsafe { ffi::libghdl__analyze_init_status() };
            if r != 0 {
                return Err(err("analysis initialization failed"));
            }
        }

        for g in &opts.groups {
            self.set_option(&format!("--work={}", g.library))?;
            //  Switch the current work library (set_option only changes the
            //  name).  empty=true: just create it if it does not exist yet,
            //  and switch to it if it is already loaded in memory.
            unsafe { ffi::libraries__load_work_library(true) };
            for f in &g.files {
                let fs = f
                    .to_str()
                    .ok_or_else(|| err(format!("path {f:?} is not UTF-8")))?;
                //  ghdl -i semantics: parse the file only (import), the
                //  semantic analysis happens during elaboration/synthesis.
                //  This allows files to be imported in any order.
                let file_id = unsafe {
                    ffi::name_table__get_identifier_with_len(fs.as_ptr(), fs.len() as u32)
                };
                let design_file = unsafe { ffi::vhdl__sem_lib__load_file_name(file_id) };
                self.check_errors(&format!("import of {}", f.display()))?;
                if design_file == 0 {
                    return Err(err(format!("cannot load {}", f.display())));
                }
                //  Add the design units to the work library.
                let mut unit = unsafe { ffi::vhdl__nodes__get_first_design_unit(design_file) };
                while unit != 0 {
                    let next = unsafe { ffi::vhdl__nodes__get_chain(unit) };
                    unsafe { ffi::vhdl__nodes__set_chain(unit, 0) };
                    unsafe { ffi::libraries__add_design_unit_into_library(unit) };
                    unit = next;
                }
            }
            //  Persist the library to the work directory (like ghdl -i does).
            unsafe { ffi::libraries__save_work_library() };
            self.check_errors(&format!("saving library {}", g.library))?;
        }

        self.state.set(State::LibrariesLoaded);
        Ok(())
    }

    /// Compile the imported design units: run the semantic analysis of all
    /// units that were only parsed by [`Ghdl::import`] (the equivalent of
    /// `ghdl -m`).  Required before [`Ghdl::synth`] when the files were
    /// imported with parse-only semantics, since synthesis elaborates the
    /// design and needs fully analyzed units.
    pub fn compile(&self) -> Result<(), GhdlError> {
        let st = self.state.get();
        if st == State::Fresh {
            return Err(err("nothing was imported; call import() first"));
        }
        assert!(st != State::SynthReady, "cannot compile after synth()");

        //  Date_Parsed (units that still need analysis); see vhdl-nodes.ads.
        const DATE_PARSED: u32 = 4;

        //  Same as the ghdl -a / -m commands: allow analysis of units that
        //  are referenced but not yet analyzed.
        unsafe { ffi::flags__flag_elaborate_with_outdated = true };

        //  Analyze the units of each library with that library set as the
        //  work library, so that `library work` / `use work.x` clauses
        //  inside the files resolve to the correct library (like a
        //  `ghdl -i --work=<lib>` / `ghdl -m --work=<lib>` invocation).
        let mut lib = unsafe { ffi::libraries__get_libraries_chain() };
        while lib != 0 {
            let name = name_id_to_string(unsafe { ffi::vhdl__nodes__get_identifier(lib) });
            lib = unsafe { ffi::vhdl__nodes__get_chain(lib) };
            if name == "std" || name == "ieee" || name.is_empty() || name.starts_with('*') {
                //  std cannot be set as work library; std/ieee units are
                //  pre-analyzed (nothing to compile).
                continue;
            }
            self.set_option(&format!("--work={name}"))?;
            unsafe { ffi::libraries__load_work_library(true) };

            let lib = unsafe { ffi::libraries__work_library };
            let mut file = unsafe { ffi::vhdl__nodes__get_design_file_chain(lib) };
            while file != 0 {
                let mut unit = unsafe { ffi::vhdl__nodes__get_first_design_unit(file) };
                while unit != 0 {
                    let date = unsafe { ffi::vhdl__nodes__get_date(unit) };
                    if date == DATE_PARSED {
                        //  Load_Design_Unit (and not Finish_Compilation) so
                        //  that the date state is handled properly and
                        //  units are analyzed only once.
                        unsafe { ffi::vhdl__sem_lib__load_design_unit(unit, unit) };
                        self.check_errors(&format!("compilation of {name}"))?;
                    }
                    unit = unsafe { ffi::vhdl__nodes__get_chain(unit) };
                }
                file = unsafe { ffi::vhdl__nodes__get_chain(file) };
            }
            unsafe { ffi::libraries__save_work_library() };
            self.check_errors(&format!("saving library {name}"))?;
        }

        //  Restore the default work library.
        self.set_option("--work=work")?;
        unsafe { ffi::libraries__load_work_library(true) };
        Ok(())
    }

    /// Synthesize a design, like `ghdl --synth`. Returns the netlist of the
    /// top design. May be called several times; `opts.files` is only allowed
    /// on the first call of a fresh session (one-shot mode).
    pub fn synth(&self, opts: &SynthOptions) -> Result<Design, GhdlError> {
        let st = self.state.get();

        let mut args: Vec<String> = Vec::new();
        args.push(format!("--std={}", opts.std));
        if let Some(wd) = &opts.workdir {
            args.push(format!("--workdir={}", wd.display()));
        }
        for p in &opts.library_paths {
            args.push(format!("-P{}", p.display()));
        }
        if let Some(kh) = &opts.keep_hierarchy {
            args.push(format!("--keep-hierarchy={kh}"));
        }
        if opts.latches {
            args.push("--latches".to_string());
        }
        if let Some(tn) = &opts.top_name {
            args.push(format!("--top-name={tn}"));
        }

        let has_files = !opts.files.is_empty();
        if has_files {
            if st != State::Fresh {
                return Err(err(
                    "files in SynthOptions are only allowed on the first synth() \
                     of a session; use import() instead",
                ));
            }
            //  One-shot mode: import the files, interleaving --work= options
            //  (supported by the synth file loader).
            for g in &opts.files {
                args.push(format!("--work={}", g.library));
                for f in &g.files {
                    args.push(f.to_string_lossy().into_owned());
                }
            }
            args.push("-e".to_string());
        }
        args.push(opts.top.to_string());

        //  init=1 performs the compile initialization (library setup).  It
        //  must not run if the libraries were already initialized (either by
        //  import() or by a previous synth()).
        let init = if st == State::Fresh { 1 } else { 0 };

        let argv: Vec<CString> = args
            .iter()
            .map(|a| cstring(a).map_err(|e| err(format!("bad argument {a:?}: {e}"))))
            .collect::<Result<_, _>>()?;
        let argv_ptrs: Vec<*const c_char> = argv.iter().map(|a| a.as_ptr()).collect();

        let module =
            unsafe { ffi::ghdlsynth__ghdl_synth(init, argv_ptrs.len() as i32, argv_ptrs.as_ptr()) };
        self.check_errors("synthesis")?;
        if module == 0 {
            return Err(err("synthesis failed (see stderr for GHDL messages)"));
        }

        self.state.set(State::SynthReady);
        Ok(Design {
            top: Module(module),
        })
    }
}

// ---------------------------------------------------------------------------
// Import / synth options
// ---------------------------------------------------------------------------

/// A group of files to analyze into one library.
pub struct ImportGroup {
    pub library: String,
    pub files: Vec<PathBuf>,
}

impl ImportGroup {
    pub fn new(library: &str, files: &[impl AsRef<Path>]) -> ImportGroup {
        ImportGroup {
            library: library.to_string(),
            files: files.iter().map(|f| f.as_ref().to_path_buf()).collect(),
        }
    }
}

/// Options for [`Ghdl::import`].
pub struct ImportOptions {
    /// VHDL standard: "93", "08", ...
    pub std: String,
    /// Directory for the `.cf` library files.
    pub workdir: PathBuf,
    /// Extra library search paths (`-P`).
    pub library_paths: Vec<PathBuf>,
    /// Groups of files, one per library.
    pub groups: Vec<ImportGroup>,
}

/// Options for [`Ghdl::synth`].
pub struct SynthOptions {
    /// VHDL standard: "93", "08", ...
    pub std: String,
    /// Work directory (`--workdir`).
    pub workdir: Option<PathBuf>,
    /// Extra library search paths (`-P`).
    pub library_paths: Vec<PathBuf>,
    /// Files to analyze on the fly (one-shot mode, first synth() only).
    pub files: Vec<ImportGroup>,
    /// Top unit, as `library.entity` or just `entity`.
    pub top: String,
    /// Infer latches (`--latches`).
    pub latches: bool,
    /// `--keep-hierarchy=yes|no` (default is yes).
    pub keep_hierarchy: Option<String>,
    /// `--top-name=asis|hash` for the top module name (default asis).
    pub top_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Netlist browsing
// ---------------------------------------------------------------------------

/// The result of a synthesis: a netlist whose root is the top module.
pub struct Design {
    top: Module,
}

impl Design {
    /// The top-level module (corresponds to the top entity).
    pub fn top(&self) -> Module {
        self.top
    }

    /// The module of the top entity itself: the design root returned by
    /// the synthesis is a container module (named `top`) whose first
    /// user sub-module is the top entity.
    pub fn top_entity(&self) -> Module {
        self.top
            .sub_modules()
            .into_iter()
            .find(|m| m.is_user_module())
            .unwrap_or(self.top)
    }

    /// All user modules (entity/architecture modules) reachable from the
    /// top (breadth first; a module instantiated several times appears
    /// only once).  Netlist gate modules are filtered out.
    pub fn modules(&self) -> Vec<Module> {
        let mut res = Vec::new();
        let mut seen = Vec::new();
        let mut queue = vec![self.top];
        while let Some(m) = queue.pop() {
            for sm in m.sub_modules() {
                if !seen.contains(&sm.0) {
                    seen.push(sm.0);
                    if sm.is_user_module() {
                        res.push(sm);
                    }
                    queue.push(sm);
                }
            }
        }
        res
    }
}

/// A module of the netlist: the synthesized form of an entity/architecture
/// pair (one module per specialization by generic values).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Module(pub u32);

impl Module {
    /// Numeric module id.  This is a *class* id, not a per-module identity:
    /// all entity modules share `Id_User_None` (128), blackboxes with
    /// parameters use 129, system gate modules (and, dff, mux, ...) use
    /// reserved ids < 128, and the design container uses 2.
    pub fn id(&self) -> u32 {
        unsafe { ffi::netlists__get_id(self.0) }
    }

    /// True if this is a user module (an entity/architecture, possibly a
    /// blackbox), as opposed to a system gate module.
    pub fn is_user_module(&self) -> bool {
        //  Entity modules get Id_User_None (128); blackboxes with
        //  parameters get Id_User_Parameters (129).  Gate modules use
        //  reserved ids < 128, and the design container uses Id_Design (2).
        self.id() >= 128
    }

    /// The self-instance of the module (represents the module ports from
    /// the inside; its inputs are the module outputs and vice versa).
    /// Useful to check if a net is directly a module port.
    pub fn self_instance(&self) -> Instance {
        Instance(unsafe { ffi::netlists__get_self_instance(self.0) })
    }

    /// Raw netlist name, e.g. `counter_Brtl_Lmylib_8_<sha1>`.
    pub fn name(&self) -> String {
        unsafe { sname_to_string(ffi::netlists__get_module_name(self.0)) }
    }

    /// Best-effort decomposition of the raw name into
    /// entity / architecture / library / generic values / hash.
    pub fn parse_name(&self) -> ParsedModuleName {
        parse_module_name(&self.name())
    }

    /// Input ports (never `inout`: inout ports are reported among the
    /// outputs, see [`Module::outputs`]).
    pub fn inputs(&self) -> Vec<Port> {
        let n = unsafe { ffi::netlists__get_nbr_inputs(self.0) };
        (0..n)
            .map(|i| Port {
                name: unsafe { sname_to_string(ffi::netlists__utils__get_input_name(self.0, i)) },
                width: unsafe { ffi::netlists__utils__get_input_width(self.0, i) },
                dir: PortDir::In,
            })
            .collect()
    }

    /// Output ports.  `inout` ports are reported here (with
    /// [`PortDir::Inout`]).
    pub fn outputs(&self) -> Vec<Port> {
        let n = unsafe { ffi::netlists__get_nbr_outputs(self.0) };
        (0..n)
            .map(|i| Port {
                name: unsafe { sname_to_string(ffi::netlists__utils__get_output_name(self.0, i)) },
                width: unsafe { ffi::netlists__utils__get_output_width(self.0, i) },
                dir: if unsafe { ffi::netlists__utils__get_inout_flag(self.0, i) & 1 != 0 } {
                    PortDir::Inout
                } else {
                    PortDir::Out
                },
            })
            .collect()
    }

    /// Generic parameters (names and kinds; values are per-instance).
    pub fn params(&self) -> Vec<Param> {
        let n = unsafe { ffi::netlists__get_nbr_params(self.0) };
        (0..n)
            .map(|i| Param {
                name: unsafe { sname_to_string(ffi::netlists__utils__get_param_name(self.0, i)) },
                kind: unsafe { ffi::netlists__utils__get_param_type(self.0, i) },
            })
            .collect()
    }

    /// User-declared signals of this module, as [`Port`] values with
    /// [`PortDir::Signal`].
    ///
    /// In the netlist, a declared signal is represented by an instance of
    /// the internal `signal` (with driver) or `isignal` (initial value
    /// only) gate, named after the signal declaration; the signal net is
    /// the output 0 of that instance, which gives the width.
    ///
    /// Notes:
    /// - signals that are optimized away (unused) do not appear;
    /// - GHDL-generated port-fitting signals use the `wrap_` name prefix;
    /// - record signals are decomposed per element (the element name
    ///   appears in brackets, e.g. `rec[elem]`).
    pub fn signals(&self) -> Vec<Port> {
        //  Id_Signal / Id_Isignal from netlists-gates.ads.
        const ID_SIGNAL: u32 = 52;
        const ID_ISIGNAL: u32 = 53;
        let mut res = Vec::new();
        for inst in self.instances() {
            let id = inst.module().id();
            if id != ID_SIGNAL && id != ID_ISIGNAL {
                continue;
            }
            let name = inst.name();
            if name.is_empty() || name.starts_with('*') {
                continue;
            }
            let width = inst.output_net(0).map(|n| n.width()).unwrap_or(0);
            res.push(Port {
                name,
                width,
                dir: PortDir::Signal,
            });
        }
        res
    }

    /// Direct sub-modules (the entities instantiated by this module).
    pub fn sub_modules(&self) -> Vec<Module> {
        let mut res = Vec::new();
        let mut m = unsafe { ffi::netlists__get_first_sub_module(self.0) };
        while m != 0 {
            res.push(Module(m));
            m = unsafe { ffi::netlists__get_next_sub_module(m) };
        }
        res
    }

    /// All instances in this module: gates, process outputs and sub-module
    /// instantiations. User signal names usually appear as instance names
    /// (of the instance driving the signal).
    pub fn instances(&self) -> Vec<Instance> {
        let mut res = Vec::new();
        let mut i = unsafe { ffi::netlists__get_first_instance(self.0) };
        while i != 0 {
            res.push(Instance(i));
            i = unsafe { ffi::netlists__get_next_instance(i) };
        }
        res
    }
}

/// Direction/kind of a [`Port`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDir {
    /// Input port.
    In,
    /// Output port.
    Out,
    /// `inout` port (reported among the outputs).
    Inout,
    /// Internal declared signal (not a port).
    Signal,
}

/// A named connection of a module: either a port ([`PortDir::In`],
/// [`PortDir::Out`] or [`PortDir::Inout`]) or an internal declared signal
/// ([`PortDir::Signal`]).
#[derive(Clone, Debug)]
pub struct Port {
    pub name: String,
    /// Width in bits.
    pub width: u32,
    pub dir: PortDir,
}

impl Port {
    /// True for `inout` ports.
    pub fn is_inout(&self) -> bool {
        self.dir == PortDir::Inout
    }

    /// True if this is a module port (input, output or inout).
    pub fn is_port(&self) -> bool {
        self.dir != PortDir::Signal
    }

    /// True if this is an internal declared signal.
    pub fn is_signal(&self) -> bool {
        self.dir == PortDir::Signal
    }
}

/// A generic parameter of a module.
#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    /// `Param_Type` value from `netlists.ads` (0 = invalid, 1 = Uns32, ...).
    pub kind: u32,
}

/// An instance in a module: either an instantiated sub-module (an entity
/// instantiation) or a netlist gate (logic operation, flip-flop, ...).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Instance(pub u32);

impl Instance {
    /// Name of the instance (the VHDL instantiation label, or a generated
    /// name for gates; user signal names often appear here).
    pub fn name(&self) -> String {
        unsafe { sname_to_string(ffi::netlists__get_instance_name(self.0)) }
    }

    /// The module instantiated by this instance.
    pub fn module(&self) -> Module {
        Module(unsafe { ffi::netlists__get_module(self.0) })
    }

    /// Value of an unsigned 32-bit generic, for parameters of that kind.
    pub fn param_uns32(&self, idx: u32) -> u32 {
        unsafe { ffi::netlists__get_param_uns32(self.0, idx) }
    }

    /// Net connected to input port `idx`.
    pub fn input_net(&self, idx: u32) -> Option<Net> {
        let n = unsafe { ffi::netlists__get_driver(ffi::netlists__get_input(self.0, idx)) };
        (n != 0).then_some(Net(n))
    }

    /// Net driven by output port `idx`.
    pub fn output_net(&self, idx: u32) -> Option<Net> {
        let n = unsafe { ffi::netlists__get_output(self.0, idx) };
        (n != 0).then_some(Net(n))
    }
}

/// A net (a signal wire) in the netlist.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Net(pub u32);

impl Net {
    /// Width in bits.
    pub fn width(&self) -> u32 {
        unsafe { ffi::netlists__get_width(self.0) }
    }

    /// The instance driving this net (for a module port, this is the module
    /// self-instance).
    pub fn parent_instance(&self) -> Instance {
        Instance(unsafe { ffi::netlists__get_net_parent(self.0) })
    }

    /// Name of the driving instance, if meaningful (often the user signal
    /// name for simple assignments).
    pub fn name(&self) -> Option<String> {
        let name = self.parent_instance().name();
        (!name.is_empty() && !name.starts_with('*')).then_some(name)
    }

    /// The instances whose inputs are connected to this net.
    pub fn sinks(&self) -> Vec<Instance> {
        let mut res = Vec::new();
        let mut i = unsafe { ffi::netlists__get_first_sink(self.0) };
        while i != 0 {
            res.push(Instance(unsafe { ffi::netlists__get_input_parent(i) }));
            i = unsafe { ffi::netlists__get_next_sink(i) };
        }
        res
    }
}

// ---------------------------------------------------------------------------
// Module name parsing
// ---------------------------------------------------------------------------

/// Best-effort decomposition of a netlist module name, see [`Module::parse_name`].
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedModuleName {
    pub entity: String,
    pub architecture: Option<String>,
    pub library: Option<String>,
    /// Inlined generic values (decimal), in declaration order.
    pub generic_values: Vec<String>,
    /// SHA-1 hex digest (40 chars), present when some generic values or port
    /// bounds could not be inlined in the name.
    pub hash: Option<String>,
}

/// Heuristic parser for the `<entity>_B<arch>[_L<lib>][<generics>][_<hash>]`
/// module names. Splits from the right: the hash (40 hex chars) and the
/// generic values (decimal digits) are stripped first, then `_L<lib>` and
/// `_B<arch>` (last occurrence wins). The result is ambiguous when the
/// entity or architecture name itself contains `_B`/`_L` in a plausible
/// position.
pub fn parse_module_name(raw: &str) -> ParsedModuleName {
    let mut s = raw.to_string();
    let mut hash = None;
    let mut generic_values = Vec::new();

    //  1. Trailing SHA-1 digest: '_' + 40 lowercase hex characters.
    if s.len() > 41 {
        let tail = &s[s.len() - 40..];
        if s.as_bytes()[s.len() - 41] == b'_'
            && tail
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            hash = Some(tail.to_string());
            s.truncate(s.len() - 41);
        }
    }

    //  2. Trailing inlined generic values: groups of '_' + decimal digits.
    while let Some(pos) = s.rfind('_') {
        let tail = &s[pos + 1..];
        if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
            generic_values.push(tail.to_string());
            s.truncate(pos);
        } else {
            break;
        }
    }
    generic_values.reverse();

    //  3. Library suffix: last '_L'.
    let mut library = None;
    if let Some(pos) = s.rfind("_L") {
        if pos > 0 {
            library = Some(s[pos + 2..].to_string());
            s.truncate(pos);
        }
    }

    //  4. Architecture suffix: last '_B'.
    let (entity, architecture) = match s.rfind("_B") {
        Some(pos) if pos > 0 => (s[..pos].to_string(), Some(s[pos + 2..].to_string())),
        _ => (s.clone(), None),
    };

    ParsedModuleName {
        entity,
        architecture,
        library,
        generic_values,
        hash,
    }
}
