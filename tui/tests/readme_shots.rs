//! README frames for `cargopit-tui` (`docs/tui/*.png`).
//!
//! Ignored in CI: it writes `/dev/shm/SIMAPI.DAT` and would race a live sim.
//! Regenerate JSON cell dumps with:
//! `CARGOPIT_TUI_SHOTS=/tmp/readme-shots cargo test --manifest-path tui/Cargo.toml --test readme_shots -- --ignored`
//!
//! simd and cargopit are not spawned. Other cargo tests treat a process with
//! those names as a live session. The flags are set after the last tick so the
//! dashboard shows the running state the status check paints.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use cargopit_tui::app::App;
use cargopit_tui::consts;
use cargopit_tui::hardware::HardwareChoice;
use cargopit_tui::simapi_shm::{self, TelemetryView};
use cargopit_tui::ui;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;
use serde_json::{json, Value};

const SHOT_WIDTH: u16 = 120;
const SHOT_HEIGHT: u16 = 34;
const ENV_SHOTS: &str = "CARGOPIT_TUI_SHOTS";
const DEMO_PROFILE: &str = "Rig";
const DEMO_CAR: &str = "911_gt3_r";
const DEMO_TRACK: &str = "spa";
const DEMO_LAUNCH_EXE: &str = "acs.exe";
const DEMO_GAME_ID: u64 = 244210;
const DEMO_RPM: u32 = 6400;
const DEMO_MAX_RPM: u32 = 8500;
const DEMO_VELOCITY: u32 = 214;
const DEMO_LAP: u32 = 8;
const DEMO_LAPS: u32 = 24;
const DEMO_POSITION: u32 = 3;
const DEMO_FUEL: f64 = 41.2;
const DEMO_FUEL_CAPACITY: f64 = 60.0;
const DEMO_GAS: f64 = 0.82;
const DEMO_BRAKE: f64 = 0.04;
const DEMO_STEER: f64 = -0.15;
const DEMO_GEAR: u32 = 4;
const DEMO_MTICK_FIRST: u64 = 100;
const DEMO_MTICK_LIVE: u64 = 101;
const DEMO_GRANULARITY: i64 = 2;
const DEMO_PAN_RIGHT: i64 = 1;
const DEMO_MOTORS_ALL: i64 = 10;
const DEMO_BAUD: i64 = 115200;
const DEMO_LED_START: i64 = 2;
const DEMO_LED_END: i64 = 5;
const DEMO_TYRE: f64 = 0.65;
const DEMO_HOLD: Duration = Duration::from_millis(consts::STATUS_REFRESH_MS + 100);
const SLEEP_ARG: &str = "30";
const SLEEP_BIN: &str = "/bin/sleep";
const USB_DEVID: &str = "98FD:83AC";
const SOUND_DEVID: &str = "alsa_output.demo";
const SERIAL_HAPTIC: &str = "/dev/ttyACM0";
const SERIAL_WIND: &str = "/dev/simdev1";
const SERIAL_LEDS: &str = "/dev/simdev0";
const LOG_NAME: &str = "cargopit.log";

#[test]
#[ignore = "writes SIMAPI.DAT; run to refresh README frames"]
fn readme_session_renders() {
    if Path::new(consts::SIMAPI_DAT_PATH).exists() {
        eprintln!("skip: {} already exists", consts::SIMAPI_DAT_PATH);
        return;
    }
    let mut env = DemoEnv::start();
    env.write_shm(DEMO_MTICK_FIRST);
    let mut app = App::new().expect("app");
    thread::sleep(DEMO_HOLD);
    app.tick();
    env.write_shm(DEMO_MTICK_LIVE);
    app.tick();
    stage_connected_devices(&mut app);
    app.message.clear();

    let shots = ShotSink::from_env();
    shot(&shots, &app, "dashboard");
    let dash = render_text(&app);
    assert!(dash.contains("Assetto Corsa"), "{dash}");
    assert!(dash.contains(consts::HEALTH_LABEL_READY), "{dash}");
    assert!(dash.contains(&DEMO_RPM.to_string()), "{dash}");

    press(&mut app, consts::KEY_TAB4);
    shot(&shots, &app, "telemetry");
    let telem = render_text(&app);
    assert!(telem.contains(DEMO_CAR), "{telem}");
    assert!(telem.contains(DEMO_TRACK), "{telem}");

    press(&mut app, consts::KEY_TAB2);
    shot(&shots, &app, "devices");
    assert!(render_text(&app).contains(consts::PRESENCE_CONNECTED));

    press(&mut app, consts::KEY_EDIT);
    shot(&shots, &app, "device-editor");
    leave(&mut app);

    press(&mut app, consts::KEY_DOWN);
    press(&mut app, consts::KEY_ENTER);
    shot(&shots, &app, "device-tune");
    leave(&mut app);

    press(&mut app, consts::KEY_TEMPLATE);
    shot(&shots, &app, "templates");
    press(&mut app, consts::KEY_ESC);

    press(&mut app, consts::KEY_TAB3);
    shot(&shots, &app, "settings");
    assert!(render_text(&app).contains("Assetto Corsa"));

    press(&mut app, consts::KEY_DOWN);
    press(&mut app, consts::KEY_ENTER);
    shot(&shots, &app, "settings-simd");
    press(&mut app, consts::KEY_ESC);

    press(&mut app, consts::KEY_TAB5);
    shot(&shots, &app, "logs");
    assert!(render_text(&app).contains(consts::SLOG_TAG_INFO));
}

