//! Observe SIMAPI.DAT through a persistent mmap and inotify, without polling.

use std::ffi::OsStr;
use std::fs::{File, Metadata};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use memmap2::Mmap;

use crate::consts;
use crate::simd_config::SimdConfig;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelemetryView {
    pub mtick: u64,
    pub simexe: u64,
    pub simstatus: u32,
    pub velocity: u32,
    pub rpms: u32,
    pub gear: u32,
    pub maxrpm: u32,
    pub idlerpm: u32,
    pub lap: u32,
    pub position: u32,
    pub numlaps: u32,
    pub simapi: u8,
    pub simon: u8,
    pub simapiversion: u8,
    pub valid: u8,
    pub gearc: [u8; consts::TELEMETRY_GEARC_LEN],
    pub car: [u8; consts::TELEMETRY_NAME_LEN],
    pub track: [u8; consts::TELEMETRY_NAME_LEN],
    pub gas: f64,
    pub brake: f64,
    pub clutch: f64,
    pub steer: f64,
    pub fuel: f64,
    pub fuelcapacity: f64,
    pub abs: f64,
    pub xvelocity: f64,
    pub yvelocity: f64,
    pub zvelocity: f64,
}

impl Default for TelemetryView {
    fn default() -> Self {
        Self {
            mtick: 0,
            simexe: 0,
            simstatus: 0,
            velocity: 0,
            rpms: 0,
            gear: 0,
            maxrpm: 0,
            idlerpm: 0,
            lap: 0,
            position: 0,
            numlaps: 0,
            simapi: 0,
            simon: 0,
            simapiversion: 0,
            valid: 0,
            gearc: [0; consts::TELEMETRY_GEARC_LEN],
            car: [0; consts::TELEMETRY_NAME_LEN],
            track: [0; consts::TELEMETRY_NAME_LEN],
            gas: 0.0,
            brake: 0.0,
            clutch: 0.0,
            steer: 0.0,
            fuel: 0.0,
            fuelcapacity: 0.0,
            abs: 0.0,
            xvelocity: 0.0,
            yvelocity: 0.0,
            zvelocity: 0.0,
        }
    }
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

pub fn c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlowSnapshot {
    pub mtick: u64,
    pub rpms: u32,
    pub velocity: u32,
    pub simstatus: u32,
    pub simon: u8,
}

pub fn flow_snapshot(view: &TelemetryView) -> FlowSnapshot {
    FlowSnapshot {
        mtick: view.mtick,
        rpms: view.rpms,
        velocity: view.velocity,
        simstatus: view.simstatus,
        simon: view.simon,
    }
}

pub fn telemetry_sending(view: &TelemetryView, prev: Option<FlowSnapshot>) -> bool {
    if view.valid == 0 {
        return false;
    }
    match prev {
        Some(prev) => flow_snapshot(view) != prev,
        None => false,
    }
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
    if view.simon != 0 || view.simstatus >= consts::SIMAPI_STATUS_ACTIVEPLAY {
        return consts::LABEL_TELEMETRY_IDLE;
    }
    consts::LABEL_TELEMETRY_RUNNING
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
        return with_flow(extra, sending);
    }
    if !shm_has_identity(view) {
        if extra.is_empty() && view.simon != 0 {
            return vec![RunningGame {
                name: consts::LABEL_TEST.to_string(),
                game_id: 0,
                sending,
                status_label: status_label(view, sending),
            }];
        }
        return with_flow(extra, sending);
    }
    if !shm_game_confirmed(view, extra) {
        return with_flow(extra, sending);
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
        games.push(with_flow_one(game, sending));
    }
    games
}

fn shm_has_identity(view: &TelemetryView) -> bool {
    view.simexe != 0 || view.simapi != 0
}

fn shm_game_confirmed(view: &TelemetryView, extra: &[RunningGame]) -> bool {
    extra.iter().any(|game| extra_matches_view(game, view))
}

fn extra_matches_view(game: &RunningGame, view: &TelemetryView) -> bool {
    view.simexe != 0 && game.game_id == view.simexe
}

fn with_flow(games: &[RunningGame], sending: bool) -> Vec<RunningGame> {
    games
        .iter()
        .map(|game| with_flow_one(game, sending))
        .collect()
}

