use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{simd_field_pairs, tune_fields, App, ConfirmKind, Screen, SettingsSub};
use crate::config::DeviceEntry;
use crate::consts;
use crate::diagnostics::{self, Diagnostics};
use crate::diagrams;
use crate::process::SessionKind;
use crate::schema::{self, DeviceClass, FieldId};
use crate::simd_config;
use crate::templates;
use crate::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();
    if size.width < consts::MIN_TERMINAL_WIDTH || size.height < consts::MIN_TERMINAL_HEIGHT {
        draw_too_small(frame, size);
        return;
    }
    let diag = app.diagnostics();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_TAB_HEIGHT),
            Constraint::Min(1),
            Constraint::Length(consts::LAYOUT_STATUS_HEIGHT),
            Constraint::Length(consts::LAYOUT_HELP_HEIGHT),
        ])
        .split(size);
    draw_tabs(frame, chunks[0], app);
    match &app.screen {
        Screen::Dashboard => draw_dashboard(frame, chunks[1], app, &diag),
        Screen::Devices => draw_devices(frame, chunks[1], app),
        Screen::Settings => draw_settings(frame, chunks[1], app),
        Screen::Logs => draw_logs(frame, chunks[1], app),
        Screen::DeviceForm => draw_form(frame, chunks[1], app, false),
        Screen::DeviceTune => draw_form(frame, chunks[1], app, true),
        Screen::ProfileEdit => draw_profile_edit(frame, chunks[1], app),
        Screen::TemplatePicker => draw_templates(frame, chunks[1], app),
        Screen::Confirm(kind) => {
            draw_devices(frame, chunks[1], app);
            draw_confirm(frame, size, kind);
        }
        Screen::SettingsSub(sub) => draw_settings_sub(frame, chunks[1], app, *sub),
    }
    draw_status(frame, chunks[2], app, &diag);
    draw_help(frame, chunks[3], app);
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let text = format!(
        "{} — minimum {}x{}",
        consts::TOO_SMALL_TITLE,
        consts::MIN_TERMINAL_WIDTH,
        consts::MIN_TERMINAL_HEIGHT
    );
    frame.render_widget(
        Paragraph::new(text)
            .style(theme::style_error())
            .block(theme::panel(consts::APP_TITLE)),
        area,
    );
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled(format!("{} ", consts::APP_TITLE), theme::style_title()),
        Span::raw(consts::TAB_PAD),
    ];
    for (index, title) in consts::TAB_TITLES.iter().enumerate() {
        let style = if index == app.tab {
            theme::style_tab_active()
        } else {
            theme::style_tab_inactive()
        };
        let label = if index == app.tab {
            format!(
                "{}{}{} {}{}{}",
                consts::TAB_PAD,
                consts::TAB_ACTIVE_LEFT,
                consts::TAB_KEYS[index],
                title,
                consts::TAB_ACTIVE_RIGHT,
                consts::TAB_PAD
            )
        } else {
            format!(
                "{}{} {}{}",
                consts::TAB_PAD,
                consts::TAB_KEYS[index],
                title,
                consts::TAB_PAD
            )
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(consts::TAB_PAD));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(theme::tab_bar()),
        area,
    );
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App, diag: &Diagnostics) {
    let configured = app.current_profile().map(|p| p.devices.len()).unwrap_or(0);
    let ratio = if configured == 0 {
        0.0
    } else {
        diag.connected as f64 / configured as f64
    };
    let device_style = if diag.missing == 0 && configured > 0 {
        theme::style_ok()
    } else if diag.connected > 0 {
        theme::style_warn()
    } else {
        theme::style_error()
    };
    let mut spans = vec![
        diagrams::led_span_compact(consts::BINARY_SIMD, app.status.simd_running),
        diagrams::led_span_compact(consts::BINARY_CARGOPIT, app.status.cargopit_running),
        diagrams::simapi_span_compact(diag.simapi_exists, diag.simapi_nonzero),
        Span::styled(
            format!(
                " {} {}/{} {} ",
                consts::LABEL_DEVICES,
                diag.connected,
                configured,
                theme::bar(ratio, consts::GAUGE_WIDTH)
            ),
            device_style,
        ),
    ];
    if !app.message.is_empty() {
        spans.push(Span::styled(consts::STATUS_SEP, theme::style_muted()));
        spans.push(Span::raw(app.message.clone()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::style_status_bar()),
        area,
    );
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    let text = match app.screen {
        Screen::Dashboard => consts::HELP_DASHBOARD,
        Screen::Devices => consts::HELP_DEVICES,
        Screen::Settings => consts::HELP_SETTINGS,
        Screen::Logs => consts::HELP_LOGS,
        Screen::DeviceForm => consts::HELP_FORM,
        Screen::DeviceTune => consts::HELP_TUNE,
        Screen::Confirm(_) => consts::HELP_CONFIRM,
        _ => consts::HELP_BACK,
    };
    frame.render_widget(
        Paragraph::new(text).style(theme::style_help_bar()),
        area,
    );
}

fn render_selectable_list(
    frame: &mut Frame,
    area: Rect,
    title: impl Into<String>,
    items: Vec<ListItem>,
    selected: usize,
) {
    let count = items.len();
    let list = List::new(items)
        .block(theme::panel(title))
        .highlight_style(theme::style_selected())
        .highlight_symbol(consts::LIST_HIGHLIGHT_SYMBOL);
    let mut state = ListState::default();
    if count > 0 {
        state.select(Some(selected.min(count.saturating_sub(1))));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App, diag: &Diagnostics) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_DASHBOARD_PIPELINE_HEIGHT),
            Constraint::Min(1),
        ])
        .split(area);
    let pipeline = vec![
        diagrams::pipeline_line(
            app.status.simd_running,
            diag.simapi_exists,
            diag.simapi_nonzero,
            app.status.cargopit_running,
        ),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "{} {}  {} {}",
                consts::LABEL_CONNECTED,
                diag.connected,
                consts::LABEL_MISSING,
                diag.missing
            ),
            theme::style_muted(),
        )),
    ];
    frame.render_widget(
        Paragraph::new(pipeline).block(theme::panel(consts::DIAGRAM_TITLE_PIPELINE)),
        rows[0],
    );
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(consts::LAYOUT_DASHBOARD_SPLIT_LEFT),
            Constraint::Percentage(consts::LAYOUT_DASHBOARD_SPLIT_RIGHT),
        ])
        .split(rows[1]);
    draw_dashboard_health(frame, cols[0], app, diag);
    draw_dashboard_actions(frame, cols[1], app);
}