struct DemoEnv {
    root: PathBuf,
    child: Option<Child>,
    shm: bool,
}

impl DemoEnv {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!("cargopit-readme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let cfg = root.join("config").join(consts::CONFIG_DIR_NAME);
        let simd = root.join("config").join(consts::SIMD_CONFIG_DIR_NAME);
        let cache = root.join("cache").join(consts::CACHE_DIR_NAME);
        fs::create_dir_all(&cfg).unwrap();
        fs::create_dir_all(&simd).unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::write(cfg.join(consts::CONFIG_FILE_NAME), seed_config()).unwrap();
        fs::write(simd.join(consts::SIMD_CONFIG_FILE_NAME), seed_simd()).unwrap();
        fs::write(cfg.join(consts::DIAMETERS_FILE_NAME), seed_tyres()).unwrap();
        fs::write(cache.join(LOG_NAME), seed_log()).unwrap();
        std::env::set_var(consts::ENV_XDG_CONFIG_HOME, root.join("config"));
        std::env::set_var(consts::ENV_XDG_CACHE_HOME, root.join("cache"));
        std::env::set_var(consts::ENV_XDG_STATE_HOME, root.join("state"));

        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let exe = bin_dir.join(DEMO_LAUNCH_EXE);
        fs::copy(SLEEP_BIN, &exe).unwrap();
        let child = Command::new(&exe)
            .arg(SLEEP_ARG)
            .spawn()
            .expect("sample game process");
        Self {
            root,
            child: Some(child),
            shm: false,
        }
    }

    fn write_shm(&mut self, mtick: u64) {
        let mut bytes = vec![0u8; simapi_shm::simdata_size()];
        assert!(simapi_shm::write_fixture(&mut bytes, &demo_view(mtick)));
        fs::write(consts::SIMAPI_DAT_PATH, bytes).unwrap();
        self.shm = true;
    }
}

