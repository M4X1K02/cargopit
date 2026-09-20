//! Observe SIMAPI.DAT through a persistent mmap and inotify, without polling.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use memmap2::Mmap;

use crate::consts;
use crate::simd_config::SimdConfig;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TelemetryView {
    pub mtick: u64,
    pub simexe: u64,
    pub simstatus: u32,
    pub velocity: u32,
    pub rpms: u32,
    pub simapi: u8,
    pub simon: u8,
    pub simapiversion: u8,
    pub valid: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningGame {
    pub name: String,
    pub game_id: u64,
    pub sending: bool,
    pub status_label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmEvent {
    Appeared,
    Disappeared,
}

extern "C" {
    fn cargopit_tui_simdata_size() -> usize;
    fn cargopit_tui_read_simdata(mem: *const u8, len: usize, out: *mut TelemetryView) -> i32;
    fn cargopit_tui_write_fixture(mem: *mut u8, len: usize, input: *const TelemetryView) -> i32;
}

pub fn simdata_size() -> usize {
    unsafe { cargopit_tui_simdata_size() }
}

pub fn read_simdata(bytes: &[u8]) -> Option<TelemetryView> {
    let mut view = TelemetryView::default();
    let rc = unsafe { cargopit_tui_read_simdata(bytes.as_ptr(), bytes.len(), &mut view) };
    if rc != 0 || view.valid == 0 {
        return None;
    }
    Some(view)
}

pub fn write_fixture(bytes: &mut [u8], view: &TelemetryView) -> bool {
    let rc = unsafe { cargopit_tui_write_fixture(bytes.as_mut_ptr(), bytes.len(), view) };
    rc == 0
}

pub fn telemetry_sending(view: &TelemetryView, mtick_changed: bool) -> bool {
    if view.valid == 0 || view.simon == 0 {
        return false;
    }
    if view.simstatus < consts::SIMAPI_STATUS_ACTIVEPLAY {
        return false;
    }
    mtick_changed
}

pub fn status_label(view: &TelemetryView, sending: bool) -> &'static str {
    if sending {
        return consts::LABEL_TELEMETRY_LIVE;
    }
    if view.valid == 0 {
        return consts::LABEL_NO_SIM;
    }
    if view.simstatus == consts::SIMAPI_STATUS_MENU {
        return consts::LABEL_TELEMETRY_MENU;
    }
    consts::LABEL_TELEMETRY_IDLE
}

pub fn simulator_api_label(simapi: u8) -> Option<&'static str> {
    consts::SIMULATOR_API_LABELS
        .iter()
        .find(|(id, _)| *id == simapi)
        .map(|(_, name)| *name)
}

pub fn resolve_games(
    simd: &SimdConfig,
    view: &TelemetryView,
    sending: bool,
    extra: &[RunningGame],
) -> Vec<RunningGame> {
    if view.valid == 0 {
        return extra.to_vec();
    }
    if view.simexe == 0 && view.simapi == 0 && view.simon == 0 {
        return extra.to_vec();
    }
    let mut games = Vec::new();
    games.push(RunningGame {
        name: display_name(simd, view),
        game_id: view.simexe,
        sending,
        status_label: status_label(view, sending),
    });
    for game in extra {
        if game.game_id != 0 && games.iter().any(|item| item.game_id == game.game_id) {
            continue;
        }
        if game.game_id == 0 && games.iter().any(|item| item.name == game.name) {
            continue;
        }
        games.push(game.clone());
    }
    games
}

