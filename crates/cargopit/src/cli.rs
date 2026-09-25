//! CLI contracts shared with the C host: subcommands, flags, and version text.

pub const PROGRAM_NAME: &str = "cargopit";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUG_URL: &str = "github.com/M4X1K02/cargopit";
const FPS_DEFAULT: i32 = 60;
const CONFIG_INDEX_UNSET: i32 = -1;
const DEVICE_INDEX_ALL: i32 = -1;
const VERBOSITY_MAX: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramAction {
    Play,
    Test,
    ConfigTach,
    Exit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub action: ProgramAction,
    pub fps: i32,
    pub verbosity: u8,
    pub force_udp: bool,
    pub disable_audio: bool,
    pub config_file: Option<String>,
    pub config_dir: Option<String>,
    pub config_index: i32,
    pub device_index: i32,
    pub max_revs: u32,
    pub granularity: u32,
    pub save_file: Option<String>,
    pub log_file: Option<String>,
    pub version_line: Option<String>,
    pub usage: bool,
}

impl Invocation {
    fn defaults() -> Self {
        Self {
            action: ProgramAction::Exit,
            fps: FPS_DEFAULT,
            verbosity: 0,
            force_udp: false,
            disable_audio: false,
            config_file: None,
            config_dir: None,
            config_index: CONFIG_INDEX_UNSET,
            device_index: DEVICE_INDEX_ALL,
            max_revs: 0,
            granularity: 1,
            save_file: None,
            log_file: None,
            version_line: None,
            usage: false,
        }
    }
}

pub fn version_line() -> String {
    format!("{PROGRAM_NAME} {VERSION}")
}

pub fn parse(args: &[String]) -> Invocation {
    let mut parsed = Invocation::defaults();
    if args.iter().any(|arg| arg == "--version") {
        parsed.version_line = Some(version_line());
        parsed.action = ProgramAction::Exit;
        return parsed;
    }
    if args.is_empty() || args.iter().any(|arg| arg == "--help") {
        parsed.usage = true;
        parsed.action = ProgramAction::Exit;
        return parsed;
    }
    match args[0].to_ascii_lowercase().as_str() {
        "play" => parse_play(&mut parsed, &args[1..]),
        "test" => parse_test(&mut parsed, &args[1..]),
        "config" => parse_config(&mut parsed, &args[1..]),
        _ => {
            parsed.usage = true;
        }
    }
    parsed
}

fn parse_play(parsed: &mut Invocation, args: &[String]) {
    parsed.action = ProgramAction::Play;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-d" || arg == "--udp" {
            parsed.force_udp = true;
        } else if arg == "-a" || arg == "--disable_audio" {
            parsed.disable_audio = true;
        } else if arg == "-v" || arg == "--verbose" {
            bump_verbosity(parsed);
        } else if arg == "-vv" {
            bump_verbosity(parsed);
            bump_verbosity(parsed);
        } else if let Some(value) = take_value(args, &mut index, arg, &["-l", "--log"]) {
            parsed.log_file = Some(value);
        } else if let Some(value) = take_value(args, &mut index, arg, &["-c", "--config-file"]) {
            parsed.config_file = Some(value);
        } else if let Some(value) = take_value(args, &mut index, arg, &["--config-dir"]) {
            parsed.config_dir = Some(value);
        } else if let Some(value) = take_value(args, &mut index, arg, &["-f", "--fps"]) {
            parsed.fps = crate::scheduler::clamp_fps(value.parse().unwrap_or(FPS_DEFAULT)) as i32;
        } else if let Some(value) = take_value(args, &mut index, arg, &["--config-index"]) {
            parsed.config_index = value.parse().unwrap_or(CONFIG_INDEX_UNSET);
        } else {
            parsed.usage = true;
            parsed.action = ProgramAction::Exit;
            return;
        }
        index += 1;
    }
}

