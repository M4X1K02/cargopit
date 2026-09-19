mod app;
mod config;
mod consts;
mod form;
mod hardware;
mod libconfig;
mod logs;
mod paths;
mod process;
mod ui;

use anyhow::{bail, Result};
use app::App;
use consts::TICK_INTERVAL_MS;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, stdout, IsTerminal};
use std::time::Duration;

fn main() -> Result<()> {
    if !io::stdout().is_terminal() {
        bail!("cargopit-tui needs a real terminal. Run it from a TTY or a terminal emulator.");
    }
    let mut app = App::new()?;
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(TICK_INTERVAL_MS))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }
        app.tick();
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
