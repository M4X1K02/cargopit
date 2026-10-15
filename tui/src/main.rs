use std::io::{self, stdout};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use cargopit_tui::app::App;
use cargopit_tui::consts;
use cargopit_tui::ui;

fn main() {
    if print_version() {
        return;
    }
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn print_version() -> bool {
    let mut arguments = std::env::args();
    arguments.next();
    if arguments.any(|argument| argument == consts::VERSION_FLAG) {
        println!("{} {}", consts::BINARY_TUI, env!("CARGOPIT_VERSION"));
        return true;
    }
    false
}

fn run() -> Result<()> {
    if !stdout_is_tty() {
        bail!("cargopit-tui needs a real terminal (stdout is not a TTY)");
    }
    let mut app = App::new()?;
    let mut terminal = setup()?;
    let result = event_loop(&mut terminal, &mut app);
    restore()?;
    result
}

fn stdout_is_tty() -> bool {
    let term = std::env::var(consts::ENV_TERM).unwrap_or_default();
    term != consts::TERM_DUMB && io::IsTerminal::is_terminal(&io::stdout())
}

fn setup() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    stdout().execute(Hide)?;
    let backend = CrosstermBackend::new(stdout());
    Ok(Terminal::new(backend)?)
}

fn restore() -> Result<()> {
    stdout().execute(DisableMouseCapture)?;
    stdout().execute(Show)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut last_test_key = None;
    loop {
        app.tick();
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(consts::EVENT_POLL_MS))? {
            drain_events(app, &mut last_test_key)?;
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn drain_events(app: &mut App, last_test_key: &mut Option<Instant>) -> Result<()> {
    for _ in 0..consts::EVENT_DRAIN_LIMIT {
        let input = event::read()?;
        if accept_test_key(app.test_running(), &input, last_test_key) {
            app.handle_event(input)?;
        }
        if app.should_quit {
            return Ok(());
        }
        if !event::poll(Duration::from_millis(consts::EVENT_DRAIN_MS))? {
            return Ok(());
        }
    }
    Ok(())
}

fn accept_test_key(test_running: bool, input: &Event, last_press: &mut Option<Instant>) -> bool {
    if test_key_released(input) {
        *last_press = None;
        return true;
    }
    if test_key_repeat(input) {
        *last_press = Some(Instant::now());
        return false;
    }
    let repeated = suppress_test_repeat(test_running, input, *last_press);
    if is_test_key(input) {
        *last_press = Some(Instant::now());
    }
    !repeated
}

fn suppress_test_repeat(test_running: bool, input: &Event, last_test_key: Option<Instant>) -> bool {
    test_running && is_test_key(input) && press_within_repeat_window(last_test_key)
}

fn press_within_repeat_window(last_press: Option<Instant>) -> bool {
    last_press.is_some_and(|at| at.elapsed() < Duration::from_millis(consts::TEST_KEY_REPEAT_MS))
}

fn test_key_released(input: &Event) -> bool {
    matches!(
        input,
        Event::Key(key)
            if key.kind == KeyEventKind::Release && key.code == consts::KEY_TEST
    )
}

fn test_key_repeat(input: &Event) -> bool {
    matches!(
        input,
        Event::Key(key)
            if key.kind == KeyEventKind::Repeat && key.code == consts::KEY_TEST
    )
}

fn is_test_key(input: &Event) -> bool {
    matches!(
        input,
        Event::Key(key)
            if key.kind == KeyEventKind::Press && key.code == consts::KEY_TEST
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn release(code: KeyCode) -> Event {
        let mut key = KeyEvent::new(code, KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        Event::Key(key)
    }

    fn repeat(code: KeyCode) -> Event {
        let mut key = KeyEvent::new(code, KeyModifiers::NONE);
        key.kind = KeyEventKind::Repeat;
        Event::Key(key)
    }

    fn repeat_window_ago() -> Option<Instant> {
        let window = Duration::from_millis(consts::TEST_KEY_REPEAT_MS);
        Instant::now().checked_sub(window * 2)
    }

    #[test]
    fn only_a_test_key_press_counts_as_the_test_key() {
        assert!(is_test_key(&press(consts::KEY_TEST)));
        assert!(!is_test_key(&release(consts::KEY_TEST)));
        assert!(!is_test_key(&press(KeyCode::Char('x'))));
    }

    #[test]
    fn repeat_is_suppressed_only_while_a_test_runs() {
        let just_pressed = Some(Instant::now());
        let key = press(consts::KEY_TEST);
        assert!(suppress_test_repeat(true, &key, just_pressed));
        assert!(!suppress_test_repeat(false, &key, just_pressed));
    }

    #[test]
    fn first_press_and_old_presses_are_never_suppressed() {
        let key = press(consts::KEY_TEST);
        assert!(!suppress_test_repeat(true, &key, None));
        assert!(!suppress_test_repeat(true, &key, repeat_window_ago()));
    }

    #[test]
    fn other_keys_are_never_suppressed() {
        let other = press(KeyCode::Char('x'));
        assert!(!suppress_test_repeat(true, &other, Some(Instant::now())));
    }

    #[test]
    fn holding_the_test_key_keeps_later_presses_suppressed() {
        let mut last = None;
        let key = press(consts::KEY_TEST);
        assert!(accept_test_key(false, &key, &mut last));
        assert!(!accept_test_key(true, &key, &mut last));
        assert!(!accept_test_key(true, &key, &mut last));
    }

    #[test]
    fn release_allows_the_next_press_immediately() {
        let mut last = Some(Instant::now());
        let key = press(consts::KEY_TEST);
        assert!(accept_test_key(true, &release(consts::KEY_TEST), &mut last));
        assert!(last.is_none());
        assert!(accept_test_key(true, &key, &mut last));
    }

    #[test]
    fn key_repeat_refreshes_the_window_without_being_delivered() {
        let mut last = repeat_window_ago();
        let key = press(consts::KEY_TEST);
        assert!(!accept_test_key(true, &repeat(consts::KEY_TEST), &mut last));
        assert!(!accept_test_key(true, &key, &mut last));
    }
}
