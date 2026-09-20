use std::io::{self, stdout};
use std::time::Duration;

use anyhow::{bail, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
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
    stdout().execute(Hide)?;
    let backend = CrosstermBackend::new(stdout());
    Ok(Terminal::new(backend)?)
}

fn restore() -> Result<()> {
    stdout().execute(Show)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(consts::EVENT_POLL_MS))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key)?;
            }
        }
        app.tick();
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