pub fn games_with_flow(games: &[RunningGame], sending: bool) -> Vec<RunningGame> {
    with_flow(games, sending)
}

pub fn flow_hold_active(last_flow: Option<std::time::Instant>, now: std::time::Instant) -> bool {
    match last_flow {
        Some(at) => {
            now.saturating_duration_since(at)
                <= std::time::Duration::from_millis(consts::FLOW_HOLD_MS)
        }
        None => false,
    }
}

fn with_flow_one(game: &RunningGame, sending: bool) -> RunningGame {
    if !sending {
        return game.clone();
    }
    RunningGame {
        name: game.name.clone(),
        game_id: game.game_id,
        sending: true,
        status_label: consts::LABEL_TELEMETRY_LIVE,
    }
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

struct MappedShm {
    map: Mmap,
    dev: u64,
    ino: u64,
}

pub struct SimApiSession {
    map: Option<MappedShm>,
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
        let mapped = self.map.as_ref()?;
        read_simdata(&mapped.map)
    }

    pub fn mapped(&self) -> bool {
        self.map.is_some()
    }

    pub fn revalidate(&mut self) {
        let path = Path::new(consts::SIMAPI_DAT_PATH);
        let meta = path.metadata().ok();
        if map_matches_file(self.map.as_ref(), meta.as_ref()) {
            return;
        }
        self.map = None;
        if meta.is_some() {
            self.remap();
        }
    }

    fn remap(&mut self) {
        self.map = open_map(Path::new(consts::SIMAPI_DAT_PATH));
    }
}

fn map_matches_file(mapped: Option<&MappedShm>, meta: Option<&Metadata>) -> bool {
    let Some(mapped) = mapped else {
        return meta.is_none();
    };
    let Some(meta) = meta else {
        return false;
    };
    meta.dev() == mapped.dev && meta.ino() == mapped.ino
}

