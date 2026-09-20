use std::io::{self, stdout};
use std::time::Duration;

use anyhow::{bail, Result};
use crossterm::event::{self, Event};
use ratatui::DefaultTerminal;

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
        bail!("{}", consts::ERR_NOT_TTY);
    }
    let mut app = App::new()?;
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn stdout_is_tty() -> bool {
    let term = std::env::var(consts::ENV_TERM).unwrap_or_default();
    term != consts::TERM_DUMB && io::IsTerminal::is_terminal(&stdout())
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
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
