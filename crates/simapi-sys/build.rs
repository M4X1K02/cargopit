use std::env;
use std::path::PathBuf;
use std::process::Command;

fn simapi_include() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("../../src/cargopit/simulatorapi/simapi/simapi")
}

fn main() {
    let include = simapi_include();
    println!("cargo:rerun-if-changed=csrc/layout_check.c");
    println!("cargo:rerun-if-changed=csrc/layout_dump.c");
    println!("cargo:rerun-if-changed={}", include.display());

    cc::Build::new()
        .file("csrc/layout_check.c")
        .include(&include)
        .compile("simapi_layout");

    let dump_bin = PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("layout_dump");
    let status = cc::Build::new()
        .file("csrc/layout_dump.c")
        .include(&include)
        .get_compiler()
        .to_command()
        .arg("-I")
        .arg(&include)
        .arg("csrc/layout_dump.c")
        .arg("-o")
        .arg(&dump_bin)
        .status()
        .expect("compile layout dump");
    if !status.success() {
        panic!("layout dump failed to compile");
    }

    let output = Command::new(&dump_bin).output().expect("run layout dump");
    if !output.status.success() {
        panic!("layout dump reported a SimData or SIMAPI_VERSION mismatch");
    }

    let dest = PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("layout.rs");
    std::fs::write(dest, &output.stdout).expect("write layout.rs");
}
