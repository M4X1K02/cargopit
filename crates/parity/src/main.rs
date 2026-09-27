use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use parity::scenarios::{self, Scenario};
use parity::{golden_dir, write_goldens};

const CMD_DUMP: &str = "dump";
const CMD_REGEN: &str = "regen";
const USAGE: &str =
    "usage: parity-scenarios dump <directory>\n       parity-scenarios regen [directory]";
const EXIT_USAGE: u8 = 2;
const EXIT_FAIL: u8 = 1;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = args.next();
    match command.as_deref() {
        Some(CMD_DUMP) => dump_command(args.next()),
        Some(CMD_REGEN) => regen_command(args.next()),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn dump_command(dir: Option<String>) -> ExitCode {
    let Some(dir) = dir else {
        eprintln!("{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    };
    if let Err(err) = dump_all(Path::new(&dir)) {
        eprintln!("{err}");
        return ExitCode::from(EXIT_FAIL);
    }
    ExitCode::SUCCESS
}

fn regen_command(dir: Option<String>) -> ExitCode {
    let path = match dir {
        Some(path) => PathBuf::from(path),
        None => golden_dir(),
    };
    match write_goldens(&path) {
        Ok(count) => {
            println!("regenerated {count} golden files in {}", path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(EXIT_FAIL)
        }
    }
}

fn dump_all(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    for scenario in scenarios::all() {
        let path = dir.join(format!("{}.simdata", scenario.name));
        fs::write(&path, encode(&scenario)).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn encode(scenario: &Scenario) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(scenarios::STREAM_MAGIC);
    let count = u32::try_from(scenario.frames.len()).expect("scenario frame count");
    bytes.extend_from_slice(&count.to_le_bytes());
    for frame in &scenario.frames {
        let frame_bytes = frame.as_bytes();
        if frame_bytes.len() != simapi_sys::SIMDATA_SIZE {
            panic!("scenario frame is not SimData sized");
        }
        bytes.extend_from_slice(frame_bytes);
    }
    bytes
}
