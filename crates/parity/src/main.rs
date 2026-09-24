use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use parity::scenarios::{self, Scenario};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = args.next();
    if command.as_deref() != Some("dump") {
        eprintln!("usage: parity-scenarios dump <directory>");
        return ExitCode::from(2);
    }
    let Some(dir) = args.next() else {
        eprintln!("usage: parity-scenarios dump <directory>");
        return ExitCode::from(2);
    };
    if let Err(err) = dump_all(Path::new(&dir)) {
        eprintln!("{err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
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
