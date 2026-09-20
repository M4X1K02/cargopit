//! Walks TUI screens with real key handling and optional JSON screenshot dumps.
//!
//! Stop/Restart are not entered: those call `pkill -x cargopit` and would hit a
//! live session. Start is entered only to exercise the "already running" path.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use cargopit_tui::app::{App, Screen, SettingsSub};
use cargopit_tui::consts;
use cargopit_tui::schema::FieldId;
use cargopit_tui::ui;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use ratatui::Terminal;
use serde_json::{json, Value};

const SHOT_WIDTH: u16 = 100;
const SHOT_HEIGHT: u16 = 28;
const TINY_WIDTH: u16 = 40;
const TINY_HEIGHT: u16 = 10;
const ENV_SHOTS: &str = "CARGOPIT_TUI_SHOTS";
const STUB_PLAY_LINE: &str = "stub-cargopit play";
const STUB_TEST_LINE: &str = "stub-cargopit test";
const SEED_CAR: &str = "demo-car";
const SEED_TYRE: f64 = 0.65;
const LOG_WAIT_STEPS: u32 = 20;
const LOG_WAIT_MS: u64 = 50;

#[test]
fn walks_all_tui_features() {
    let root = isolate_xdg();
    seed_files(&root);
    let stub = write_stub_bin(&root);
    let mut app = App::new().expect("app");
    std::env::set_var(consts::ENV_PATH, &stub);

    let shots = ShotSink::from_env();
    let mut step = 0;

    shot(&mut step, &shots, &app, "dashboard", &[
        consts::TAB_TITLES[consts::TAB_DASHBOARD],
        consts::DIAGRAM_TITLE_PIPELINE,
        consts::ACTION_START,
    ]);

    press(&mut app, consts::KEY_DOWN);
    press(&mut app, consts::KEY_DOWN);
    press(&mut app, consts::KEY_DOWN);
    shot(&mut step, &shots, &app, "dashboard-stop-selected", &[consts::ACTION_STOP]);

    press(&mut app, consts::KEY_START);
    app.dashboard_index = 0;
    press(&mut app, consts::KEY_ENTER);
    let start_ok = app.message == consts::MSG_PLAY_ALREADY_RUNNING
        || app.message == consts::MSG_STARTED_PLAY;
    assert!(start_ok, "start play message: {}", app.message);
    shot(&mut step, &shots, &app, "dashboard-start", &[]);

    press(&mut app, consts::KEY_START);
    app.dashboard_index = 1;
    press(&mut app, consts::KEY_ENTER);
    wait_for_log(&mut app, STUB_TEST_LINE);
    shot(&mut step, &shots, &app, "logs-after-test", &[
        consts::TITLE_LOGS,
        STUB_TEST_LINE,
    ]);
    assert!(
        app.message == consts::MSG_STARTED_TEST || app.message.contains("exited"),
        "unexpected test message: {}",
        app.message
    );

    press(&mut app, consts::KEY_TAB2);
    shot(&mut step, &shots, &app, "devices", &[
        consts::TITLE_PROFILE,
        consts::CLASS_USB,
        consts::CLASS_SOUND,
        consts::CLASS_SERIAL,
    ]);
    assert!(matches!(app.screen, Screen::Devices));

    press(&mut app, consts::KEY_SPACE);
    shot(&mut step, &shots, &app, "devices-toggled", &["Saved"]);
    press(&mut app, consts::KEY_SPACE);

    press(&mut app, consts::KEY_EDIT);
    shot(&mut step, &shots, &app, "device-editor", &[
        consts::TITLE_DEVICE_EDITOR,
        consts::DIAGRAM_TITLE_PREVIEW,
        FieldId::Class.label(),
        consts::TYPE_TACHOMETER,
    ]);
    assert!(matches!(app.screen, Screen::DeviceForm));

    press(&mut app, consts::KEY_RIGHT);
    shot(&mut step, &shots, &app, "device-editor-class-cycled", &[consts::CLASS_SOUND]);
    press(&mut app, consts::KEY_LEFT);
    let devid_index = app
        .form
        .fields()
        .iter()
        .position(|field| *field == FieldId::Devid)
        .expect("devid field");
    app.form.field_index = devid_index;
    press(&mut app, consts::KEY_ENTER);
    press_char(&mut app, 'x');
    shot(&mut step, &shots, &app, "device-editor-typing", &[]);
    let buffer = app.form.edit_buffer.as_deref().unwrap_or("");
    assert!(
        buffer.ends_with('x'),
        "expected typed x in edit buffer, got {buffer:?}"
    );
    press(&mut app, consts::KEY_ESC);
    press(&mut app, consts::KEY_ESC);
    assert!(matches!(app.screen, Screen::Devices));

    press(&mut app, consts::KEY_DOWN);
    press(&mut app, consts::KEY_G_UPPER);
    shot(&mut step, &shots, &app, "devices-reordered", &[consts::EFFECT_ENGINE]);

    press(&mut app, consts::KEY_ENTER);
    shot(&mut step, &shots, &app, "device-tune", &[consts::TITLE_TUNE]);
    press(&mut app, consts::KEY_RIGHT);
    press(&mut app, consts::KEY_ESC);

    press(&mut app, consts::KEY_ADD);
    shot(&mut step, &shots, &app, "device-add-blank", &[consts::TITLE_DEVICE_EDITOR]);
    press(&mut app, consts::KEY_SAVE);
    shot(&mut step, &shots, &app, "device-add-validation", &[]);
    assert!(app.form.error.is_some() || !app.message.is_empty());
    press(&mut app, consts::KEY_ESC);

    press(&mut app, consts::KEY_TEMPLATE);
    shot(&mut step, &shots, &app, "templates", &[consts::TITLE_TEMPLATES]);
    press(&mut app, consts::KEY_ENTER);
    shot(&mut step, &shots, &app, "confirm-template", &[consts::CONFIRM_TEMPLATE]);
    press(&mut app, consts::KEY_CONFIRM_YES);
    shot(&mut step, &shots, &app, "devices-after-template", &[consts::EFFECT_GEAR]);

    press(&mut app, consts::KEY_DUPLICATE);
    shot(&mut step, &shots, &app, "devices-duplicated", &[]);
    press(&mut app, consts::KEY_DELETE);
    shot(&mut step, &shots, &app, "confirm-delete-device", &[consts::CONFIRM_DELETE_DEVICE]);
    press(&mut app, consts::KEY_CONFIRM_YES);

    press(&mut app, consts::KEY_SAVE);
    shot(&mut step, &shots, &app, "profile-edit", &[consts::TITLE_PROFILE, consts::KEY_SIM]);
    press(&mut app, consts::KEY_RIGHT);
    press(&mut app, consts::KEY_DOWN);
    press_char(&mut app, 'z');
    press(&mut app, consts::KEY_ADD);
    shot(&mut step, &shots, &app, "devices-new-profile", &[consts::TITLE_PROFILE]);
    press(&mut app, consts::KEY_PREV_PROFILE);
    shot(&mut step, &shots, &app, "devices-profile-switched", &[]);

    press(&mut app, consts::KEY_DELETE_UPPER);
    shot(&mut step, &shots, &app, "confirm-delete-profile", &[consts::CONFIRM_DELETE_PROFILE]);
    press(&mut app, consts::KEY_CONFIRM_NO);

    press(&mut app, consts::KEY_TAB3);
    shot(&mut step, &shots, &app, "settings", &[consts::TITLE_SETTINGS, consts::SETTINGS_ITEMS[0]]);

    press(&mut app, consts::KEY_ENTER);
    press(&mut app, consts::KEY_RIGHT);
    shot(&mut step, &shots, &app, "settings-flags", &[consts::TITLE_FLAGS, consts::FLAG_LABEL_VERBOSITY]);
    assert!(matches!(app.screen, Screen::SettingsSub(SettingsSub::Flags)));
    press(&mut app, consts::KEY_SAVE);
    press(&mut app, consts::KEY_ESC);

    open_settings_item(&mut app, 1);
    shot(&mut step, &shots, &app, "settings-simd", &[consts::TITLE_SIMD]);
    press(&mut app, consts::KEY_ESC);

    open_settings_item(&mut app, 2);
    shot(&mut step, &shots, &app, "settings-lua", &[consts::TITLE_LUA]);
    press(&mut app, consts::KEY_ENTER);
    shot(&mut step, &shots, &app, "settings-lua-copied", &[]);
    press(&mut app, consts::KEY_ESC);

    open_settings_item(&mut app, 3);
    press(&mut app, consts::KEY_RIGHT);
    shot(&mut step, &shots, &app, "settings-tach", &[consts::TITLE_TACH, consts::DIAGRAM_TITLE_LEDS]);
    press(&mut app, consts::KEY_ESC);

    open_settings_item(&mut app, 4);
    press(&mut app, consts::KEY_ADD);
    shot(&mut step, &shots, &app, "settings-tyres", &[consts::TITLE_TYRES, consts::DIAGRAM_TITLE_CHASSIS]);
    press(&mut app, consts::KEY_ESC);

    open_settings_item(&mut app, 5);
    shot(&mut step, &shots, &app, "settings-diagnostics", &[consts::TITLE_DIAGNOSTICS, consts::LABEL_GROUPS]);
    press(&mut app, consts::KEY_ESC);

    open_settings_item(&mut app, 6);
    shot(&mut step, &shots, &app, "settings-raw", &[consts::TITLE_RAW]);
    press(&mut app, consts::KEY_ESC);

    press(&mut app, consts::KEY_TAB4);
    press(&mut app, consts::KEY_SPACE);
    shot(&mut step, &shots, &app, "logs-filtered", &[consts::TITLE_LOGS, consts::LOG_FILTER_PREFIX]);

    press(&mut app, consts::KEY_START);
    assert!(matches!(app.screen, Screen::Dashboard));

    let tiny = render_text(&app, TINY_WIDTH, TINY_HEIGHT);
    assert!(tiny.contains(consts::TOO_SMALL_TITLE));
    shots.save("too-small", &app, TINY_WIDTH, TINY_HEIGHT);

    assert!(!app.should_quit);
    press(&mut app, consts::KEY_QUIT);
    assert!(app.should_quit);
}

