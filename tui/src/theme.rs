//! Pit-brass palette: zinc neutrals and a single brass accent.
//! Color literals stay in this module.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

use crate::consts::{self, HelpBinding};

pub const COLOR_ACCENT: Color = Color::Rgb(201, 152, 74);
pub const COLOR_TEXT: Color = Color::Rgb(214, 208, 196);
pub const COLOR_MUTED: Color = Color::Rgb(132, 126, 116);
pub const COLOR_OK: Color = COLOR_ACCENT;
pub const COLOR_ERR: Color = COLOR_TEXT;
pub const COLOR_WARN: Color = COLOR_MUTED;
pub const COLOR_SELECTED_FG: Color = Color::Rgb(28, 26, 24);
pub const COLOR_SELECTED_BG: Color = COLOR_ACCENT;
pub const COLOR_BAR_BG: Color = Color::Rgb(38, 36, 34);
pub const COLOR_GAUGE_BG: Color = Color::Rgb(52, 50, 48);

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
    Style::default().fg(COLOR_TEXT).bg(COLOR_BAR_BG)
}

pub fn style_help_bar() -> Style {
    Style::default().fg(COLOR_MUTED).bg(COLOR_BAR_BG)
}

pub fn style_hotkey() -> Style {
    Style::default()
        .fg(COLOR_ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub fn style_help_desc() -> Style {
    Style::default().fg(COLOR_MUTED)
}

pub fn style_error() -> Style {
    Style::default().fg(COLOR_ERR).add_modifier(Modifier::BOLD)
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

pub fn style_health_gauge(all_met: bool, any_met: bool) -> Style {
    if all_met {
        return style_gauge();
    }
    if any_met {
        return Style::default().fg(COLOR_WARN).bg(COLOR_GAUGE_BG);
    }
    Style::default().fg(COLOR_ERR).bg(COLOR_GAUGE_BG)
}

pub fn style_dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

pub fn help_spans(items: &[HelpBinding]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (index, item) in items.iter().enumerate() {
        push_help_item(&mut spans, index > 0, item);
    }
    spans
}

fn push_help_item(spans: &mut Vec<Span<'static>>, separate: bool, item: &HelpBinding) {
    if separate {
        spans.push(Span::styled(consts::HELP_ITEM_SEP, style_help_desc()));
    }
    if item.keys.is_empty() {
        spans.push(Span::styled(item.desc, style_help_desc()));
        return;
    }
    spans.push(Span::styled(item.keys, style_hotkey()));
    if item.desc.is_empty() {
        return;
    }
    spans.push(Span::styled(consts::HELP_KEY_DESC_SEP, style_help_desc()));
    spans.push(Span::styled(item.desc, style_help_desc()));
}

pub fn style_profile_pin() -> Style {
    style_selected()
}

pub fn panel(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border())
        .title(title.into())
        .title_style(style_title())
}

pub fn panel_line(title: Line<'static>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border())
        .title(title)
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
    let levels = width.saturating_mul(consts::GAUGE_FRACTION_COUNT);
    let filled_levels = (clamped * levels as f64).round() as usize;
    let filled_levels = filled_levels.min(levels);
    let full_cells = filled_levels / consts::GAUGE_FRACTION_COUNT;
    let remainder = filled_levels % consts::GAUGE_FRACTION_COUNT;
    let mut out = String::with_capacity(width);
    for _ in 0..full_cells {
        out.push(consts::GAUGE_FILL);
    }
    let mut cells = full_cells;
    if remainder > 0 && cells < width {
        out.push(fraction_glyph(remainder));
        cells += 1;
    }
    for _ in cells..width {
        out.push(consts::GAUGE_EMPTY);
    }
    out
}

fn fraction_glyph(eighths: usize) -> char {
    if eighths == 0 {
        return consts::GAUGE_EMPTY;
    }
    if eighths >= consts::GAUGE_FRACTION_COUNT {
        return consts::GAUGE_FILL;
    }
    consts::GAUGE_FRACTION_GLYPHS[eighths]
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
        assert_eq!(
            bar(0.0, consts::GAUGE_WIDTH).chars().count(),
            consts::GAUGE_WIDTH
        );
        assert!(bar(0.0, consts::GAUGE_WIDTH)
            .chars()
            .all(|ch| ch == consts::GAUGE_EMPTY));
        assert!(bar(1.0, consts::GAUGE_WIDTH)
            .chars()
            .all(|ch| ch == consts::GAUGE_FILL));
    }

    #[test]
    fn bar_shows_each_percent_step() {
        let max = f64::from(consts::GAUGE_PERCENT_MAX);
        for step in 0..consts::GAUGE_PERCENT_MAX {
            let current = bar(f64::from(step) / max, consts::GAUGE_WIDTH);
            let next = bar(f64::from(step + 1) / max, consts::GAUGE_WIDTH);
            assert_eq!(current.chars().count(), consts::GAUGE_WIDTH);
            assert_eq!(next.chars().count(), consts::GAUGE_WIDTH);
            assert_ne!(
                current,
                next,
                "percent {step} and {} rendered the same bar",
                step + 1
            );
        }
    }

    #[test]
    fn percent_clamps() {
        assert_eq!(percent(-1.0), 0);
        assert_eq!(percent(2.0), consts::GAUGE_PERCENT_MAX);
        assert_eq!(percent(0.5), consts::GAUGE_PERCENT_MAX / 2);
    }

    #[test]
    fn help_spans_color_keys_apart_from_descriptions() {
        let spans = help_spans(consts::HELP_DASHBOARD);
        let key = spans
            .iter()
            .find(|span| span.content.as_ref() == consts::HOTKEY_QUIT)
            .expect("quit key");
        let desc = spans
            .iter()
            .find(|span| span.content.as_ref() == consts::HELP_DESC_QUIT)
            .expect("quit description");
        assert_eq!(key.style.fg, Some(COLOR_ACCENT));
        assert_eq!(desc.style.fg, Some(COLOR_MUTED));
        assert_ne!(key.style.fg, desc.style.fg);
    }
}