impl Drop for DemoEnv {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if self.shm {
            let _ = fs::remove_file(consts::SIMAPI_DAT_PATH);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn demo_view(mtick: u64) -> TelemetryView {
    let mut view = TelemetryView {
        mtick,
        simexe: DEMO_GAME_ID,
        simstatus: consts::SIMAPI_STATUS_ACTIVEPLAY,
        velocity: DEMO_VELOCITY,
        rpms: DEMO_RPM,
        gear: DEMO_GEAR,
        maxrpm: DEMO_MAX_RPM,
        lap: DEMO_LAP,
        position: DEMO_POSITION,
        numlaps: DEMO_LAPS,
        simapi: consts::SIMULATOR_API_ASSETTO_CORSA,
        simon: 1,
        simapiversion: consts::SIMAPI_VERSION,
        valid: 1,
        gas: DEMO_GAS,
        brake: DEMO_BRAKE,
        steer: DEMO_STEER,
        fuel: DEMO_FUEL,
        fuelcapacity: DEMO_FUEL_CAPACITY,
        ..TelemetryView::default()
    };
    put_c_str(&mut view.car, DEMO_CAR);
    put_c_str(&mut view.track, DEMO_TRACK);
    view
}

fn put_c_str(dest: &mut [u8], text: &str) {
    let bytes = text.as_bytes();
    let n = bytes.len().min(dest.len().saturating_sub(1));
    dest[..n].copy_from_slice(&bytes[..n]);
}

fn stage_connected_devices(app: &mut App) {
    app.discovery.hid_devices.push(choice(USB_DEVID));
    app.discovery.pulse_sinks.push(choice(SOUND_DEVID));
    app.discovery.serial_ports.push(choice(SERIAL_HAPTIC));
    app.discovery.serial_ports.push(choice(SERIAL_WIND));
    app.discovery.serial_ports.push(choice(SERIAL_LEDS));
    press(app, consts::KEY_NEXT_PROFILE);
    app.status.simd_running = true;
    app.status.cargopit_running = true;
}

fn choice(value: &str) -> HardwareChoice {
    HardwareChoice {
        value: value.to_string(),
        label: value.to_string(),
    }
}

fn leave(app: &mut App) {
    press(app, consts::KEY_ESC);
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
        .expect("key");
}

fn seed_log() -> String {
    format!(
        "{tag} play started\n{tag} Revburner {USB_DEVID}\n",
        tag = consts::SLOG_TAG_INFO
    )
}

fn seed_tyres() -> String {
    format!(
        r#"
{cars} = (
  {{
    {sim} = "{default_sim}";
    {car} = "{seed_car}";
    {t0} = {tyre};
    {t1} = {tyre};
    {t2} = {tyre};
    {t3} = {tyre};
  }}
);
"#,
        cars = consts::KEY_CARS,
        sim = consts::KEY_SIM,
        car = consts::KEY_CAR,
        t0 = consts::KEY_TYRE0,
        t1 = consts::KEY_TYRE1,
        t2 = consts::KEY_TYRE2,
        t3 = consts::KEY_TYRE3,
        default_sim = consts::DEFAULT_SIM,
        seed_car = DEMO_CAR,
        tyre = DEMO_TYRE,
    )
}

fn seed_simd() -> String {
    format!(
        r#"
{sims} = (
  {{
    {name} = "Assetto Corsa";
    {gameid} = "{game_id}";
    {launch_key} = "{launch_exe}";
    {telemetry} = "{auto}";
  }}
);
"#,
        sims = consts::KEY_SIMS,
        name = consts::SIMD_FIELD_NAME,
        gameid = consts::SIMD_FIELD_GAMEID,
        launch_key = consts::SIMD_FIELD_LAUNCHEXE,
        telemetry = consts::SIMD_FIELD_TELEMETRY,
        auto = consts::SIMD_TELEMETRY_AUTO,
        game_id = DEMO_GAME_ID,
        launch_exe = DEMO_LAUNCH_EXE,
    )
}

fn seed_config() -> String {
    format!(
        r#"
configs = (
  {{
    {name} = "{profile}";
    {sim} = "{default_sim}";
    {car} = "{default_car}";
    {devices} = (
      {{
        {device} = "{usb}";
        {typ} = "{tach}";
        {subtype} = "{rev}";
        {devid} = "{usb_id}";
        {granularity} = {granularity_value};
        {enabled} = true;
      }},
      {{
        {device} = "{sound}";
        {effect} = "{engine}";
        {devid} = "{sink}";
        {volume} = {volume_value};
        {pan} = {pan_value};
        {channels} = {channels_value};
        {fps} = {fps_value};
        {frequency} = {frequency_value};
        {frequency_max} = {frequency_max_value};
        {threshold} = {threshold_value};
        {enabled} = true;
      }},
      {{
        {device} = "{serial}";
        {typ} = "{haptic}";
        {effect} = "{slip}";
        {devpath} = "{haptic_path}";
        {motors} = {motors_value};
        {ampfactor} = {ampfactor_value};
        {baud} = {baud_value};
        {enabled} = true;
      }},
      {{
        {device} = "{serial}";
        {typ} = "{wind}";
        {devpath} = "{wind_path}";
        {fanpower} = {fanpower_value};
        {enabled} = true;
      }},
      {{
        {device} = "{serial}";
        {typ} = "{leds}";
        {devpath} = "{led_path}";
        {numleds} = {numleds_value};
        {startled} = {startled_value};
        {endled} = {endled_value};
        {enabled} = true;
      }}
    );
  }}
);
"#,
        name = consts::KEY_PROFILE_NAME,
        profile = DEMO_PROFILE,
        sim = consts::KEY_SIM,
        car = consts::KEY_CAR,
        devices = consts::KEY_DEVICES,
        device = consts::KEY_DEVICE,
        typ = consts::KEY_TYPE,
        subtype = consts::KEY_SUBTYPE,
        devid = consts::KEY_DEVID,
        granularity = consts::KEY_GRANULARITY,
        granularity_value = DEMO_GRANULARITY,
        volume_value = consts::DEFAULT_VOLUME,
        pan_value = DEMO_PAN_RIGHT,
        channels_value = consts::DEFAULT_CHANNELS,
        fps_value = consts::DEFAULT_FPS,
        frequency_value = consts::DEFAULT_FREQUENCY,
        frequency_max_value = consts::DEFAULT_FREQUENCY_MAX,
        threshold_value = consts::DEFAULT_THRESHOLD,
        motors_value = DEMO_MOTORS_ALL,
        ampfactor_value = consts::DEFAULT_AMPFACTOR,
        baud_value = DEMO_BAUD,
        fanpower_value = consts::DEFAULT_FANPOWER,
        numleds_value = consts::DEFAULT_NUMLEDS,
        startled_value = DEMO_LED_START,
        endled_value = DEMO_LED_END,
        enabled = consts::KEY_ENABLED,
        usb = consts::CLASS_USB,
        tach = consts::TYPE_TACHOMETER,
        rev = consts::SUBTYPE_REVBURNER,
        usb_id = USB_DEVID,
        sound = consts::CLASS_SOUND,
        effect = consts::KEY_EFFECT,
        engine = consts::EFFECT_ENGINE,
        sink = SOUND_DEVID,
        volume = consts::KEY_VOLUME,
        pan = consts::KEY_PAN,
        channels = consts::KEY_CHANNELS,
        fps = consts::KEY_FPS,
        frequency = consts::KEY_FREQUENCY,
        frequency_max = consts::KEY_FREQUENCY_MAX,
        threshold = consts::KEY_THRESHOLD,
        serial = consts::CLASS_SERIAL,
        haptic = consts::TYPE_HAPTIC,
        slip = consts::EFFECT_TYRE_SLIP,
        devpath = consts::KEY_DEVPATH,
        haptic_path = SERIAL_HAPTIC,
        motors = consts::KEY_MOTORS,
        ampfactor = consts::KEY_AMPFACTOR,
        baud = consts::KEY_BAUD,
        wind = consts::TYPE_SIM_WIND,
        wind_path = SERIAL_WIND,
        fanpower = consts::KEY_FANPOWER,
        leds = consts::TYPE_SIMLEDS,
        led_path = SERIAL_LEDS,
        numleds = consts::KEY_NUMLEDS,
        startled = consts::KEY_STARTLED,
        endled = consts::KEY_ENDLED,
        default_sim = consts::DEFAULT_SIM,
        default_car = consts::DEFAULT_CAR,
    )
}

struct ShotSink {
    dir: Option<PathBuf>,
}

impl ShotSink {
    fn from_env() -> Self {
        let dir = std::env::var_os(ENV_SHOTS).map(PathBuf::from);
        if let Some(dir) = &dir {
            fs::create_dir_all(dir).unwrap();
        }
        Self { dir }
    }

