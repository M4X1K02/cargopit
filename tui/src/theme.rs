//! Palette and shared widget styles. Color literals stay in this module.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use crate::consts;

pub const COLOR_ACCENT: Color = Color::Cyan;
pub const COLOR_OK: Color = Color::Green;
pub const COLOR_ERR: Color = Color::Red;
pub const COLOR_WARN: Color = Color::Yellow;
pub const COLOR_MUTED: Color = Color::Gray;
pub const COLOR_TEXT: Color = Color::White;
pub const COLOR_USB: Color = Color::Cyan;
pub const COLOR_SOUND: Color = Color::Magenta;
pub const COLOR_SERIAL: Color = Color::Yellow;
pub const COLOR_SELECTED_FG: Color = Color::Black;
pub const COLOR_SELECTED_BG: Color = Color::Cyan;
pub const COLOR_STATUS_BG: Color = Color::Blue;
pub const COLOR_HELP_BG: Color = Color::DarkGray;
pub const COLOR_GAUGE_BG: Color = Color::DarkGray;

pub fn style_title() -> Style {
    Style::default()
        .fg(COLOR_ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub fn style_tab_active() -> Style {
    Style::default()
        .fg(COLOR_SELECTED_FG)
        .bg(COLOR_SELECTED_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn style_tab_inactive() -> Style {
    Style::default().fg(COLOR_MUTED)
}

pub fn style_selected() -> Style {
    Style::default()
        .fg(COLOR_SELECTED_FG)
        .bg(COLOR_SELECTED_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn style_status_bar() -> Style {
    Style::default()
        .fg(COLOR_TEXT)
        .bg(COLOR_STATUS_BG)
}

pub fn style_help_bar() -> Style {
    Style::default().fg(COLOR_TEXT).bg(COLOR_HELP_BG)
}

pub fn style_error() -> Style {
    Style::default().fg(COLOR_ERR)
}

pub fn style_ok() -> Style {
    Style::default().fg(COLOR_OK).add_modifier(Modifier::BOLD)
}

pub fn style_warn() -> Style {
    Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD)
}

pub fn style_muted() -> Style {
    Style::default().fg(COLOR_MUTED)
}

pub fn style_border() -> Style {
    Style::default().fg(COLOR_ACCENT)
}

pub fn style_gauge() -> Style {
    Style::default().fg(COLOR_OK).bg(COLOR_GAUGE_BG)
}

pub fn style_dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn panel(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border())
        .title(title.into())
        .title_style(style_title())
}

pub fn tab_bar() -> Block<'static> {
    Block::default()
        .borders(Borders::BOTTOM)
        .border_style(style_border())
}

pub fn bar(ratio: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let clamped = ratio.clamp(0.0, 1.0);
    let filled = (clamped * width as f64).round() as usize;
    let filled = filled.min(width);
    let mut out = String::with_capacity(width);
    for _ in 0..filled {
        out.push(consts::GAUGE_FILL);
    }
    for _ in filled..width {
        out.push(consts::GAUGE_EMPTY);
    }
    out
}

pub fn percent(ratio: f64) -> u16 {
    let pct = (ratio.clamp(0.0, 1.0) * f64::from(consts::GAUGE_PERCENT_MAX)).round();
    pct as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_empty_and_full() {
        assert_eq!(bar(0.0, consts::GAUGE_WIDTH).chars().count(), consts::GAUGE_WIDTH);
        assert!(bar(0.0, consts::GAUGE_WIDTH)
            .chars()
            .all(|ch| ch == consts::GAUGE_EMPTY));
        assert!(bar(1.0, consts::GAUGE_WIDTH)
            .chars()
            .all(|ch| ch == consts::GAUGE_FILL));
    }

    #[test]
    fn percent_clamps() {
        assert_eq!(percent(-1.0), 0);
        assert_eq!(percent(2.0), consts::GAUGE_PERCENT_MAX);
        assert_eq!(percent(0.5), consts::GAUGE_PERCENT_MAX / 2);
    }
}