fn open_map(path: &Path) -> Option<MappedShm> {
    let file = File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    let map = unsafe { Mmap::map(&file).ok()? };
    if map.len() < simdata_size() {
        return None;
    }
    Some(MappedShm {
        map,
        dev: meta.dev(),
        ino: meta.ino(),
    })
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
    notifier.watches().add(
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
    listing
        .lines()
        .skip(1)
        .any(|line| listing_line_has_exe(line, exe))
}

fn listing_line_has_exe(line: &str, exe: &str) -> bool {
    let mut parts = line.split_whitespace();
    let Some(_pid) = parts.next() else {
        return false;
    };
    let Some(comm) = parts.next() else {
        return false;
    };
    if token_is_exe(comm, exe) {
        return true;
    }
    if !comm_hosts_sim_exe(comm) {
        return false;
    }
    parts.any(|token| token_is_exe(token, exe))
}

fn comm_hosts_sim_exe(comm: &str) -> bool {
    let base = exe_basename(comm);
    let name = base.trim_end_matches(consts::WINDOWS_EXE_SUFFIX);
    consts::SIM_EXE_HOST_COMMS.iter().any(|host| name == *host)
}

fn token_is_exe(token: &str, exe: &str) -> bool {
    let exe = exe.trim_matches('"').to_ascii_lowercase();
    if exe.is_empty() {
        return false;
    }
    exe_basename(token) == exe
}

fn exe_basename(token: &str) -> String {
    token
        .trim_matches('"')
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token)
        .to_ascii_lowercase()
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
            status_label: consts::LABEL_TELEMETRY_RUNNING,
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
            gear: 3,
            maxrpm: 8000,
            gas: 0.4,
            simapi: 1,
            simon: 1,
            simapiversion: consts::SIMAPI_VERSION,
            valid: 1,
            ..TelemetryView::default()
        };
        assert!(write_fixture(&mut buf, &input));
        let parsed = read_simdata(&buf).expect("view");
        assert_eq!(parsed.mtick, input.mtick);
        assert_eq!(parsed.simexe, input.simexe);
        assert_eq!(parsed.simstatus, input.simstatus);
        assert_eq!(parsed.rpms, input.rpms);
        assert_eq!(parsed.simon, 1);
        assert_eq!(parsed.gear, input.gear);
        assert_eq!(parsed.gas, input.gas);
    }

    #[test]
    fn flow_hold_covers_the_window() {
        let now = std::time::Instant::now();
        assert!(!flow_hold_active(None, now));
        assert!(flow_hold_active(Some(now), now));
        let still = now
            .checked_sub(std::time::Duration::from_millis(consts::FLOW_HOLD_MS))
            .unwrap_or(now);
        assert!(flow_hold_active(Some(still), now));
        let expired = now
            .checked_sub(std::time::Duration::from_millis(consts::FLOW_HOLD_MS + 1))
            .unwrap_or(now);
        if still != expired {
            assert!(!flow_hold_active(Some(expired), now));
        }
    }

    #[test]
    fn sending_requires_a_field_change() {
        let view = TelemetryView {
            valid: 1,
            mtick: 10,
            rpms: 100,
            ..TelemetryView::default()
        };
        assert!(!telemetry_sending(&view, None));
        assert!(!telemetry_sending(&view, Some(flow_snapshot(&view))));
        let prev = FlowSnapshot {
            mtick: 9,
            rpms: 100,
            ..FlowSnapshot::default()
        };
        assert!(telemetry_sending(&view, Some(prev)));
        let rpm_prev = FlowSnapshot {
            mtick: 10,
            rpms: 90,
            ..FlowSnapshot::default()
        };
        assert!(telemetry_sending(&view, Some(rpm_prev)));
        let blank = TelemetryView::default();
        assert!(!telemetry_sending(&blank, Some(FlowSnapshot::default())));
    }

    #[test]
    fn blank_shm_still_animates_process_games_when_flowing() {
        let simd = simd_with("DirtRally2", 690790, "dirtrally2.exe");
        let extras = extras_from_listing(&simd, "  PID COMM ARGS\n  1 wine dirtrally2.exe\n");
        let view = TelemetryView {
            valid: 1,
            ..TelemetryView::default()
        };
        let live = resolve_games(&simd, &view, true, &extras);
        assert_eq!(live[0].name, "DirtRally2");
        assert!(live[0].sending);
        assert_eq!(live[0].status_label, consts::LABEL_TELEMETRY_LIVE);
        let parked = resolve_games(&simd, &view, false, &extras);
        assert!(!parked[0].sending);
        assert_eq!(parked[0].status_label, consts::LABEL_TELEMETRY_RUNNING);
        let named_but_blank_api = TelemetryView {
            valid: 1,
            simon: 1,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            mtick: 8,
            ..TelemetryView::default()
        };
        let flowing = resolve_games(&simd, &named_but_blank_api, true, &extras);
        assert_eq!(flowing[0].name, "DirtRally2");
        assert!(flowing[0].sending);
        assert_eq!(flowing[0].status_label, consts::LABEL_TELEMETRY_LIVE);
    }

    #[test]
    fn extras_from_listing_are_running_not_idle() {
        let simd = simd_with("DirtRally2", 690790, "dirtrally2.exe");
        let listing = "  PID COMM ARGS\n  1 wine dirtrally2.exe -novr\n";
        let extras = extras_from_listing(&simd, listing);
        assert_eq!(extras.len(), 1);
        assert_eq!(extras[0].name, "DirtRally2");
        assert!(!extras[0].sending);
        assert_eq!(extras[0].status_label, consts::LABEL_TELEMETRY_RUNNING);
        let blank = TelemetryView {
            valid: 1,
            ..TelemetryView::default()
        };
        let games = resolve_games(&simd, &blank, false, &extras);
        assert_eq!(games[0].name, "DirtRally2");
        assert_eq!(games[0].status_label, consts::LABEL_TELEMETRY_RUNNING);
        assert!(!games[0].sending);
    }

    #[test]
    fn resolve_games_uses_simd_name() {
        let simd = simd_with("Assetto Corsa", 244210, "acs.exe");
        let extras = extras_from_listing(&simd, "  PID COMM ARGS\n  1 acs.exe -w\n");
        let view = TelemetryView {
            valid: 1,
            simexe: 244210,
            simapi: 1,
            simon: 1,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            ..TelemetryView::default()
        };
        let games = resolve_games(&simd, &view, true, &extras);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Assetto Corsa");
        assert!(games[0].sending);
    }

    #[test]
    fn resolve_games_labels_simapi_test() {
        let simd = simd_with("Assetto Corsa", 244210, "acs.exe");
        let view = TelemetryView {
            valid: 1,
            simon: 1,
            simapi: consts::SIMULATOR_API_TEST,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            ..TelemetryView::default()
        };
        let games = resolve_games(&simd, &view, true, &[]);
        assert_eq!(games[0].name, consts::LABEL_TEST);
        assert!(games[0].sending);
    }

    #[test]
    fn exe_match_finds_wine_args() {
        let listing = "  PID COMM ARGS\n  1 bash acs.exe -w\n";
        assert!(exe_matches_listing(listing, "acs.exe"));
        assert!(!exe_matches_listing(listing, "ams2.exe"));
    }

    #[test]
    fn exe_match_finds_acr_process() {
        let listing = "  PID COMM ARGS\n  9 acr.exe Z:\\steam\\acr.exe\n";
        assert!(exe_matches_listing(listing, consts::SIM_EXE_ACR));
    }

    #[test]
    fn exe_match_ignores_pgrep_for_acr() {
        let listing =
            "  PID COMM ARGS\n  1 pgrep pgrep -f Assetto Corsa Rally/acr/Binaries/Win64/acr.exe\n";
        assert!(!exe_matches_listing(listing, consts::SIM_EXE_ACR));
        let simd = simd_with(
            "AssettoCorsaRally",
            consts::SIMULATOR_EXE_ASSETTO_CORSA_RALLY as i64,
            consts::SIM_EXE_ACR,
        );
        assert!(extras_from_listing(&simd, listing).is_empty());
    }

    #[test]
    fn exe_match_ignores_acr_watch_script() {
        let listing = "  PID COMM ARGS\n  1 bash bash /home/user/.local/bin/acr-moza-watch\n";
        assert!(!exe_matches_listing(listing, consts::SIM_EXE_ACR));
    }

    #[test]
    fn exe_match_ignores_acr_path_in_steam_wrapper() {
        let listing = format!(
            "  PID COMM ARGS\n  1 reaper reaper SteamLaunch AppId={} -- proton waitforexitandrun /games/Assetto Corsa Rally/{}\n",
            consts::SIMULATOR_EXE_ASSETTO_CORSA_RALLY,
            consts::SIM_EXE_ACR,
        );
        assert!(!exe_matches_listing(&listing, consts::SIM_EXE_ACR));
        let simd = simd_with(
            "AssettoCorsaRally",
            consts::SIMULATOR_EXE_ASSETTO_CORSA_RALLY as i64,
            consts::SIM_EXE_ACR,
        );
        assert!(extras_from_listing(&simd, &listing).is_empty());
    }

    #[test]
    fn parked_acr_shm_without_process_is_not_running() {
        let simd = simd_with(
            "AssettoCorsaRally",
            consts::SIMULATOR_EXE_ASSETTO_CORSA_RALLY as i64,
            consts::SIM_EXE_ACR,
        );
        let view = TelemetryView {
            valid: 1,
            simexe: consts::SIMULATOR_EXE_ASSETTO_CORSA_RALLY,
            simapi: consts::SIMULATOR_API_ASSETTO_CORSA,
            simon: 1,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            ..TelemetryView::default()
        };
        let games = resolve_games(&simd, &view, true, &[]);
        assert!(games.is_empty(), "parked ACR shm {games:?}");
    }

    #[test]
    fn parked_acr_shm_with_process_stays_visible() {
        let simd = simd_with(
            "AssettoCorsaRally",
            consts::SIMULATOR_EXE_ASSETTO_CORSA_RALLY as i64,
            consts::SIM_EXE_ACR,
        );
        let listing = "  PID COMM ARGS\n  9 acr.exe Z:\\steam\\acr.exe\n";
        let extras = extras_from_listing(&simd, listing);
        assert_eq!(extras.len(), 1);
        let view = TelemetryView {
            valid: 1,
            simexe: consts::SIMULATOR_EXE_ASSETTO_CORSA_RALLY,
            simapi: consts::SIMULATOR_API_ASSETTO_CORSA,
            simon: 1,
            simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
            ..TelemetryView::default()
        };
        let games = resolve_games(&simd, &view, false, &extras);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "AssettoCorsaRally");
        assert_eq!(games[0].status_label, consts::LABEL_TELEMETRY_IDLE);
    }
}
