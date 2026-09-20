//! Live SIMAPI session checks. Ignored in CI; run with:
//! `cargo test --manifest-path tui/Cargo.toml --test live_session -- --ignored --nocapture`

use std::thread;
use std::time::Duration;

use cargopit_tui::app::App;
use cargopit_tui::consts;
use cargopit_tui::process;
use cargopit_tui::simapi_shm::{self, SimApiSession};
use cargopit_tui::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

const LIVE_WAIT: Duration = Duration::from_millis(consts::EVENT_POLL_MS * 3);
const DASH_WIDTH: u16 = 100;
const DASH_HEIGHT: u16 = 28;
const DR2_CONFIG_NAME: &str = "DirtRally2";
const DR2_LAUNCH_EXE: &str = "dirtrally2.exe";

#[test]
#[ignore = "needs a live DR2 / simd session"]
fn live_session_shows_dr2_on_signal_path() {
    let listing = process::process_listing();
    assert!(
        simapi_shm::exe_matches_listing(&listing, DR2_LAUNCH_EXE),
        "dirtrally2.exe is not in the process table"
    );

    let session = SimApiSession::new();
    assert!(session.mapped(), "SIMAPI.DAT is not mapped");
    let first = session.sample().expect("first telemetry sample");
    assert_eq!(first.valid, 1);
    thread::sleep(LIVE_WAIT);
    let second = session.sample().expect("second telemetry sample");
    let mtick_changed = first.mtick != second.mtick;
    let sending = simapi_shm::telemetry_sending(&second, mtick_changed);

    let mut app = App::new().expect("app");
    app.tick();
    thread::sleep(LIVE_WAIT);
    app.tick();

    let names: Vec<_> = app.running_games.iter().map(|g| g.name.clone()).collect();
    assert!(
        names.iter().any(|name| name == DR2_CONFIG_NAME || name.contains("DiRT")),
        "running games missing DR2: {names:?} telemetry={second:?}"
    );

    let dump = render_dashboard(&app);
    assert!(dump.contains(consts::DIAGRAM_TITLE_PIPELINE), "{dump}");
    assert!(
        dump.contains(DR2_CONFIG_NAME) || dump.contains("DiRT"),
        "dashboard missing DR2 title\n{dump}"
    );
    if sending {
        assert!(dump.contains(consts::LABEL_TELEMETRY_LIVE), "{dump}");
        assert!(app.telemetry_live);
    } else {
        assert!(
            dump.contains(consts::LABEL_TELEMETRY_IDLE)
                || dump.contains(consts::LABEL_TELEMETRY_MENU),
            "expected idle/menu telemetry marker\n{dump}"
        );
    }
    eprintln!(
        "live DR2: mtick {} -> {} sending={sending} simon={} status={} rpm={} vel={} games {names:?}",
        first.mtick,
        second.mtick,
        second.simon,
        second.simstatus,
        second.rpms,
        second.velocity
    );
    eprintln!("{dump}");
}

fn render_dashboard(app: &App) -> String {
    let backend = TestBackend::new(DASH_WIDTH, DASH_HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..DASH_HEIGHT {
        for x in 0..DASH_WIDTH {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