fn draw_dashboard_health(frame: &mut Frame, area: Rect, app: &App, diag: &Diagnostics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_BORDER_LINES + 1),
            Constraint::Min(1),
        ])
        .split(area);
    let configured = app.current_profile().map(|p| p.devices.len()).unwrap_or(0);
    let ratio = if configured == 0 {
        0.0
    } else {
        diag.connected as f64 / configured as f64
    };
    let gauge = Gauge::default()
        .block(theme::panel(consts::DIAGRAM_TITLE_HEALTH))
        .gauge_style(theme::style_gauge())
        .percent(theme::percent(ratio))
        .label(format!("{}/{}", diag.connected, configured));
    frame.render_widget(gauge, chunks[0]);
    let mut lines = Vec::new();
    if let Some(profile) = app.current_profile() {
        for class in DeviceClass::all() {
            lines.push(diagrams::class_presence_line(profile, class, &app.discovery));
        }
    } else {
        lines.push(Line::from(consts::DIAGRAM_NO_PROFILE));
    }
    frame.render_widget(
        Paragraph::new(lines).block(theme::panel(consts::DIAGRAM_TITLE_PRESENCE)),
        chunks[1],
    );
}

fn draw_dashboard_actions(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = consts::DASHBOARD_ACTIONS
        .iter()
        .zip(consts::ACTION_HINTS.iter())
        .map(|(action, hint)| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<width$}", action, width = consts::ACTION_COLUMN_WIDTH),
                    action_style(action),
                ),
                Span::styled(*hint, theme::style_muted()),
            ]))
        })
        .collect();
    render_selectable_list(
        frame,
        area,
        consts::DIAGRAM_TITLE_ACTIONS,
        items,
        app.dashboard_index,
    );
}

fn action_style(action: &str) -> Style {
    match action {
        consts::ACTION_START => theme::style_ok(),
        consts::ACTION_TEST => theme::style_warn(),
        consts::ACTION_RESTART => theme::style_title(),
        consts::ACTION_STOP => theme::style_error(),
        _ => Style::default(),
    }
}

