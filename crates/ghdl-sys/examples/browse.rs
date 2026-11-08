//! Browse a synthesized design using ghdl-sys.
//!
//! Replicates the flow of `example/.tmp/compile.sh`:
//!   1. import VHDL files into libraries (ghdl -i)
//!   2. synthesize the top-level (ghdl --synth)
//!   3. browse the resulting netlist: modules (entities), instantiations,
//!      ports and signal names.
//!
//! By default uses the small `example/multilib` project. Pass `full` as the
//! first argument to use the `example/.tmp/compile.sh` project
//! (neorv32 + fpgacapzero + fpgan32), which takes significantly longer.
use std::path::{Path, PathBuf};

use ghdl_sys::{Ghdl, ImportGroup, ImportOptions, Module, SynthOptions};

fn print_module(m: Module, indent: usize, seen: &mut Vec<u32>) {
    let pad = "  ".repeat(indent);
    let raw = m.name();
    let parsed = m.parse_name();
    println!(
        "{pad}module {} [entity: {}, arch: {}, lib: {}]",
        raw,
        parsed.entity,
        parsed.architecture.as_deref().unwrap_or("-"),
        parsed.library.as_deref().unwrap_or("-")
    );

    for p in m.inputs() {
        println!(
            "{pad}  port in : {} [{}{}]",
            p.name,
            p.width,
            if p.inout { ", inout" } else { "" }
        );
    }
    for p in m.outputs() {
        println!(
            "{pad}  port out: {} [{}{}]",
            p.name,
            p.width,
            if p.inout { ", inout" } else { "" }
        );
    }
    for p in m.params() {
        println!("{pad}  generic : {} [kind {}]", p.name, p.kind);
    }

    for s in m.signals() {
        println!("{pad}  signal  : {} [{}]", s.name, s.width);
    }

    //  Instances: entity instantiations and netlist gates.  User signal
    //  names appear as instance names.  Only entity instantiations are
    //  followed recursively (gate modules are system modules).
    let insts = m.instances();
    println!("{pad}  {} instance(s)", insts.len());
    for i in &insts {
        let sub = i.module();
        let is_entity = sub.is_user_module();
        println!(
            "{pad}  instance '{}' -> {}{}",
            i.name(),
            sub.name(),
            if is_entity { " [entity]" } else { "" }
        );
        if is_entity && !seen.contains(&sub.0) && sub.0 != m.0 {
            seen.push(sub.0);
            print_module(sub, indent + 2, seen);
        }
    }
}

fn run(base: &PathBuf, libs: Vec<(&str, Vec<PathBuf>)>, top: &str) {
    let work = base.join(".work");
    std::fs::create_dir_all(&work).unwrap();

    //  1. Initialize the library.
    let ghdl = Ghdl::init(None).expect("ghdl init");

    //  2. Import the files into their libraries (ghdl -i).
    let work_opts = |groups: Vec<ImportGroup>| ImportOptions {
        std: "08".to_string(),
        workdir: work.clone(),
        library_paths: vec![work.clone()],
        groups,
    };
    for (lib, files) in &libs {
        ghdl.import(&work_opts(vec![ImportGroup::new(lib, files)]))
            .expect("import failed");
    }

    //  3. Compile (analyze) the imported units (ghdl -m).
    ghdl.compile().expect("compilation failed");

    //  4. Synthesize the top level (ghdl --synth).
    let design = ghdl
        .synth(&SynthOptions {
            std: "08".to_string(),
            workdir: Some(work.clone()),
            library_paths: vec![work.clone()],
            files: vec![],
            top: top.to_string(),
            latches: true,
            keep_hierarchy: None,
            top_name: None,
        })
        .expect("synthesis failed");

    //  4. Browse the netlist.
    let top_entity = design.top_entity();
    println!(
        "\n=== design hierarchy (top entity: {}) ===",
        top_entity.name()
    );
    let mut seen = vec![top_entity.0];
    print_module(top_entity, 0, &mut seen);

    println!(
        "\n=== all {} distinct entity modules ===",
        design.modules().len()
    );
    for m in design.modules() {
        println!("  {}", m.name());
    }
}

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example = manifest.join("../../example");

    if std::env::args().nth(1).as_deref() == Some("full") {
        //  The example/.tmp/compile.sh project.
        run(
            &example.join(".tmp"),
            vec![
                (
                    "neorv32",
                    glob_files(&example.join("neorv32/rtl/core/*.vhd")),
                ),
                (
                    "fpgacapzero",
                    [
                        glob_files(&example.join("fpgacapzero/rtl/vhdl/pkg/*.vhd")),
                        vec![example.join("fpgacapzero/rtl/vhdl/fcapz_ela_polarfire.vhd")],
                        vec![example.join("fpgacapzero/rtl/vhdl/jtag_tap/jtag_tap_polarfire.vhd")],
                        glob_files(&example.join("fpgacapzero/rtl/vhdl/core/*.vhd")),
                    ]
                    .concat(),
                ),
                ("fpgan32", vec![example.join("fpgan32/toplevel.vhd")]),
            ],
            "fpgan32.toplevel",
        );
    } else {
        //  The small multilib project.
        let ml = example.join("multilib");
        run(
            &ml,
            vec![
                ("lib2", vec![ml.join("lib2/my_module.vhd")]),
                (
                    "lib1",
                    vec![
                        ml.join("lib1/a_submodule.vhd"),
                        ml.join("lib1/my_module.vhd"),
                    ],
                ),
            ],
            "lib1.my_module",
        );
    }
}

fn glob_files(pattern: &Path) -> Vec<PathBuf> {
    //  Minimal glob without an external dependency: only '*' in the file
    //  name part is supported, which is enough for the example.
    let pat = pattern.to_string_lossy().into_owned();
    let (dir, name_pat) = match pat.rsplit_once('/') {
        Some((d, n)) => (PathBuf::from(d), n.to_string()),
        None => (PathBuf::from("."), pat.clone()),
    };
    let mut res = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().into_string().unwrap();
            let prefix = name_pat.split('*').next().unwrap_or("");
            if name.starts_with(prefix) && name.ends_with(".vhd") {
                res.push(e.path());
            }
        }
    }
    res.sort();
    res
}