fn parse_test(parsed: &mut Invocation, args: &[String]) {
    parsed.action = ProgramAction::Test;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-a" || arg == "--disable_audio" {
            parsed.disable_audio = true;
        } else if arg == "-v" || arg == "--verbose" {
            bump_verbosity(parsed);
        } else if arg == "-vv" {
            bump_verbosity(parsed);
            bump_verbosity(parsed);
        } else if let Some(value) = take_value(args, &mut index, arg, &["-c", "--config-file"]) {
            parsed.config_file = Some(value);
        } else if let Some(value) = take_value(args, &mut index, arg, &["--device-index"]) {
            parsed.device_index = value.parse().unwrap_or(DEVICE_INDEX_ALL);
        } else if let Some(value) = take_value(args, &mut index, arg, &["--config-index"]) {
            parsed.config_index = value.parse().unwrap_or(CONFIG_INDEX_UNSET);
        } else {
            parsed.usage = true;
            parsed.action = ProgramAction::Exit;
            return;
        }
        index += 1;
    }
}

fn parse_config(parsed: &mut Invocation, args: &[String]) {
    if args
        .first()
        .map(|arg| arg.eq_ignore_ascii_case("tachometer"))
        != Some(true)
    {
        parsed.usage = true;
        return;
    }
    parsed.action = ProgramAction::ConfigTach;
    let mut index = 1;
    let mut saw_revs = false;
    let mut saw_save = false;
    while index < args.len() {
        let arg = &args[index];
        if arg == "-v" || arg == "--verbose" {
            bump_verbosity(parsed);
        } else if arg == "-vv" {
            bump_verbosity(parsed);
            bump_verbosity(parsed);
        } else if let Some(value) = take_value(args, &mut index, arg, &["-m", "--max_revs"]) {
            parsed.max_revs = value.parse().unwrap_or(0);
            saw_revs = true;
        } else if let Some(value) = take_value(args, &mut index, arg, &["-g", "--granularity"]) {
            parsed.granularity = accepted_granularity(value.parse().unwrap_or(1));
        } else if let Some(value) = take_value(args, &mut index, arg, &["-s", "--savefile"]) {
            parsed.save_file = Some(value);
            saw_save = true;
        } else {
            parsed.usage = true;
            parsed.action = ProgramAction::Exit;
            return;
        }
        index += 1;
    }
    if !saw_revs || !saw_save {
        parsed.usage = true;
        parsed.action = ProgramAction::Exit;
    }
}

fn take_value(args: &[String], index: &mut usize, arg: &str, names: &[&str]) -> Option<String> {
    if !names.contains(&arg) {
        return None;
    }
    let value = args.get(*index + 1)?.clone();
    *index += 1;
    Some(value)
}

fn bump_verbosity(parsed: &mut Invocation) {
    parsed.verbosity = parsed.verbosity.saturating_add(1).min(VERBOSITY_MAX);
}

fn accepted_granularity(value: u32) -> u32 {
    if (1..=4).contains(&value) && value != 3 {
        return value;
    }
    1
}

pub fn usage_text() -> String {
    format!(
        "Usage: {PROGRAM_NAME}\n\
         Usage 1: {PROGRAM_NAME} play\n\
         Usage 2: {PROGRAM_NAME} config tachometer\n\
         Usage 3: {PROGRAM_NAME} test\n\
         \n\
         Report bugs on {BUG_URL}.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| (*part).to_string()).collect()
    }

    #[test]
    fn version_and_help_exit_without_running() {
        let version = parse(&args(&["--version"]));
        assert_eq!(version.action, ProgramAction::Exit);
        assert_eq!(version.version_line.as_deref(), Some("cargopit 0.4.0"));
        let help = parse(&args(&["--help"]));
        assert!(help.usage);
        assert!(parse(&[]).usage);
    }

    #[test]
    fn play_and_test_flags_match_the_c_defaults() {
        let play = parse(&args(&[
            "play",
            "--disable_audio",
            "--udp",
            "--fps",
            "144",
            "-v",
            "-v",
        ]));
        assert_eq!(play.action, ProgramAction::Play);
        assert!(play.disable_audio);
        assert!(play.force_udp);
        assert_eq!(play.fps, 144);
        assert_eq!(parse(&args(&["play", "-vv"])).verbosity, 2);
        assert_eq!(parse(&args(&["play", "--fps", "0"])).fps, 1);
        assert_eq!(
            parse(&args(&["play", "--log", "/tmp/cargopit.log"]))
                .log_file
                .as_deref(),
            Some("/tmp/cargopit.log")
        );
        assert_eq!(play.verbosity, 2);
        assert_eq!(play.config_index, CONFIG_INDEX_UNSET);
        let test = parse(&args(&["test", "--device-index", "1"]));
        assert_eq!(test.action, ProgramAction::Test);
        assert_eq!(test.device_index, 1);
    }
}
