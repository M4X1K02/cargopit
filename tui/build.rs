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
}
