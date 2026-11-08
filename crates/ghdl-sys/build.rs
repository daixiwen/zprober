//  build.rs for ghdl-sys
//
//  Builds the GHDL shared library (libghdl) from the `ghdl` git submodule
//  and links it into the Rust crate.
//
//  The build only creates files that are already gitignored in the GHDL
//  repository (config.status, default_paths.ads, lib/, pic/, ...), so the
//  submodule stays pristine.
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run(cmd: &mut Command) {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn {cmd:?}: {e}"));
    if !status.success() {
        panic!("command failed: {cmd:?}");
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ghdl_dir = manifest_dir.join("ghdl");
    assert!(
        ghdl_dir.join("configure").exists(),
        "GHDL submodule not found in {} - did you `git submodule update --init` ?",
        ghdl_dir.display()
    );

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ghdl/src");
    println!("cargo:rerun-if-changed=ghdl/libraries");
    println!("cargo:rerun-if-changed=ghdl/configure");

    //  1. Configure GHDL with the libghdl feature (only once).
    //     The default backend (mcode) is used, which is enough for the
    //     analyzer and the synthesizer.
    if !ghdl_dir.join("config.status").exists() {
        run(Command::new("./configure")
            .arg("--enable-libghdl")
            .current_dir(&ghdl_dir));
    }

    //  2. Build the shared library and the VHDL standard libraries.
    //     Both targets are incremental, so this is fast after the first run.
    run(Command::new("make")
        .arg("all.libghdl")
        .current_dir(&ghdl_dir));
    run(Command::new("make").arg("libs").current_dir(&ghdl_dir));

    let lib_dir = ghdl_dir.join("lib");

    if cfg!(target_os = "windows") {
        //  Windows: gnatmake appends '.exe', the Makefile renames the result
        //  to 'libghdl-<version>.dll'.
        let mut dll_name = None;
        for entry in fs::read_dir(&lib_dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            //  libghdl-<version>.dll  =>  link name ghdl-<version>
            if let Some(stem) = name
                .strip_prefix("lib")
                .and_then(|s| s.strip_suffix(".dll"))
            {
                dll_name = Some(stem.to_string());
            }
        }
        let dll_name = dll_name.expect("libghdl-*.dll not found; did 'make all.libghdl' succeed?");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=dylib={dll_name}");
        //  No rpath on Windows: the DLL directory is exported through an
        //  environment variable, and ghdl-sys prepends it to PATH at runtime
        //  (see lib.rs).
    } else {
        //  Linux/macOS: find the versioned shared library.
        let ext = if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };
        let mut lib_name = None;
        for entry in fs::read_dir(&lib_dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            //  libghdl-<version>.so  =>  link name ghdl-<version>
            if let Some(stem) = name
                .strip_prefix("lib")
                .and_then(|s| s.strip_suffix(&format!(".{ext}")))
            {
                if stem.starts_with("ghdl-") {
                    lib_name = Some(stem.to_string());
                }
            }
        }
        let lib_name = lib_name.unwrap_or_else(|| {
            panic!(
                "libghdl-*.{} not found in {}; did 'make all.libghdl' succeed?",
                ext,
                lib_dir.display()
            )
        });
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=dylib={lib_name}");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    }

    //  Give the runtime code the location of the VHDL standard libraries
    //  (built by 'make libs' inside the submodule).
    println!("cargo:rustc-env=GHDL_SYS_PREFIX={}", ghdl_dir.display());
}
