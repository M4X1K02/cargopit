use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const SIMAPI_REL: &str = "../../src/cargopit/simulatorapi/simapi";
const PROC2_OLD_PID_CUTOFF: (u64, u64, u64) = (4, 0, 5);

const MAPPER_SOURCES: &[&str] = &[
    "simapi/simmapper.c",
    "simmap/mapacdata.c",
    "simapi/mapping/acmapper.c",
    "simapi/mapping/rf2mapper.c",
    "simapi/mapping/r3emapper.c",
    "simapi/mapping/pcars2mapper.c",
    "simapi/mapping/dirt2mapper.c",
    "simapi/mapping/scs2mapper.c",
    "simapi/mapping/outgaugemapper.c",
    "simapi/mapping/f12018mapper.c",
    "simapi/mapping/rbrmapper.c",
    "simapi/mapping/wreckfest2mapper.c",
    "simapi/mapping/forzamapper.c",
    "simapi/getpid.c",
];

struct ProcLibs {
    include_flags: Vec<String>,
    link_libs: Vec<String>,
    old_pid_val: bool,
}

fn simapi_root() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).join(SIMAPI_REL)
}

fn pkg_config(args: &[&str]) -> Option<String> {
    let output = Command::new("pkg-config").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

fn version_is_numeric(version: &str) -> bool {
    version.chars().next().is_some_and(|ch| ch.is_ascii_digit())
}

fn version_older_than(version: &str, cutoff: (u64, u64, u64)) -> bool {
    if !version_is_numeric(version) {
        return false;
    }
    let mut parts = version.split(|ch: char| !ch.is_ascii_digit());
    let major = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    (major, minor, patch) < cutoff
}

fn proc_libs() -> ProcLibs {
    if let Some(version) = pkg_config(&["--modversion", "libproc2"]) {
        let cflags = pkg_config(&["--cflags", "libproc2"]).unwrap_or_default();
        let libs = pkg_config(&["--libs", "libproc2"]).unwrap_or_default();
        return ProcLibs {
            include_flags: split_flags(&cflags),
            link_libs: split_flags(&libs),
            old_pid_val: version_older_than(&version, PROC2_OLD_PID_CUTOFF),
        };
    }
    if pkg_config(&["--exists", "libprocps"]).is_some()
        || Command::new("pkg-config")
            .args(["--exists", "libprocps"])
            .status()
            .is_ok_and(|status| status.success())
    {
        let cflags = pkg_config(&["--cflags", "libprocps"]).unwrap_or_default();
        let libs = pkg_config(&["--libs", "libprocps"]).unwrap_or_default();
        return ProcLibs {
            include_flags: split_flags(&cflags),
            link_libs: split_flags(&libs),
            old_pid_val: true,
        };
    }
    panic!("Either libprocps or libproc2 is required");
}

fn split_flags(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|flag| !flag.is_empty())
        .map(str::to_string)
        .collect()
}

fn emit_layout(include: &Path, out_dir: &Path) {
    println!("cargo:rerun-if-changed=csrc/layout_check.c");
    println!("cargo:rerun-if-changed=csrc/layout_dump.c");
    cc::Build::new()
        .file("csrc/layout_check.c")
        .include(include)
        .compile("simapi_layout");

    let dump_bin = out_dir.join("layout_dump");
    let status = cc::Build::new()
        .file("csrc/layout_dump.c")
        .include(include)
        .get_compiler()
        .to_command()
        .arg("-I")
        .arg(include)
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
    std::fs::write(out_dir.join("layout.rs"), &output.stdout).expect("write layout.rs");
}

fn compile_mappers(root: &Path, include: &Path, proc: &ProcLibs) {
    let mut build = cc::Build::new();
    build
        .include(include)
        .include(root.join("include"))
        .include(root.join("simmap"))
        .flag("-fPIC");
    if proc.old_pid_val {
        build.define("USE_OLD_PID_VAL", "1");
    }
    for flag in &proc.include_flags {
        if let Some(dir) = flag.strip_prefix("-I") {
            build.include(dir);
        }
    }
    for source in MAPPER_SOURCES {
        let path = root.join(source);
        println!("cargo:rerun-if-changed={}", path.display());
        build.file(path);
    }
    build.compile("simapi_mappers");

    println!("cargo:rustc-link-lib=m");
    for flag in &proc.link_libs {
        if let Some(lib) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
}

fn generate_bindings(include: &Path, root: &Path, out_dir: &Path) {
    println!("cargo:rerun-if-changed=csrc/wrapper.h");
    let builder = bindgen::Builder::default()
        .header("csrc/wrapper.h")
        .clang_arg(format!("-I{}", include.display()))
        .clang_arg(format!("-I{}", root.join("include").display()))
        .clang_arg(format!("-I{}", root.join("simmap").display()))
        .allowlist_function("simapi_.*")
        .allowlist_function("map_.*")
        .allowlist_type("SimData")
        .allowlist_type("SimMap")
        .allowlist_type("SimInfo")
        .allowlist_type("SimCompatMap")
        .allowlist_var("SIMAPI_.*")
        .allowlist_var("SIMULATOREXE_.*")
        .derive_default(true);
    let bindings = builder.generate().expect("bindgen simapi headers");
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("write bindings.rs");
}

fn main() {
    let root = simapi_root();
    let include = root.join("simapi");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    println!("cargo:rerun-if-changed={}", include.display());
    emit_layout(&include, &out_dir);
    let proc = proc_libs();
    compile_mappers(&root, &include, &proc);
    generate_bindings(&include, &root, &out_dir);
}
