use crate::app::{App, Screen};
use crate::consts::*;
use crate::form::{field_is_combo, DeviceForm};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();
    if size.width < MIN_TERMINAL_WIDTH || size.height < MIN_TERMINAL_HEIGHT {
        draw_too_small(frame, size);
        return;
    }
    match &app.screen {
        Screen::Main => draw_main(frame, app, size),
        Screen::DeviceEdit(form) => draw_edit(frame, app, form, size),
        Screen::DeviceTune { device_index } => draw_tune(frame, app, *device_index, size),
        Screen::ConfirmDelete { device_index } => draw_confirm(frame, app, *device_index, size),
    }
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let text = format!("Terminal too small.\nMinimum: {MIN_TERMINAL_WIDTH}x{MIN_TERMINAL_HEIGHT}");
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().title(APP_TITLE).borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        area,
    );
    frame.render_widget(
        Paragraph::new(FOOTER_TOO_SMALL),
        Rect {
            x: area.x,
            y: area.bottom().saturating_sub(1),
            width: area.width,
            height: 1,
        },
    );
}

fn chrome(frame: &mut Frame, app: &App, area: Rect, footer: &str) -> Rect {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);
    let titles: Vec<Line> = TAB_LABELS.iter().copied().map(Line::from).collect();
    frame.render_widget(
        Tabs::new(titles)
            .block(Block::default().title(APP_TITLE).borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .select(app.tab),
        chunks[0],
    );
    let simd = status_span(app.status.simd_running);
    let mono = status_span(app.status.monocoque_running);
    let shm = if app.status.simapi_present {
        STATUS_PRESENT
    } else {
        STATUS_ABSENT
    };
    let status = Line::from(vec![
        Span::raw("simd "),
        simd,
        Span::raw("  monocoque "),
        mono,
        Span::raw(format!("  SIMAPI.DAT {shm}")),
    ]);
    frame.render_widget(
        Paragraph::new(status).block(Block::default().title("Status").borders(Borders::ALL)),
        chunks[1],
    );
    let message = app.message.clone().unwrap_or_default();
    frame.render_widget(
        Paragraph::new(message).block(Block::default().title("Message").borders(Borders::ALL)),
        chunks[3],
    );
    frame.render_widget(Paragraph::new(footer), chunks[4]);
    chunks[2]
}

fn status_span(running: bool) -> Span<'static> {
    if running {
        Span::styled(STATUS_RUNNING, Style::default().fg(Color::Green))
    } else {
        Span::styled(STATUS_STOPPED, Style::default().fg(Color::Red))
    }
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    let footer = match app.tab {
        TAB_DASHBOARD => FOOTER_DASHBOARD,
        TAB_DEVICES => FOOTER_DEVICES,
        TAB_LOGS => FOOTER_LOGS,
        _ => FOOTER_TABS,
    };
    let body = chrome(frame, app, area, footer);
    match app.tab {
        TAB_DASHBOARD => draw_dashboard(frame, app, body),
        TAB_DEVICES => draw_devices(frame, app, body),
        TAB_LOGS => draw_logs(frame, app, body),
        _ => {}
    }
}

fn draw_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = DASHBOARD_ACTION_LABELS
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let marker = if index == app.dashboard_action {
                ">"
            } else {
                " "
            };
            let disabled = index == ACTION_START && app.status.monocoque_running;
            let text = if disabled {
                format!("{marker} [{label}] (already running)")
            } else {
                format!("{marker} {label}")
            };
            let style = if index == app.dashboard_action {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if disabled {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(text).style(style)
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::default().title("Actions").borders(Borders::ALL)),
        area,
    );
}

fn draw_devices(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);
    let config_label = app
        .file
        .configs
        .get(app.selected_config)
        .map(|config| config.label())
        .unwrap_or_else(|| "(no configs)".to_string());
    frame.render_widget(
        Paragraph::new(format!(
            "Config {}/{}: {config_label}",
            app.selected_config + 1,
            app.file.configs.len().max(1)
        ))
        .block(
            Block::default()
                .title("Configuration")
                .borders(Borders::ALL),
        ),
        chunks[0],
    );
    let devices = app.current_devices();
    let items: Vec<ListItem> = if devices.is_empty() {
        vec![ListItem::new(
            "No devices in this configuration. Press a to add.",
        )]
    } else {
        devices
            .iter()
            .enumerate()
            .map(|(index, device)| {
                let marker = if index == app.selected_device {
                    ">"
                } else {
                    " "
                };
                let style = if index == app.selected_device {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{marker} {}", device.summary())).style(style)
            })
            .collect()
    };
    frame.render_widget(
        List::new(items).block(Block::default().title("Devices").borders(Borders::ALL)),
        chunks[1],
    );
}

fn draw_logs(frame: &mut Frame, app: &App, area: Rect) {
    let visible = area.height.saturating_sub(2) as usize;
    let total = app.log_lines.len();
    let start = total.saturating_sub(visible + app.log_offset);
    let end = (start + visible).min(total);
    let text = if total == 0 {
        "No log lines yet.".to_string()
    } else {
        app.log_lines[start..end].join("\n")
    };
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().title("Logs").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_edit(frame: &mut Frame, app: &App, form: &DeviceForm, area: Rect) {
    let body = chrome(frame, app, area, FOOTER_EDIT);
    let fields = form.visible_fields();
    let items: Vec<ListItem> = fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let label = DeviceForm::field_label(*field);
            let mut value = if form.text_editing && index == form.field_index {
                format!("{}█", form.text_buffer)
            } else {
                form.field_value(*field)
            };
            if field_is_combo(*field) {
                value = format!("< {value} >");
            }
            let marker = if index == form.field_index { ">" } else { " " };
            let style = if index == form.field_index {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(format!("{marker} {label}: {value}")).style(style)
        })
        .collect();
    let title = if form.device_index.is_some() {
        "Edit device"
    } else {
        "Add device"
    };
    frame.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::ALL)),
        body,
    );
}

fn draw_tune(frame: &mut Frame, app: &App, device_index: usize, area: Rect) {
    let body = chrome(frame, app, area, FOOTER_TUNE);
    let device = app.current_devices().get(device_index);
    let mut lines = vec![TUNING_STUB_MESSAGE.to_string(), String::new()];
    if let Some(device) = device {
        lines.push(format!("Class: {}", device.class()));
        lines.push(format!("Type: {}", device.string_or(DEVICE_TYPE_KEY, "")));
        lines.push(format!("Id: {}", device.identity()));
        for (name, value) in &device.fields {
            lines.push(format!("{name} = {value:?}"));
        }
    } else {
        lines.push("Device is no longer in this configuration.".to_string());
    }
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(
                Block::default()
                    .title("Device tuning")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        body,
    );
}

fn draw_confirm(frame: &mut Frame, app: &App, device_index: usize, area: Rect) {
    let body = chrome(frame, app, area, FOOTER_CONFIRM);
    let summary = app
        .current_devices()
        .get(device_index)
        .map(|device| device.summary())
        .unwrap_or_else(|| "unknown device".to_string());
    frame.render_widget(
        Paragraph::new(format!(
            "Delete {summary}?\nThis writes {CONFIG_FILE_NAME} immediately."
        ))
        .block(
            Block::default()
                .title("Confirm delete")
                .borders(Borders::ALL),
        ),
        body,
    );
}