fn draw_devices(frame: &mut Frame, area: Rect, app: &App) {
    let profile = app.current_profile();
    let title = match profile {
        Some(p) => format!(
            "{} {}/{}  {} / {}",
            consts::TITLE_PROFILE,
            app.profile_index + 1,
            app.config.profiles.len(),
            p.sim,
            p.car
        ),
        None => consts::TITLE_NO_PROFILES.into(),
    };
    let mut items = Vec::new();
    if let Some(profile) = profile {
        if profile.devices.is_empty() {
            items.push(ListItem::new(consts::DIAGRAM_EMPTY_DEVICES));
        }
        for device in profile.devices.iter() {
            items.push(device_row(app, device));
        }
    }
    render_selectable_list(frame, area, title, items, app.device_index);
}

fn device_row(app: &App, device: &DeviceEntry) -> ListItem<'static> {
    let presence = app.discovery.presence(
        device.class(),
        device.get_str(consts::KEY_DEVID).unwrap_or(""),
        device.get_str(consts::KEY_DEVPATH).unwrap_or(""),
    );
    let glyph = if presence == consts::PRESENCE_CONNECTED {
        consts::LED_ON
    } else if presence == consts::PRESENCE_MISSING {
        consts::LED_OFF
    } else {
        consts::LED_OFF
    };
    let enabled = if device.enabled() {
        consts::LABEL_ENABLED_ON
    } else {
        consts::LABEL_ENABLED_OFF
    };
    let extra = device
        .get_str(consts::KEY_SUBTYPE)
        .or_else(|| device.get_str(consts::KEY_EFFECT))
        .unwrap_or("");
    let kind = schema::normalize_type_name(device.class(), device.type_name());
    let detail = if extra.is_empty() {
        kind.to_string()
    } else {
        format!("{kind} - {extra}")
    };
    let mut line = Line::from(vec![
        Span::raw(format!("{enabled} ")),
        Span::styled(format!("{glyph} "), diagrams::presence_style(presence)),
        Span::styled(device.class().as_str().to_string(), diagrams::class_style(device.class())),
        Span::raw(format!("  {detail}  ")),
        Span::styled(device.identity(), theme::style_muted()),
        Span::styled(format!("  [{presence}]"), diagrams::presence_style(presence)),
    ]);
    if !device.enabled() {
        line = line.style(theme::style_dim());
    }
    ListItem::new(line)
}

fn draw_form(frame: &mut Frame, area: Rect, app: &App, tune: bool) {
    let show_diagram = area.width
        >= consts::LAYOUT_FORM_LIST_MIN + consts::LAYOUT_FORM_DIAGRAM_WIDTH;
    if !show_diagram {
        draw_form_list(frame, area, app, tune);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(consts::LAYOUT_FORM_LIST_MIN),
            Constraint::Length(consts::LAYOUT_FORM_DIAGRAM_WIDTH),
        ])
        .split(area);
    draw_form_list(frame, chunks[0], app, tune);
    draw_form_diagram(frame, chunks[1], app);
}

fn draw_form_list(frame: &mut Frame, area: Rect, app: &App, tune: bool) {
    let fields = if tune {
        tune_fields(&app.form)
    } else {
        app.form.fields()
    };
    let selected = if tune {
        app.tune_index
    } else {
        app.form.field_index
    };
    let mut items = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let mut value = schema::display_value(&app.form.device, *field);
        if !tune && app.form.edit_buffer.is_some() && index == app.form.field_index {
            value = format!("{}{}", app.form.edit_buffer.as_deref().unwrap_or(""), consts::EDIT_CURSOR);
        }
        if schema::is_identity(*field) {
            let choices = app.discovery.choices_for(
                app.form.device.class(),
                *field == FieldId::Devpath,
            );
            if let Some(choice) = choices.iter().find(|c| c.value == value) {
                value = choice.label.clone();
            }
        }
        items.push(field_row(*field, &value, &app.form.device));
    }
    if let Some(err) = &app.form.error {
        items.push(ListItem::new(err.clone()).style(theme::style_error()));
    }
    if tune {
        items.push(ListItem::new(consts::TUNING_OFFLINE_HINT).style(theme::style_muted()));
    }
    let title = if tune {
        consts::TITLE_TUNE
    } else {
        consts::TITLE_DEVICE_EDITOR
    };
    render_selectable_list(frame, area, title, items, selected);
}

