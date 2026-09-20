use std::io::{self, stdout};
use std::time::Duration;

use anyhow::{bail, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture};
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
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
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
    loop {
        app.tick();
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(consts::EVENT_POLL_MS))? {
            drain_events(app)?;
        }
        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn drain_events(app: &mut App) -> Result<()> {
    loop {
        app.handle_event(event::read()?)?;
        if app.should_quit {
            return Ok(());
        }
        if !event::poll(Duration::from_millis(consts::EVENT_DRAIN_MS))? {
            return Ok(());
        }
    }
}