    fn save(&self, name: &str, app: &App) {
        let Some(dir) = &self.dir else {
            return;
        };
        let payload = capture_json(app, name);
        fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_string(&payload).unwrap(),
        )
        .unwrap();
    }
}

fn shot(shots: &ShotSink, app: &App, name: &str) {
    shots.save(name, app);
}

fn render_text(app: &App) -> String {
    let backend = TestBackend::new(SHOT_WIDTH, SHOT_HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..SHOT_HEIGHT {
        for x in 0..SHOT_WIDTH {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn capture_json(app: &App, name: &str) -> Value {
    let backend = TestBackend::new(SHOT_WIDTH, SHOT_HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut cells = Vec::new();
    for y in 0..SHOT_HEIGHT {
        for x in 0..SHOT_WIDTH {
            let cell = &buffer[(x, y)];
            cells.push(json!({
                "x": x,
                "y": y,
                "ch": cell.symbol(),
                "fg": color_hex(cell.fg, true),
                "bg": color_hex(cell.bg, false),
                "bold": cell.modifier.contains(ratatui::style::Modifier::BOLD),
            }));
        }
    }
    json!({ "name": name, "width": SHOT_WIDTH, "height": SHOT_HEIGHT, "cells": cells })
}

fn color_hex(color: Color, is_fg: bool) -> String {
    let (red, green, blue) = match color {
        Color::Reset if is_fg => (214, 208, 196),
        Color::Reset => (16, 18, 24),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red | Color::LightRed => (220, 50, 47),
        Color::Green | Color::LightGreen => (133, 153, 0),
        Color::Yellow | Color::LightYellow => (181, 137, 0),
        Color::Blue | Color::LightBlue => (38, 139, 210),
        Color::Magenta | Color::LightMagenta => (211, 54, 130),
        Color::Cyan | Color::LightCyan => (42, 161, 152),
        Color::Gray => (147, 161, 161),
        Color::DarkGray => (132, 126, 116),
        Color::White => (253, 246, 227),
        Color::Indexed(index) => indexed_rgb(index),
    };
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn indexed_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0 => (0, 0, 0),
        1 => (220, 50, 47),
        2 => (133, 153, 0),
        3 => (181, 137, 0),
        4 => (38, 139, 210),
        5 => (211, 54, 130),
        6 => (42, 161, 152),
        7 => (214, 208, 196),
        8 => (132, 126, 116),
        _ => (214, 208, 196),
    }
}