fn field_row(field: FieldId, value: &str, device: &DeviceEntry) -> ListItem<'static> {
    let mut spans = vec![Span::raw(format!(
        "{:<width$} {value}",
        field.label(),
        width = consts::FIELD_LABEL_WIDTH
    ))];
    if field == FieldId::Pan {
        let channels = device
            .get_i64(consts::KEY_CHANNELS)
            .unwrap_or(consts::DEFAULT_CHANNELS);
        let pan = device.get_i64(consts::KEY_PAN).unwrap_or(consts::DEFAULT_PAN);
        spans.push(Span::raw("  "));
        spans.extend(diagrams::pan_line(pan, channels).spans);
    } else if let Some(ratio) = diagrams::field_ratio(device, field) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            theme::bar(ratio, consts::GAUGE_WIDTH),
            theme::style_ok(),
        ));
    }
    ListItem::new(Line::from(spans))
}

fn draw_form_diagram(frame: &mut Frame, area: Rect, app: &App) {
    let lines = diagrams::device_diagram_lines(&app.form.device);
    frame.render_widget(
        Paragraph::new(lines).block(theme::panel(consts::DIAGRAM_TITLE_PREVIEW)),
        area,
    );
}

fn draw_profile_edit(frame: &mut Frame, area: Rect, app: &App) {
    let items = vec![
        ListItem::new(format!(
            "{}:  {}  (h/l cycle)",
            consts::KEY_SIM,
            app.profile_edit.sim
        )),
        ListItem::new(format!("{}:  {}", consts::KEY_CAR, app.profile_edit.car)),
        ListItem::new(consts::PROFILE_HINT),
    ];
    render_selectable_list(frame, area, consts::TITLE_PROFILE, items, app.profile_field);
}

fn draw_templates(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = templates::all()
        .iter()
        .map(|template| ListItem::new(template.name))
        .collect();
    render_selectable_list(
        frame,
        area,
        consts::TITLE_TEMPLATES,
        items,
        app.template_index,
    );
}

fn draw_confirm(frame: &mut Frame, area: Rect, kind: &ConfirmKind) {
    let text = match kind {
        ConfirmKind::DeleteDevice => consts::CONFIRM_DELETE_DEVICE,
        ConfirmKind::DeleteProfile => consts::CONFIRM_DELETE_PROFILE,
        ConfirmKind::RestartAfterSave => consts::CONFIRM_RESTART,
        ConfirmKind::ApplyTemplate(_) => consts::CONFIRM_TEMPLATE,
    };
    let popup = centered(
        area,
        consts::LAYOUT_CONFIRM_WIDTH,
        consts::LAYOUT_CONFIRM_HEIGHT,
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text)
            .style(theme::style_warn())
            .block(theme::panel(consts::TITLE_CONFIRM)),
        popup,
    );
}

fn draw_settings(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = consts::SETTINGS_ITEMS
        .iter()
        .map(|label| ListItem::new(*label))
        .collect();
    render_selectable_list(
        frame,
        area,
        consts::TITLE_SETTINGS,
        items,
        app.settings_index,
    );
}

fn draw_settings_sub(frame: &mut Frame, area: Rect, app: &App, sub: SettingsSub) {
    match sub {
        SettingsSub::Flags => draw_flags(frame, area, app),
        SettingsSub::Simd => draw_simd(frame, area, app),
        SettingsSub::Lua => draw_lua(frame, area, app),
        SettingsSub::Tach => draw_tach(frame, area, app),
        SettingsSub::Tyres => draw_tyres(frame, area, app),
        SettingsSub::Diagnostics => draw_diagnostics(frame, area, app),
        SettingsSub::Raw => draw_raw(frame, area, app),
    }
}

fn draw_flags(frame: &mut Frame, area: Rect, app: &App) {
    let flags = &app.tui_state.play_flags;
    let rows = [
        format!(
            "{:<width$} {}",
            consts::FLAG_LABEL_VERBOSITY,
            flags.verbosity,
            width = consts::FIELD_LABEL_WIDTH
        ),
        format!(
            "{:<width$} {}",
            consts::FLAG_LABEL_DISABLE_AUDIO,
            flags.disable_audio,
            width = consts::FIELD_LABEL_WIDTH
        ),
        format!(
            "{:<width$} {}",
            consts::FLAG_LABEL_UDP,
            flags.udp,
            width = consts::FIELD_LABEL_WIDTH
        ),
        format!(
            "{:<width$} {}",
            consts::FLAG_LABEL_FPS,
            flags.fps.unwrap_or(consts::DEFAULT_FPS),
            width = consts::FIELD_LABEL_WIDTH
        ),
        format!(
            "{:<width$} {}",
            consts::FLAG_LABEL_LOG,
            flags.log_file.as_deref().unwrap_or(consts::FLAG_LOG_DEFAULT),
            width = consts::FIELD_LABEL_WIDTH
        ),
    ];
    let items: Vec<ListItem> = rows.iter().map(|row| ListItem::new(row.clone())).collect();
    render_selectable_list(frame, area, consts::TITLE_FLAGS, items, app.flags_field);
}

