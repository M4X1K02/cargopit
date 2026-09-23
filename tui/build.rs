use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let version_file = manifest.join("..").join("version.txt");
    let version = fs::read_to_string(&version_file)
        .expect("the repository version.txt file is required")
        .trim()
        .to_owned();
    if version.is_empty() {
        panic!("the repository version.txt file must not be empty");
    }
    println!("cargo:rerun-if-changed={}", version_file.display());
    println!("cargo:rustc-env=CARGOPIT_VERSION={version}");

    let header_dir = manifest
        .join("..")
        .join("src")
        .join("cargopit")
        .join("simulatorapi")
        .join("simapi")
        .join("simapi");
    let header = header_dir.join("simdata.h");
    let mut build = cc::Build::new();
    build.include(manifest.join("native"));
    if header.is_file() {
        build.file("native/simapi_view.c");
        build.include(&header_dir);
        println!("cargo:rerun-if-changed={}", header.display());
        println!(
            "cargo:rerun-if-changed={}",
            header_dir.join("simapi.h").display()
        );
    } else {
        build.file("native/simapi_view_stub.c");
    }
    println!("cargo:rerun-if-changed=native/simapi_view.c");
    println!("cargo:rerun-if-changed=native/simapi_view_stub.c");
    println!("cargo:rerun-if-changed=native/simapi_view.h");
    build.compile("cargopit_simapi_view");
}