fn wait_for_log(app: &mut App, needle: &str) {
    for _ in 0..LOG_WAIT_STEPS {
        app.tick();
        if render_text(app, SHOT_WIDTH, SHOT_HEIGHT).contains(needle) {
            return;
        }
        thread::sleep(Duration::from_millis(LOG_WAIT_MS));
    }
}

fn open_settings_item(app: &mut App, index: usize) {
    press(app, consts::KEY_TAB3);
    app.settings_index = index;
    press(app, consts::KEY_ENTER);
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
        .expect("key");
}

fn press_char(app: &mut App, ch: char) {
    press(app, KeyCode::Char(ch));
}

fn isolate_xdg() -> PathBuf {
    let root = std::env::temp_dir().join(format!("cargopit-tui-walk-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    std::env::set_var(consts::ENV_XDG_CONFIG_HOME, root.join("config"));
    std::env::set_var(consts::ENV_XDG_CACHE_HOME, root.join("cache"));
    std::env::set_var(consts::ENV_XDG_STATE_HOME, root.join("state"));
    root
}

fn seed_files(root: &Path) {
    let cfg_dir = root.join("config").join(consts::CONFIG_DIR_NAME);
    let simd_dir = root.join("config").join(consts::SIMD_CONFIG_DIR_NAME);
    let cache = root.join("cache").join(consts::CACHE_DIR_NAME);
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::create_dir_all(&simd_dir).unwrap();
    fs::create_dir_all(&cache).unwrap();
    fs::write(cfg_dir.join(consts::CONFIG_FILE_NAME), seed_config()).unwrap();
    fs::write(simd_dir.join(consts::SIMD_CONFIG_FILE_NAME), seed_simd()).unwrap();
    fs::write(cfg_dir.join(consts::DIAMETERS_FILE_NAME), seed_tyres()).unwrap();
    fs::write(cache.join("cargopit.log"), "seed log line\n").unwrap();
}

fn write_stub_bin(root: &Path) -> PathBuf {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let path = bin.join(consts::BINARY_CARGOPIT);
    let script = format!(
        "#!/bin/sh\necho '{STUB_PLAY_LINE} '\"$*\"\necho '{STUB_TEST_LINE}'\nexit 0\n"
    );
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    bin
}

fn seed_config() -> String {
    format!(
        r#"
configs = (
  {{
    {sim} = "{default_sim}";
    {car} = "{default_car}";
    {devices} = (
      {{
        {device} = "{usb}";
        {typ} = "{tach}";
        {subtype} = "{rev}";
        {devid} = "98FD:83AC";
        {granularity} = 2;
        {enabled} = true;
      }},
      {{
        {device} = "{sound}";
        {effect} = "{engine}";
        {devid} = "alsa_output.demo";
        {volume} = 70;
        {pan} = 1;
        {channels} = 2;
        {fps} = 60;
        {frequency} = 17;
        {frequency_max} = 37;
        {threshold} = 0.2;
        {enabled} = true;
      }},
      {{
        {device} = "{serial}";
        {typ} = "{haptic}";
        {effect} = "{slip}";
        {devpath} = "/dev/ttyACM0";
        {motors} = 10;
        {ampfactor} = 1.0;
        {baud} = 115200;
        {enabled} = true;
      }},
      {{
        {device} = "{serial}";
        {typ} = "{wind}";
        {devpath} = "/dev/simdev1";
        {fanpower} = 0.6;
        {enabled} = true;
      }},
      {{
        {device} = "{serial}";
        {typ} = "{leds}";
        {devpath} = "/dev/simdev0";
        {numleds} = 6;
        {startled} = 2;
        {endled} = 5;
        {enabled} = true;
      }}
    );
  }},
  {{
    {sim} = "ac";
    {car} = "all";
    {devices} = ();
  }}
);
"#,
        sim = consts::KEY_SIM,
        car = consts::KEY_CAR,
        devices = consts::KEY_DEVICES,
        device = consts::KEY_DEVICE,
        typ = consts::KEY_TYPE,
        subtype = consts::KEY_SUBTYPE,
        devid = consts::KEY_DEVID,
        granularity = consts::KEY_GRANULARITY,
        enabled = consts::KEY_ENABLED,
        usb = consts::CLASS_USB,
        tach = consts::TYPE_TACHOMETER,
        rev = consts::SUBTYPE_REVBURNER,
        sound = consts::CLASS_SOUND,
        effect = consts::KEY_EFFECT,
        engine = consts::EFFECT_ENGINE,
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
        motors = consts::KEY_MOTORS,
        ampfactor = consts::KEY_AMPFACTOR,
        baud = consts::KEY_BAUD,
        wind = consts::TYPE_SIM_WIND,
        fanpower = consts::KEY_FANPOWER,
        leds = consts::TYPE_SIMLEDS,
        numleds = consts::KEY_NUMLEDS,
        startled = consts::KEY_STARTLED,
        endled = consts::KEY_ENDLED,
        default_sim = consts::DEFAULT_SIM,
        default_car = consts::DEFAULT_CAR,
    )
}

fn seed_simd() -> String {
    format!(
        r#"
{sims} = (
  {{
    {name} = "Assetto Corsa";
    {gameid} = "244210";
    {useudp} = false;
  }}
);
"#,
        sims = consts::KEY_SIMS,
        name = consts::SIMD_FIELD_NAME,
        gameid = consts::SIMD_FIELD_GAMEID,
        useudp = consts::SIMD_FIELD_USEUDP,
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
        seed_car = SEED_CAR,
        tyre = SEED_TYRE,
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

    fn save(&self, name: &str, app: &App, width: u16, height: u16) {
        let Some(dir) = &self.dir else {
            return;
        };
        let payload = capture_json(app, width, height, name);
        let path = dir.join(format!("{name}.json"));
        fs::write(path, serde_json::to_string(&payload).unwrap()).unwrap();
    }
}

fn shot(step: &mut usize, shots: &ShotSink, app: &App, name: &str, needles: &[&str]) {
    *step += 1;
    let dump = render_text(app, SHOT_WIDTH, SHOT_HEIGHT);
    for needle in needles {
        assert!(
            dump.contains(needle),
            "step {step} ({name}) missing `{needle}`\n{dump}"
        );
    }
    shots.save(&format!("{step:02}-{name}"), app, SHOT_WIDTH, SHOT_HEIGHT);
}

fn render_text(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn capture_json(app: &App, width: u16, height: u16, name: &str) -> Value {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut cells = Vec::new();
    for y in 0..height {
        for x in 0..width {
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
    json!({ "name": name, "width": width, "height": height, "cells": cells })
}

fn color_hex(color: Color, is_fg: bool) -> String {
    let (red, green, blue) = match color {
        Color::Reset if is_fg => (238, 232, 213),
        Color::Reset => (16, 18, 24),
        Color::Black => (0, 0, 0),
        Color::Red | Color::LightRed => (220, 50, 47),
        Color::Green | Color::LightGreen => (133, 153, 0),
        Color::Yellow | Color::LightYellow => (181, 137, 0),
        Color::Blue | Color::LightBlue => (38, 139, 210),
        Color::Magenta | Color::LightMagenta => (211, 54, 130),
        Color::Cyan | Color::LightCyan => (42, 161, 152),
        Color::Gray => (147, 161, 161),
        Color::DarkGray => (88, 110, 117),
        Color::White => (253, 246, 227),
        Color::Rgb(r, g, b) => (r, g, b),
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
        7 => (238, 232, 213),
        8 => (88, 110, 117),
        9 => (220, 50, 47),
        10 => (133, 153, 0),
        11 => (181, 137, 0),
        12 => (38, 139, 210),
        13 => (211, 54, 130),
        14 => (42, 161, 152),
        _ => (238, 232, 213),
    }
}