fn draw_simd(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![Line::from(Span::styled(consts::SIMD_HINT, theme::style_muted()))];
    if app.simd.sims.is_empty() {
        lines.push(Line::from(consts::SIMD_EMPTY));
    }
    for (index, sim) in app.simd.sims.iter().enumerate() {
        let marker = if index == app.simd_index {
            consts::LIST_HIGHLIGHT_SYMBOL
        } else {
            consts::TAB_PAD
        };
        let style = if index == app.simd_index {
            theme::style_selected()
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(format!("{marker}{}", sim.name()), style)));
        if index != app.simd_index {
            continue;
        }
        for (key, value) in simd_field_pairs(sim) {
            lines.push(Line::from(Span::styled(
                format!("{}{key} = {value}  ({})", consts::SIMD_FIELD_INDENT, simd_config::field_help(&key)),
                theme::style_muted(),
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(theme::panel(consts::TITLE_SIMD)),
        area,
    );
}

fn draw_lua(frame: &mut Frame, area: Rect, app: &App) {
    let mut names: Vec<String> = consts::BUNDLED_LUA.iter().map(|s| (*s).to_string()).collect();
    names.extend(diagnostics::lua_scripts());
    let items: Vec<ListItem> = names.iter().map(|name| ListItem::new(name.clone())).collect();
    render_selectable_list(frame, area, consts::TITLE_LUA, items, app.lua_index);
}

fn draw_tach(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(consts::LAYOUT_BORDER_LINES + 2)])
        .split(area);
    let rows = [
        format!(
            "{}     {}",
            consts::TACH_LABEL_MAX_REVS,
            app.tach_max_revs
        ),
        format!(
            "{}  {}",
            FieldId::Granularity.label(),
            app.tach_granularity
        ),
        format!("{}   {}", consts::TACH_LABEL_OUTPUT, app.tach_path),
    ];
    let items: Vec<ListItem> = rows.iter().map(|row| ListItem::new(row.clone())).collect();
    render_selectable_list(frame, chunks[0], consts::TITLE_TACH, items, app.tach_field);
    let count = consts::DEFAULT_NUMLIGHTS;
    frame.render_widget(
        Paragraph::new(diagrams::led_strip(count, 1, count))
            .block(theme::panel(consts::DIAGRAM_TITLE_LEDS)),
        chunks[1],
    );
}

fn draw_tyres(frame: &mut Frame, area: Rect, app: &App) {
    let show_diagram = area.width >= consts::LAYOUT_TYRE_DIAGRAM_WIDTH * 2;
    if !show_diagram {
        draw_tyre_list(frame, area, app);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(consts::LAYOUT_TYRE_DIAGRAM_WIDTH),
        ])
        .split(area);
    draw_tyre_list(frame, chunks[0], app);
    let lines = match app.tyres.cars.get(app.tyre_index) {
        Some(car) => diagrams::tyre_lines(car),
        None => vec![Line::from(consts::DIAGRAM_NO_TYRES)],
    };
    frame.render_widget(
        Paragraph::new(lines).block(theme::panel(consts::DIAGRAM_TITLE_CHASSIS)),
        chunks[1],
    );
}

fn draw_tyre_list(frame: &mut Frame, area: Rect, app: &App) {
    let mut items = Vec::new();
    if app.tyres.cars.is_empty() {
        items.push(ListItem::new(consts::DIAGRAM_NO_TYRES));
    }
    for car in app.tyres.cars.iter() {
        let row = format!(
            "{} / {}  {} {} {} {}",
            car.sim, car.car, car.tyre0, car.tyre1, car.tyre2, car.tyre3
        );
        items.push(ListItem::new(row));
    }
    render_selectable_list(frame, area, consts::TITLE_TYRES, items, app.tyre_index);
}