fn display_name(simd: &SimdConfig, view: &TelemetryView) -> String {
    if let Some(name) = simd.name_for_game_id(view.simexe) {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    if let Some(label) = simulator_api_label(view.simapi) {
        return label.to_string();
    }
    if view.simexe != 0 {
        return format!("{} {}", consts::LABEL_SIM_ID, view.simexe);
    }
    consts::LABEL_NO_SIM.to_string()
}

pub struct SimApiSession {
    map: Option<Mmap>,
    events: Option<Receiver<ShmEvent>>,
}

impl SimApiSession {
    pub fn new() -> Self {
        let events = spawn_watcher();
        let mut session = Self { map: None, events };
        session.remap();
        session
    }

    pub fn drain(&mut self) -> bool {
        let (batch, disconnected) = {
            let Some(rx) = &self.events else {
                return false;
            };
            let mut batch = Vec::new();
            let mut disconnected = false;
            loop {
                match rx.try_recv() {
                    Ok(event) => batch.push(event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            (batch, disconnected)
        };
        if disconnected {
            self.events = None;
        }
        if batch.is_empty() {
            return false;
        }
        let mut appeared = false;
        let mut disappeared = false;
        for event in batch {
            match event {
                ShmEvent::Appeared => appeared = true,
                ShmEvent::Disappeared => disappeared = true,
            }
        }
        if disappeared {
            self.map = None;
        }
        if appeared {
            self.remap();
        }
        true
    }

    pub fn sample(&self) -> Option<TelemetryView> {
        let map = self.map.as_ref()?;
        read_simdata(map)
    }

    pub fn mapped(&self) -> bool {
        self.map.is_some()
    }

    fn remap(&mut self) {
        self.map = open_map(Path::new(consts::SIMAPI_DAT_PATH));
    }
}

fn open_map(path: &Path) -> Option<Mmap> {
    let file = File::open(path).ok()?;
    let map = unsafe { Mmap::map(&file).ok()? };
    if map.len() < simdata_size() {
        return None;
    }
    Some(map)
}

fn spawn_watcher() -> Option<Receiver<ShmEvent>> {
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name(consts::SIMAPI_WATCH_THREAD.into())
        .spawn(move || {
            if let Err(err) = watch_loop(tx) {
                eprintln!("{}: {err}", consts::SIMAPI_WATCH_THREAD);
            }
        })
        .ok()?;
    Some(rx)
}

fn watch_loop(tx: mpsc::Sender<ShmEvent>) -> io::Result<()> {
    let mut notifier = inotify::Inotify::init()?;
    notifier
        .watches()
        .add(
            consts::SIMAPI_DAT_DIR,
            inotify::WatchMask::CREATE
                | inotify::WatchMask::DELETE
                | inotify::WatchMask::MOVED_FROM
                | inotify::WatchMask::MOVED_TO,
        )?;
    let mut buf = [0u8; consts::SIMAPI_WATCH_BUF];
    loop {
        let events = notifier.read_events_blocking(&mut buf)?;
        for event in events {
            let Some(name) = event.name else {
                continue;
            };
            if name != OsStr::new(consts::SIMAPI_DAT_NAME) {
                continue;
            }
            let kind = if event.mask.contains(inotify::EventMask::DELETE)
                || event.mask.contains(inotify::EventMask::MOVED_FROM)
            {
                ShmEvent::Disappeared
            } else {
                ShmEvent::Appeared
            };
            if tx.send(kind).is_err() {
                return Ok(());
            }
        }
    }
}

pub fn exe_matches_listing(listing: &str, exe: &str) -> bool {
    if exe.is_empty() {
        return false;
    }
    let needle = exe.to_ascii_lowercase();
    for line in listing.lines().skip(1) {
        let mut parts = line.split_whitespace();
        let Some(_pid) = parts.next() else {
            continue;
        };
        let Some(comm) = parts.next() else {
            continue;
        };
        let args = parts.collect::<Vec<_>>().join(" ");
        if comm.to_ascii_lowercase().contains(&needle) || args.to_ascii_lowercase().contains(&needle)
        {
            return true;
        }
    }
    false
}

pub fn extras_from_listing(simd: &SimdConfig, listing: &str) -> Vec<RunningGame> {
    let mut extras = Vec::new();
    for sim in &simd.sims {
        let launch = sim.get_str(consts::SIMD_FIELD_LAUNCHEXE);
        let live = sim.get_str(consts::SIMD_FIELD_LIVEEXE);
        if !exe_matches_listing(listing, launch) && !exe_matches_listing(listing, live) {
            continue;
        }
        extras.push(RunningGame {
            name: sim.name().to_string(),
            game_id: sim.game_id().unwrap_or(0),
            sending: false,
            status_label: consts::LABEL_TELEMETRY_IDLE,
        });
    }
    extras
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libconfig::Value;
    use crate::simd_config::SimdSim;

    fn simd_with(name: &str, game_id: i64, exe: &str) -> SimdConfig {
        SimdConfig {
            sims: vec![SimdSim {
                settings: vec![
                    (consts::SIMD_FIELD_NAME.into(), Value::String(name.into())),
                    (consts::SIMD_FIELD_GAMEID.into(), Value::Int(game_id)),
                    (
                        consts::SIMD_FIELD_LAUNCHEXE.into(),
                        Value::String(exe.into()),
                    ),
                ],
            }],
            extra: Vec::new(),
        }
    }

    #[test]
    fn fixture_round_trip_preserves_identity() {
        let size = simdata_size();
        assert!(size > 0);
        let mut buf = vec![0u8; size];
        let input = TelemetryView {
            mtick: 42,
            simexe: 244210,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            velocity: 12,
            rpms: 4500,
            simapi: 1,
            simon: 1,
            simapiversion: consts::SIMAPI_VERSION,
            valid: 1,
        };
        assert!(write_fixture(&mut buf, &input));
        let parsed = read_simdata(&buf).expect("view");
        assert_eq!(parsed.mtick, input.mtick);
        assert_eq!(parsed.simexe, input.simexe);
        assert_eq!(parsed.simstatus, input.simstatus);
        assert_eq!(parsed.rpms, input.rpms);
        assert_eq!(parsed.simon, 1);
    }

    #[test]
    fn sending_requires_mtick_change() {
        let view = TelemetryView {
            valid: 1,
            simon: 1,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            ..TelemetryView::default()
        };
        assert!(!telemetry_sending(&view, false));
        assert!(telemetry_sending(&view, true));
    }

    #[test]
    fn resolve_games_uses_simd_name() {
        let simd = simd_with("Assetto Corsa", 244210, "acs.exe");
        let view = TelemetryView {
            valid: 1,
            simexe: 244210,
            simapi: 1,
            simon: 1,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            ..TelemetryView::default()
        };
        let games = resolve_games(&simd, &view, true, &[]);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Assetto Corsa");
        assert!(games[0].sending);
    }

    #[test]
    fn exe_match_finds_wine_args() {
        let listing = "  PID COMM ARGS\n  1 bash acs.exe -w\n";
        assert!(exe_matches_listing(listing, "acs.exe"));
        assert!(!exe_matches_listing(listing, "ams2.exe"));
    }
}