fn draw_diagnostics(frame: &mut Frame, area: Rect, app: &App) {
    let d = app.diagnostics();
    let lines = vec![
        Line::from(format!("{}: {}", consts::LABEL_GROUPS, d.groups)),
        Line::from(diagrams::led_span(
            consts::UDEV_GROUP_INPUT,
            d.in_input,
            consts::LABEL_YES,
            consts::LABEL_NO,
        )),
        Line::from(vec![
            diagrams::led_span(
                consts::UDEV_GROUP_DIALOUT,
                d.in_dialout,
                consts::LABEL_YES,
                consts::LABEL_NO,
            ),
            diagrams::led_span(
                consts::UDEV_GROUP_UUCP,
                d.in_uucp,
                consts::LABEL_YES,
                consts::LABEL_NO,
            ),
        ]),
        Line::from(diagrams::led_span(
            consts::UDEV_RULES_PATH,
            d.udev_present,
            consts::LABEL_YES,
            consts::LABEL_NO,
        )),
        Line::from(format!(
            "{}: {}",
            consts::BINARY_CARGOPIT,
            d.cargopit_bin.as_deref().unwrap_or(consts::SIMAPI_MISSING)
        )),
        Line::from(format!(
            "{}: {}",
            consts::BINARY_SIMD,
            d.simd_bin.as_deref().unwrap_or(consts::SIMAPI_MISSING)
        )),
        Line::from(diagrams::simapi_span(d.simapi_exists, d.simapi_nonzero)),
        Line::from(format!(
            "{} {}  {} {}",
            consts::LABEL_CONNECTED,
            d.connected,
            consts::LABEL_MISSING,
            d.missing
        )),
        Line::from(format!("{} {}", consts::CONFIG_FILE_NAME, app.config_path.display())),
        Line::from(Span::styled(consts::RAW_VIEW_HINT, theme::style_muted())),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(theme::panel(consts::TITLE_DIAGNOSTICS)),
        area,
    );
}

fn draw_raw(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(
        Paragraph::new(app.raw_on_disk.clone())
            .wrap(Wrap { trim: false })
            .block(theme::panel(consts::TITLE_RAW)),
        area,
    );
}

fn draw_logs(frame: &mut Frame, area: Rect, app: &App) {
    let visible = app.logs.visible();
    let start = visible
        .len()
        .saturating_sub(area.height.saturating_sub(consts::LAYOUT_BORDER_LINES) as usize + app.logs.scroll);
    let play = SessionKind::Play.as_str();
    let test = SessionKind::Test.as_str();
    let lines: Vec<Line> = visible
        .iter()
        .skip(start)
        .map(|line| {
            let style = if line.source == play {
                theme::style_ok()
            } else if line.source == test {
                theme::style_warn()
            } else {
                theme::style_muted()
            };
            Line::from(Span::styled(
                format!("[{}] {}", line.source, line.text),
                style,
            ))
        })
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(theme::panel(format!(
            "{} {}{}",
            consts::TITLE_LOGS,
            consts::LOG_FILTER_PREFIX,
            app.logs.filter.as_str()
        ))),
        area,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn render_dump(app: &App) -> String {
        let backend = TestBackend::new(consts::MIN_TERMINAL_WIDTH + 20, consts::MIN_TERMINAL_HEIGHT + 6);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().to_string()
    }

    fn with_app<F: FnOnce(&mut App)>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(consts::ENV_XDG_CONFIG_HOME, dir.path().join("config"));
        std::env::set_var(consts::ENV_XDG_CACHE_HOME, dir.path().join("cache"));
        std::env::set_var(consts::ENV_XDG_STATE_HOME, dir.path().join("state"));
        let mut app = App::new().unwrap();
        f(&mut app);
    }

    #[test]
    fn dashboard_shows_tabs_and_status() {
        with_app(|app| {
            let dump = render_dump(app);
            assert!(dump.contains(consts::TAB_TITLES[consts::TAB_DASHBOARD]));
            assert!(dump.contains(consts::BINARY_SIMD));
            assert!(dump.contains(consts::DIAGRAM_TITLE_PIPELINE));
            assert!(dump.contains(consts::TAB_ACTIVE_LEFT));
        });
    }

    #[test]
    fn devices_and_settings_render() {
        with_app(|app| {
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::Devices;
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_PROFILE) || dump.contains(consts::TITLE_NO_PROFILES));
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::Settings;
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_SETTINGS));
        });
    }

    #[test]
    fn action_hints_match_actions() {
        assert_eq!(consts::ACTION_HINTS.len(), consts::DASHBOARD_ACTIONS.len());
    }
}
