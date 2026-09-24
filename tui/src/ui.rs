use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap,
};
use ratatui::Frame;

use crate::app::{tune_fields, App, ConfirmKind, Screen, SettingsSub};
use crate::config::DeviceEntry;
use crate::consts;
use crate::diagnostics::{devices_healthy, health_score, Diagnostics, HealthScore};
use crate::diagrams;
use crate::logs;
use crate::process::SessionKind;
use crate::schema::{self, DeviceClass, FieldId};
use crate::simapi_shm;
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
    let hint = page_hint(&app.screen);
    let hint_height = if hint.is_some() {
        consts::LAYOUT_HINT_HEIGHT
    } else {
        0
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_TAB_HEIGHT),
            Constraint::Min(1),
            Constraint::Length(hint_height),
            Constraint::Length(consts::LAYOUT_STATUS_HEIGHT),
            Constraint::Length(consts::LAYOUT_HELP_HEIGHT),
        ])
        .split(size);
    draw_tabs(frame, chunks[0], app);
    match &app.screen {
        Screen::Dashboard => draw_dashboard(frame, chunks[1], app, &diag),
        Screen::Devices => draw_devices(frame, chunks[1], app),
        Screen::Settings => draw_settings(frame, chunks[1], app),
        Screen::Telemetry => draw_telemetry(frame, chunks[1], app),
        Screen::Logs => draw_logs(frame, chunks[1], app),
        Screen::DeviceForm => draw_form(frame, chunks[1], app, false),
        Screen::DeviceTune => draw_form(frame, chunks[1], app, true),
        Screen::ProfileEdit => draw_profile_edit(frame, chunks[1], app),
        Screen::TemplatePicker => draw_templates(frame, chunks[1], app),
        Screen::Confirm(kind) => {
            draw_confirm_background(frame, chunks[1], app, &diag);
            draw_confirm(frame, size, kind);
        }
        Screen::SettingsSub(sub) => draw_settings_sub(frame, chunks[1], app, *sub),
    }
    if let Some(text) = hint {
        draw_page_hint(frame, chunks[2], text);
    }
    draw_status(frame, chunks[3], app, &diag);
    draw_help(frame, chunks[4], app);
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
        spans.extend(tab_spans(index, title, index == app.tab));
        spans.push(Span::raw(consts::TAB_PAD));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(theme::tab_bar()),
        area,
    );
}

fn tab_spans(index: usize, title: &str, active: bool) -> Vec<Span<'static>> {
    let key = consts::TAB_KEYS[index];
    if active {
        return vec![
            Span::styled(consts::TAB_PAD, theme::style_tab_active()),
            Span::styled(consts::TAB_ACTIVE_LEFT, theme::style_tab_active()),
            Span::styled(key, theme::style_tab_active()),
            Span::styled(format!(" {title}"), theme::style_tab_active()),
            Span::styled(consts::TAB_ACTIVE_RIGHT, theme::style_tab_active()),
            Span::styled(consts::TAB_PAD, theme::style_tab_active()),
        ];
    }
    vec![
        Span::styled(consts::TAB_PAD, theme::style_tab_inactive()),
        Span::styled(key, theme::style_hotkey()),
        Span::styled(format!(" {title}"), theme::style_tab_inactive()),
        Span::styled(consts::TAB_PAD, theme::style_tab_inactive()),
    ]
}

fn bound_panel_title(base: &str, app: &App) -> String {
    format!(
        "{}{}{}",
        base,
        consts::SETTINGS_BOUND_SEP,
        app.settings_game_label()
    )
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App, diag: &Diagnostics) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(consts::STATUS_HEALTH_WIDTH),
        ])
        .split(area);
    let mut spans = vec![
        diagrams::led_span_compact(consts::BINARY_SIMD, app.status.simd_running),
        diagrams::led_span_compact(consts::BINARY_CARGOPIT, app.pipeline_pit()),
        diagrams::simapi_span_compact(diag.simapi_exists, diag.simapi_live),
        profile_pin_span(app),
    ];
    if !app.message.is_empty() {
        spans.push(Span::styled(consts::STATUS_SEP, theme::style_muted()));
        spans.push(Span::raw(app.message.clone()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(theme::style_status_bar()),
        chunks[0],
    );
    frame.render_widget(health_gauge(current_health(app, diag), true), chunks[1]);
}

fn page_hint(screen: &Screen) -> Option<&'static str> {
    match screen {
        Screen::DeviceTune => Some(consts::TUNING_OFFLINE_HINT),
        Screen::SettingsSub(SettingsSub::Raw) => Some(consts::RAW_VIEW_HINT),
        _ => None,
    }
}

fn draw_page_hint(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(Paragraph::new(text).style(theme::style_muted()), area);
}

fn current_health(app: &App, diag: &Diagnostics) -> HealthScore {
    let configured = app.current_profile().map(|p| p.devices.len()).unwrap_or(0);
    health_score(
        app.status.simd_running,
        app.pipeline_pit(),
        diag.simapi_exists,
        devices_healthy(configured, diag.missing),
    )
}

fn health_gauge(score: HealthScore, status_bar: bool) -> Gauge<'static> {
    let gauge = Gauge::default()
        .ratio(score.ratio())
        .label(score.label())
        .use_unicode(consts::HEALTH_GAUGE_UNICODE)
        .gauge_style(theme::style_health_gauge(score.all_met(), score.met > 0));
    if status_bar {
        return gauge.style(theme::style_status_bar());
    }
    gauge
}

fn health_check_line(app: &App, diag: &Diagnostics) -> Line<'static> {
    let configured = app.current_profile().map(|p| p.devices.len()).unwrap_or(0);
    Line::from(vec![
        diagrams::led_span_compact(consts::BINARY_SIMD, app.status.simd_running),
        diagrams::led_span_compact(consts::BINARY_CARGOPIT, app.pipeline_pit()),
        diagrams::simapi_span_compact(diag.simapi_exists, diag.simapi_live),
        diagrams::led_span_compact(
            consts::LABEL_DEVICES,
            devices_healthy(configured, diag.missing),
        ),
    ])
}

fn draw_help(frame: &mut Frame, area: Rect, app: &App) {
    let items = match app.screen {
        Screen::Dashboard => consts::HELP_DASHBOARD,
        Screen::Devices => consts::HELP_DEVICES,
        Screen::Settings => consts::HELP_SETTINGS,
        Screen::Telemetry => consts::HELP_TELEMETRY,
        Screen::Logs => consts::HELP_LOGS,
        Screen::DeviceForm => consts::HELP_FORM,
        Screen::DeviceTune => consts::HELP_TUNE,
        Screen::Confirm(_) => consts::HELP_CONFIRM,
        Screen::TemplatePicker => consts::HELP_TEMPLATES,
        Screen::SettingsSub(sub) => settings_sub_help(sub),
        _ => consts::HELP_BACK,
    };
    frame.render_widget(
        Paragraph::new(Line::from(theme::help_spans(items))).style(theme::style_help_bar()),
        area,
    );
}

fn settings_sub_help(sub: SettingsSub) -> &'static [consts::HelpBinding] {
    match sub {
        SettingsSub::Flags => consts::HELP_FLAGS,
        SettingsSub::Simd => consts::HELP_SIMD,
        SettingsSub::Lua => consts::HELP_LUA,
        SettingsSub::Tach => consts::HELP_TACH,
        SettingsSub::Tyres => consts::HELP_TYRES,
        SettingsSub::Raw => consts::HELP_RAW,
        SettingsSub::Diagnostics => consts::HELP_BACK,
    }
}

fn render_selectable_list(
    frame: &mut Frame,
    area: Rect,
    title: impl Into<String>,
    items: Vec<ListItem>,
    selected: usize,
) {
    render_selectable_list_block(frame, area, theme::panel(title), items, selected);
}

fn render_selectable_list_block(
    frame: &mut Frame,
    area: Rect,
    block: ratatui::widgets::Block<'static>,
    items: Vec<ListItem>,
    selected: usize,
) {
    let count = items.len();
    let list = List::new(items)
        .block(block)
        .highlight_style(theme::style_selected())
        .highlight_symbol(consts::LIST_HIGHLIGHT_SYMBOL);
    let mut state = ListState::default();
    if count > 0 {
        state.select(Some(selected.min(count.saturating_sub(1))));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

/// State reported by the running play session over its control socket.
fn play_session_line(app: &App) -> Option<Line<'static>> {
    let status = app.play_status.as_ref()?;
    let mut parts = vec![
        format!("{} {}", consts::LABEL_PLAY_SESSION, status.state),
        format!("{} {}", status.devices, consts::LABEL_PLAY_DEVICES),
        format!("{} {}", status.overruns, consts::LABEL_PLAY_OVERRUNS),
    ];
    if status.releasing {
        parts.push(consts::LABEL_PLAY_RELEASING.to_string());
    }
    if status.paused {
        parts.push(consts::LABEL_PLAY_PAUSED.to_string());
    }
    Some(Line::from(Span::styled(
        parts.join(consts::PLAY_STATUS_SEP),
        theme::style_muted(),
    )))
}

fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App, diag: &Diagnostics) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_DASHBOARD_PIPELINE_HEIGHT),
            Constraint::Min(1),
        ])
        .split(area);
    let mut pipeline = diagrams::pipeline_lines(
        app.status.simd_running,
        &app.running_games,
        diag.simapi_exists,
        diag.simapi_live,
        app.pipeline_pit(),
        app.flow_frame,
    );
    pipeline.push(Line::from(""));
    pipeline.push(Line::from(Span::styled(
        format!(
            "{} {}  {} {}",
            consts::LABEL_CONNECTED,
            diag.connected,
            consts::LABEL_MISSING,
            diag.missing
        ),
        theme::style_muted(),
    )));
    if let Some(session) = play_session_line(app) {
        pipeline.push(session);
    }
    if let Some(rpm) = telemetry_rpm_line(app) {
        pipeline.push(rpm);
    }
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

fn telemetry_rpm_line(app: &App) -> Option<Line<'static>> {
    if !app.telemetry_live || app.telemetry.rpms == 0 {
        return None;
    }
    Some(Line::from(Span::styled(
        format!("{} {}", consts::LABEL_RPM, app.telemetry.rpms),
        theme::style_muted(),
    )))
}

fn draw_telemetry(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(consts::LAYOUT_TELEMETRY_SPLIT_LEFT),
            Constraint::Percentage(consts::LAYOUT_TELEMETRY_SPLIT_RIGHT),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(telemetry_session_lines(app))
            .block(theme::panel(consts::TITLE_TELEMETRY_SESSION)),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(telemetry_control_lines(app))
            .block(theme::panel(consts::TITLE_TELEMETRY_CONTROLS)),
        cols[1],
    );
}

fn telemetry_session_lines(app: &App) -> Vec<Line<'static>> {
    let view = &app.telemetry;
    if view.valid == 0 {
        return vec![kv_line(
            consts::LABEL_SIMAPI,
            consts::SIMAPI_MISSING.to_string(),
        )];
    }
    let game = app
        .running_games
        .first()
        .map(|game| game.name.clone())
        .or_else(|| simapi_shm::simulator_api_label(view.simapi).map(str::to_string))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| consts::LABEL_NO_SIM.to_string());
    let car = field_or_dash(simapi_shm::c_string(&view.car));
    let track = field_or_dash(simapi_shm::c_string(&view.track));
    let status = simapi_shm::status_label(view, app.telemetry_live);
    let flow = if app.telemetry_live {
        consts::LABEL_TELEMETRY_LIVE
    } else {
        status
    };
    vec![
        kv_line(consts::LABEL_SIMAPI, flow.to_string()),
        kv_line(consts::BINARY_SIMD, game),
        kv_line(consts::LABEL_CAR, car),
        kv_line(consts::LABEL_TRACK, track),
        kv_line(
            consts::LABEL_LAP,
            format!("{} / {}", view.lap, view.numlaps),
        ),
        kv_line(consts::LABEL_POSITION, view.position.to_string()),
        kv_line(consts::LABEL_MTICK, view.mtick.to_string()),
    ]
}

fn telemetry_control_lines(app: &App) -> Vec<Line<'static>> {
    let view = &app.telemetry;
    if view.valid == 0 {
        return vec![kv_line(
            consts::LABEL_RPM,
            consts::TELEMETRY_DASH_VALUE.to_string(),
        )];
    }
    let gear = telemetry_gear_label(view);
    let fuel = if view.fuelcapacity > 0.0 {
        format!("{:.1} / {:.1}", view.fuel, view.fuelcapacity)
    } else {
        format!("{:.1}", view.fuel)
    };
    vec![
        gauge_line(
            consts::LABEL_RPM,
            rpm_ratio(view),
            format!("{} / {}", view.rpms, view.maxrpm),
        ),
        kv_line(consts::LABEL_GEAR, gear),
        kv_line(consts::LABEL_VELOCITY, view.velocity.to_string()),
        gauge_line(
            consts::LABEL_THROTTLE,
            clamp_unit(view.gas),
            format!("{:.2}", view.gas),
        ),
        gauge_line(
            consts::LABEL_BRAKE,
            clamp_unit(view.brake),
            format!("{:.2}", view.brake),
        ),
        gauge_line(
            consts::LABEL_CLUTCH,
            clamp_unit(view.clutch),
            format!("{:.2}", view.clutch),
        ),
        kv_line(consts::LABEL_STEER, format!("{:.2}", view.steer)),
        kv_line(consts::LABEL_FUEL, fuel),
        kv_line(consts::LABEL_ABS, format!("{:.2}", view.abs)),
        kv_line(
            consts::LABEL_LOCAL_VEL,
            format!(
                "{:.1} {:.1} {:.1}",
                view.xvelocity, view.yvelocity, view.zvelocity
            ),
        ),
    ]
}

fn kv_line(label: &'static str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = consts::TELEMETRY_LABEL_WIDTH),
            theme::style_muted(),
        ),
        Span::styled(value, theme::style_ok()),
    ])
}

fn gauge_line(label: &'static str, ratio: f64, detail: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<width$} ", width = consts::TELEMETRY_LABEL_WIDTH),
            theme::style_muted(),
        ),
        Span::styled(theme::bar(ratio, consts::GAUGE_WIDTH), theme::style_ok()),
        Span::raw(" "),
        Span::styled(detail, theme::style_ok()),
    ])
}

fn telemetry_gear_label(view: &simapi_shm::TelemetryView) -> String {
    let gear = field_or_dash(simapi_shm::c_string(&view.gearc));
    if gear == consts::TELEMETRY_DASH_VALUE {
        return view.gear.to_string();
    }
    format!("{gear} ({})", view.gear)
}

fn field_or_dash(value: String) -> String {
    if value.is_empty() {
        consts::TELEMETRY_DASH_VALUE.to_string()
    } else {
        value
    }
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn rpm_ratio(view: &simapi_shm::TelemetryView) -> f64 {
    if view.maxrpm == 0 {
        return 0.0;
    }
    clamp_unit(view.rpms as f64 / view.maxrpm as f64)
}

fn draw_dashboard_health(frame: &mut Frame, area: Rect, app: &App, diag: &Diagnostics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(
                consts::LAYOUT_BORDER_LINES
                    + consts::LAYOUT_HEALTH_GAUGE_HEIGHT
                    + consts::LAYOUT_HEALTH_CHECKS_HEIGHT,
            ),
            Constraint::Min(1),
        ])
        .split(area);
    let block = theme::panel(consts::DIAGRAM_TITLE_HEALTH);
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);
    let health_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_HEALTH_GAUGE_HEIGHT),
            Constraint::Length(consts::LAYOUT_HEALTH_CHECKS_HEIGHT),
        ])
        .split(inner);
    frame.render_widget(
        health_gauge(current_health(app, diag), false),
        health_rows[0],
    );
    frame.render_widget(Paragraph::new(health_check_line(app, diag)), health_rows[1]);
    let mut lines = Vec::new();
    if let Some(profile) = app.current_profile() {
        for class in DeviceClass::all() {
            lines.push(diagrams::class_presence_line(
                profile,
                class,
                &app.discovery,
            ));
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
        consts::ACTION_TEST => theme::style_title(),
        consts::ACTION_RESTART => theme::style_title(),
        consts::ACTION_STOP => theme::style_error(),
        _ => Style::default(),
    }
}

fn profile_pin_span(app: &App) -> Span<'static> {
    Span::styled(
        format!(
            "{}{}{}",
            consts::PROFILE_PIN_PAD,
            app.profile_pin_label(),
            consts::PROFILE_PIN_PAD
        ),
        theme::style_profile_pin(),
    )
}

fn profile_scoped_title(base: &str, app: &App) -> Line<'static> {
    Line::from(vec![
        Span::styled(base.to_string(), theme::style_title()),
        Span::raw(consts::SETTINGS_BOUND_SEP),
        profile_pin_span(app),
    ])
}

fn draw_devices(frame: &mut Frame, area: Rect, app: &App) {
    let profile = app.current_profile();
    let mut items = Vec::new();
    if let Some(profile) = profile {
        if profile.devices.is_empty() {
            items.push(ListItem::new(consts::DIAGRAM_EMPTY_DEVICES));
        }
        for device in profile.devices.iter() {
            items.push(device_row(app, device));
        }
    } else {
        items.push(ListItem::new(consts::TITLE_NO_PROFILES));
    }
    draw_list_with_help(
        frame,
        area,
        consts::TITLE_DEVICES,
        items,
        app.device_index,
        consts::TITLE_PROFILE_SHARED,
        consts::NOTE_PROFILES_CROSS_GAME,
    );
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
        Span::styled(
            device.class().as_str().to_string(),
            diagrams::class_style(device.class()),
        ),
        Span::raw(format!("  {detail}  ")),
        Span::styled(device.identity(), theme::style_muted()),
        Span::styled(
            format!("  [{presence}]"),
            diagrams::presence_style(presence),
        ),
    ]);
    if !device.enabled() {
        line = line.style(theme::style_dim());
    }
    ListItem::new(line)
}

fn split_list_help(area: Rect) -> Option<(Rect, Rect)> {
    if area.width < consts::LAYOUT_FORM_LIST_MIN + consts::LAYOUT_FORM_DIAGRAM_WIDTH {
        return None;
    }
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(consts::LAYOUT_FORM_LIST_MIN),
            Constraint::Length(consts::LAYOUT_FORM_DIAGRAM_WIDTH),
        ])
        .split(area);
    Some((chunks[0], chunks[1]))
}

fn draw_about_panel(frame: &mut Frame, area: Rect, title: impl Into<String>, text: &str) {
    frame.render_widget(
        Paragraph::new(text.to_string())
            .wrap(Wrap { trim: true })
            .style(theme::style_muted())
            .block(theme::panel(title)),
        area,
    );
}

fn draw_list_with_help(
    frame: &mut Frame,
    area: Rect,
    title: impl Into<String>,
    items: Vec<ListItem>,
    selected: usize,
    help_title: impl Into<String>,
    help: &str,
) {
    let title = title.into();
    let Some((list, side)) = split_list_help(area) else {
        render_selectable_list(frame, area, title, items, selected);
        return;
    };
    render_selectable_list(frame, list, title, items, selected);
    draw_about_panel(frame, side, help_title, help);
}

fn draw_form(frame: &mut Frame, area: Rect, app: &App, tune: bool) {
    let Some((list, side)) = split_list_help(area) else {
        draw_form_list(frame, area, app, tune);
        return;
    };
    draw_form_list(frame, list, app, tune);
    draw_form_side(frame, side, app, tune);
}

fn draw_form_side(frame: &mut Frame, area: Rect, app: &App, tune: bool) {
    let show_diagram = area.height
        >= consts::LAYOUT_FORM_HELP_HEIGHT
            + consts::LAYOUT_FORM_TEST_MIN_HEIGHT
            + consts::LAYOUT_FORM_DIAGRAM_MIN_HEIGHT;
    if !show_diagram {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(consts::LAYOUT_FORM_HELP_HEIGHT),
                Constraint::Min(1),
            ])
            .split(area);
        draw_field_help(frame, chunks[0], app, tune);
        draw_device_test_panel(frame, chunks[1], app);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_FORM_HELP_HEIGHT),
            Constraint::Min(consts::LAYOUT_FORM_TEST_MIN_HEIGHT),
            Constraint::Length(consts::LAYOUT_FORM_DIAGRAM_MIN_HEIGHT),
        ])
        .split(area);
    draw_field_help(frame, chunks[0], app, tune);
    draw_device_test_panel(frame, chunks[1], app);
    draw_form_diagram(frame, chunks[2], app);
}

fn draw_device_test_panel(frame: &mut Frame, area: Rect, app: &App) {
    let height = area.height.saturating_sub(consts::LAYOUT_BORDER_LINES) as usize;
    frame.render_widget(
        Paragraph::new(device_test_lines(app, height)).block(theme::panel_line(
            profile_scoped_title(consts::TITLE_DEVICE_TEST, app),
        )),
        area,
    );
}

fn device_test_lines(app: &App, height: usize) -> Vec<Line<'static>> {
    let status = if app.test_running() {
        consts::TEST_PANEL_RUNNING
    } else {
        consts::TEST_PANEL_IDLE
    };
    let mut lines = vec![Line::from(Span::styled(status, theme::style_ok()))];
    lines.extend(device_test_telemetry_lines(app));
    let log_room = height.saturating_sub(lines.len() + 1);
    if log_room == 0 {
        return lines;
    }
    lines.push(Line::from(""));
    lines.extend(device_test_log_lines(app, log_room.saturating_sub(1)));
    lines
}

fn device_test_telemetry_lines(app: &App) -> Vec<Line<'static>> {
    let view = &app.telemetry;
    if view.valid == 0 {
        return vec![kv_line(
            consts::LABEL_RPM,
            consts::TELEMETRY_DASH_VALUE.to_string(),
        )];
    }
    vec![
        gauge_line(
            consts::LABEL_RPM,
            rpm_ratio(view),
            format!("{} / {}", view.rpms, view.maxrpm),
        ),
        kv_line(consts::LABEL_GEAR, telemetry_gear_label(view)),
        kv_line(consts::LABEL_VELOCITY, view.velocity.to_string()),
        gauge_line(
            consts::LABEL_THROTTLE,
            clamp_unit(view.gas),
            format!("{:.2}", view.gas),
        ),
        gauge_line(
            consts::LABEL_BRAKE,
            clamp_unit(view.brake),
            format!("{:.2}", view.brake),
        ),
    ]
}

fn device_test_log_lines(app: &App, limit: usize) -> Vec<Line<'static>> {
    let steps = app.logs.recent_containing(consts::TEST_STEP_PREFIX, limit);
    let lines = if steps.is_empty() {
        app.logs.recent_from(SessionKind::Test.as_str(), limit)
    } else {
        steps
    };
    lines
        .into_iter()
        .map(|line| Line::from(Span::styled(line.text.clone(), log_line_style(&line.text))))
        .collect()
}

fn selected_form_field(app: &App, tune: bool) -> Option<FieldId> {
    if tune {
        tune_fields(&app.form).get(app.tune_index).copied()
    } else {
        app.form.current_field()
    }
}

fn draw_field_help(frame: &mut Frame, area: Rect, app: &App, tune: bool) {
    let (title, text) = match selected_form_field(app, tune) {
        Some(field) => (field.label(), field_help_text(app, field)),
        None => (consts::TITLE_ABOUT, consts::DIAGRAM_NO_DEVICE.to_string()),
    };
    draw_about_panel(frame, area, title, &text);
}

/// Identity cells are too narrow for a full sink label, so the help panel repeats it.
fn field_help_text(app: &App, field: FieldId) -> String {
    let help = field.help();
    let value = schema::display_value(&app.form.device, field);
    match identity_choice_label(app, field, &value) {
        Some(label) => format!("{help}\n{label}"),
        None => help.to_string(),
    }
}

fn identity_choice_label(app: &App, field: FieldId, value: &str) -> Option<String> {
    if !schema::is_identity(field) || value.is_empty() {
        return None;
    }
    app.discovery
        .choices_for(app.form.device.class(), field == FieldId::Devpath)
        .iter()
        .find(|choice| choice.value == value)
        .map(|choice| choice.label.clone())
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
    let mut rows = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let value = form_field_value(app, *field, index, tune);
        rows.push(field_table_row(
            *field,
            &value,
            &app.form.device,
            app.form.output_slot,
            *field == FieldId::Pan && index == selected,
        ));
    }
    if let Some(err) = &app.form.error {
        rows.push(form_notice_row(err.clone(), theme::style_error()));
    }
    let title = if tune {
        consts::TITLE_TUNE
    } else {
        consts::TITLE_DEVICE_EDITOR
    };
    let table = Table::new(rows, form_column_constraints())
        .column_spacing(consts::FORM_COL_SPACING)
        .flex(Flex::Start)
        .block(theme::panel_line(profile_scoped_title(title, app)))
        .row_highlight_style(theme::style_selected())
        .highlight_symbol(consts::LIST_HIGHLIGHT_SYMBOL);
    let mut state = TableState::default();
    if !fields.is_empty() {
        state.select(Some(selected.min(fields.len() - 1)));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn form_field_value(app: &App, field: FieldId, index: usize, tune: bool) -> String {
    if !tune && app.form.edit_buffer.is_some() && index == app.form.field_index {
        return format!(
            "{}{}",
            app.form.edit_buffer.as_deref().unwrap_or(""),
            consts::EDIT_CURSOR
        );
    }
    let mut value = schema::display_value(&app.form.device, field);
    if let Some(label) = identity_choice_label(app, field, &value) {
        value = label;
    }
    if value.is_empty() {
        return consts::FIELD_UNSET.to_string();
    }
    value
}

fn form_column_constraints() -> [Constraint; consts::FORM_COLUMN_COUNT] {
    [
        Constraint::Length(consts::FIELD_LABEL_WIDTH as u16),
        Constraint::Length(consts::FORM_VALUE_WIDTH),
        Constraint::Length(consts::FORM_WIDGET_WIDTH),
    ]
}

fn field_table_row(
    field: FieldId,
    value: &str,
    device: &DeviceEntry,
    output_slot: usize,
    pan_focused: bool,
) -> Row<'static> {
    Row::new([
        Cell::from(field.label().to_string()),
        Cell::from(value.to_string()),
        Cell::from(field_widget(field, device, output_slot, pan_focused)),
    ])
}

fn form_notice_row(text: String, style: Style) -> Row<'static> {
    Row::new([
        Cell::from(""),
        Cell::from(text).style(style),
        Cell::from(""),
    ])
}

fn field_widget(
    field: FieldId,
    device: &DeviceEntry,
    output_slot: usize,
    pan_focused: bool,
) -> Line<'static> {
    if field == FieldId::Pan {
        return pan_widget(device, output_slot, pan_focused);
    }
    let Some(ratio) = diagrams::field_ratio(device, field) else {
        return Line::default();
    };
    Line::from(Span::styled(
        theme::bar(ratio, consts::GAUGE_WIDTH),
        theme::style_ok(),
    ))
}

fn pan_widget(device: &DeviceEntry, output_slot: usize, pan_focused: bool) -> Line<'static> {
    let channels = schema::sound_channel_count(device);
    let mask = schema::sound_resolve_channel_mask(device);
    let focus = if pan_focused { Some(output_slot) } else { None };
    diagrams::pan_compact_line(mask, channels, focus)
}

fn draw_form_diagram(frame: &mut Frame, area: Rect, app: &App) {
    let lines = diagrams::device_diagram_lines(&app.form.device);
    frame.render_widget(
        Paragraph::new(lines).block(theme::panel(consts::DIAGRAM_TITLE_PREVIEW)),
        area,
    );
}

fn draw_profile_edit(frame: &mut Frame, area: Rect, app: &App) {
    let name = if app.profile_edit.name.is_empty() {
        consts::FIELD_UNSET.to_string()
    } else {
        app.profile_edit.name.clone()
    };
    let items = vec![
        ListItem::new(format!("{}:  {name}", consts::KEY_PROFILE_NAME)),
        ListItem::new(Line::from(theme::help_spans(consts::PROFILE_HELP))),
    ];
    render_selectable_list(
        frame,
        area,
        consts::TITLE_PROFILE_SHARED,
        items,
        app.profile_field,
    );
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
        ConfirmKind::DiscardUnsaved => consts::CONFIRM_DISCARD_UNSAVED,
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

fn draw_confirm_background(frame: &mut Frame, area: Rect, app: &App, diag: &Diagnostics) {
    match &app.previous {
        Screen::Dashboard => draw_dashboard(frame, area, app, diag),
        Screen::Devices => draw_devices(frame, area, app),
        Screen::Settings => draw_settings(frame, area, app),
        Screen::Telemetry => draw_telemetry(frame, area, app),
        Screen::Logs => draw_logs(frame, area, app),
        Screen::DeviceForm => draw_form(frame, area, app, false),
        Screen::DeviceTune => draw_form(frame, area, app, true),
        Screen::ProfileEdit => draw_profile_edit(frame, area, app),
        Screen::TemplatePicker => draw_templates(frame, area, app),
        Screen::SettingsSub(sub) => draw_settings_sub(frame, area, app, *sub),
        Screen::Confirm(_) => draw_devices(frame, area, app),
    }
}

fn draw_settings(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = consts::SETTINGS_ITEMS
        .iter()
        .map(|label| ListItem::new(*label))
        .collect();
    let help = consts::SETTINGS_ITEM_HELP
        .get(app.settings_index)
        .copied()
        .unwrap_or("");
    let title = consts::SETTINGS_ITEMS
        .get(app.settings_index)
        .copied()
        .unwrap_or(consts::TITLE_ABOUT);
    draw_list_with_help(
        frame,
        area,
        bound_panel_title(consts::TITLE_SETTINGS, app),
        items,
        app.settings_index,
        title,
        help,
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
    let fps = match flags.fps {
        Some(value) => value.to_string(),
        None => consts::FIELD_UNSET.to_string(),
    };
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
            "{:<width$} {fps}",
            consts::FLAG_LABEL_FPS,
            width = consts::FIELD_LABEL_WIDTH
        ),
        format!(
            "{:<width$} {}",
            consts::FLAG_LABEL_LOG,
            flags
                .log_file
                .as_deref()
                .unwrap_or(consts::FLAG_LOG_DEFAULT),
            width = consts::FIELD_LABEL_WIDTH
        ),
    ];
    let items: Vec<ListItem> = rows.iter().map(|row| ListItem::new(row.clone())).collect();
    let help = consts::FLAG_HELP
        .get(app.flags_field)
        .copied()
        .unwrap_or("");
    draw_list_with_help(
        frame,
        area,
        bound_panel_title(consts::TITLE_FLAGS, app),
        items,
        app.flags_field,
        consts::TITLE_ABOUT,
        help,
    );
}

fn draw_simd(frame: &mut Frame, area: Rect, app: &App) {
    let columns = simd_config::table_columns(&app.simd);
    let field = columns
        .get(app.simd_field)
        .map(String::as_str)
        .unwrap_or(consts::TITLE_ABOUT);
    let help = simd_config::field_help(field);
    let Some((table, side)) = split_simd_help(area) else {
        render_simd_table(frame, area, app, &columns);
        return;
    };
    render_simd_table(frame, table, app, &columns);
    draw_about_panel(frame, side, field, help);
}

fn split_simd_help(area: Rect) -> Option<(Rect, Rect)> {
    if area.height < consts::LAYOUT_SIMD_TABLE_MIN + consts::LAYOUT_SIMD_HELP_HEIGHT {
        return None;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(consts::LAYOUT_SIMD_TABLE_MIN),
            Constraint::Length(consts::LAYOUT_SIMD_HELP_HEIGHT),
        ])
        .split(area);
    Some((chunks[0], chunks[1]))
}

fn render_simd_table(frame: &mut Frame, area: Rect, app: &App, columns: &[String]) {
    let budget = simd_table_inner_width(area);
    let widths = simd_config::column_widths(&app.simd, columns);
    let (col_start, col_end) = simd_config::column_window(&widths, app.simd_field, budget);
    let visible = columns.get(col_start..col_end).unwrap_or(&[]);
    let header = Row::new(
        visible
            .iter()
            .enumerate()
            .map(|(offset, key)| simd_header_cell(key, col_start + offset == app.simd_field)),
    );
    let mut rows = Vec::new();
    if app.simd.sims.is_empty() {
        rows.push(Row::new([Cell::from(consts::SIMD_EMPTY)]));
    }
    for sim in &app.simd.sims {
        let cells = visible.iter().map(|key| Cell::from(sim.field_display(key)));
        rows.push(Row::new(cells));
    }
    let constraints: Vec<Constraint> = (col_start..col_end)
        .map(|index| simd_column_length(widths.get(index).copied().unwrap_or(0), budget))
        .collect();
    let table = Table::new(rows, constraints)
        .header(header)
        .column_spacing(consts::SIMD_COL_SPACING)
        .block(theme::panel(bound_panel_title(consts::TITLE_SIMD, app)))
        .row_highlight_style(theme::style_selected())
        .cell_highlight_style(theme::style_selected())
        .highlight_symbol(consts::LIST_HIGHLIGHT_SYMBOL);
    let mut state = TableState::default();
    if !app.simd.sims.is_empty() {
        state.select(Some(app.simd_index.min(app.simd.sims.len() - 1)));
        if !visible.is_empty() {
            state.select_column(Some(app.simd_field.saturating_sub(col_start)));
        }
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn simd_table_inner_width(area: Rect) -> u16 {
    area.width
        .saturating_sub(consts::LAYOUT_BORDER_LINES)
        .saturating_sub(consts::LIST_HIGHLIGHT_WIDTH)
}

fn simd_header_cell(key: &str, selected: bool) -> Cell<'static> {
    let style = if selected {
        theme::style_title()
    } else {
        theme::style_muted()
    };
    Cell::from(key.to_string()).style(style)
}

fn simd_column_length(width: u16, budget: u16) -> Constraint {
    Constraint::Length(width.min(budget))
}

fn draw_lua(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = consts::BUNDLED_LUA
        .iter()
        .map(|name| ListItem::new(*name))
        .collect();
    let name = consts::BUNDLED_LUA
        .get(app.lua_index)
        .copied()
        .unwrap_or(consts::TITLE_ABOUT);
    let help = lua_item_help(app.lua_index);
    draw_list_with_help(
        frame,
        area,
        consts::TITLE_LUA,
        items,
        app.lua_index,
        name,
        &help,
    );
}

fn lua_item_help(index: usize) -> String {
    let intro = consts::SETTINGS_ITEM_HELP[consts::SETTINGS_INDEX_LUA];
    let Some(name) = consts::BUNDLED_LUA.get(index) else {
        return intro.to_string();
    };
    let dest = crate::paths::config_home()
        .join(consts::CONFIG_DIR_NAME)
        .join(name);
    if dest.is_file() {
        return format!("{intro} Copied to {}.", dest.display());
    }
    intro.to_string()
}

fn draw_tach(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(consts::LAYOUT_BORDER_LINES + 2),
        ])
        .split(area);
    let rows = [
        format!(
            "{:<width$} {}",
            consts::TACH_LABEL_MAX_REVS,
            app.tach_max_revs,
            width = consts::FIELD_LABEL_WIDTH
        ),
        format!(
            "{:<width$} {}",
            FieldId::Granularity.label(),
            app.tach_granularity,
            width = consts::FIELD_LABEL_WIDTH
        ),
        format!(
            "{:<width$} {}",
            consts::TACH_LABEL_OUTPUT,
            app.tach_path,
            width = consts::FIELD_LABEL_WIDTH
        ),
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
        Line::from(diagrams::simapi_span(d.simapi_exists, d.simapi_live)),
        Line::from(format!(
            "{} {}  {} {}",
            consts::LABEL_CONNECTED,
            d.connected,
            consts::LABEL_MISSING,
            d.missing
        )),
        Line::from(format!(
            "{} {}",
            consts::CONFIG_FILE_NAME,
            app.config_path.display()
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(theme::panel(consts::TITLE_DIAGNOSTICS)),
        area,
    );
}

fn draw_raw(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = Vec::new();
    if !app.raw_on_disk.is_empty() {
        for line in app.raw_on_disk.lines() {
            lines.push(Line::from(line.to_string()));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::panel(consts::TITLE_RAW)),
        area,
    );
}

fn draw_logs(frame: &mut Frame, area: Rect, app: &App) {
    let visible = app.logs.visible();
    let start = visible.len().saturating_sub(
        area.height.saturating_sub(consts::LAYOUT_BORDER_LINES) as usize + app.logs.scroll,
    );
    let lines: Vec<Line> = visible
        .iter()
        .skip(start)
        .map(|line| {
            let style = log_line_style(&line.text);
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

fn log_line_style(text: &str) -> Style {
    if logs::is_error_line(text) {
        return theme::style_error();
    }
    if logs::is_warn_line(text) {
        return theme::style_warn();
    }
    if logs::is_info_line(text) {
        return theme::style_ok();
    }
    theme::style_muted()
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
        render_dump_size(
            app,
            consts::MIN_TERMINAL_WIDTH + 20,
            consts::MIN_TERMINAL_HEIGHT + 6,
        )
    }

    fn render_dump_size(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().to_string()
    }

    fn widget_column(dump: &str, label: &str) -> usize {
        for line in dump.lines() {
            if !line.contains(label) {
                continue;
            }
            if let Some(index) = line.find(consts::GAUGE_FILL) {
                return index;
            }
            if let Some(index) = line.find(consts::GAUGE_EMPTY) {
                return index;
            }
        }
        panic!("no slider widget on a {label} row:\n{dump}");
    }

    fn with_app<F: FnOnce(&mut App)>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(consts::ENV_XDG_CONFIG_HOME, dir.path().join("config"));
        std::env::set_var(consts::ENV_XDG_CACHE_HOME, dir.path().join("cache"));
        std::env::set_var(consts::ENV_XDG_STATE_HOME, dir.path().join("state"));
        let mut app = App::new().unwrap();
        f(&mut app);
    }

    const SIMD_SCROLL_TAIL: &str = "ZzzScrollTail";

    fn simd_named(name: &str) -> simd_config::SimdSim {
        simd_with_game(name, 0)
    }

    fn simd_with_game(name: &str, game_id: i64) -> simd_config::SimdSim {
        simd_config::SimdSim {
            settings: vec![
                (
                    consts::SIMD_FIELD_NAME.to_string(),
                    crate::libconfig::Value::String(name.to_string()),
                ),
                (
                    consts::SIMD_FIELD_GAMEID.to_string(),
                    crate::libconfig::Value::Int(game_id),
                ),
            ],
        }
    }

    fn pad_simd_until(app: &mut App, count: usize) {
        while app.simd.sims.len() < count {
            let index = app.simd.sims.len();
            app.simd.sims.push(simd_named(&format!("Pad{index}")));
        }
    }

    fn header_contains(dump: &str, column: &str) -> bool {
        dump.lines().any(|line| {
            if !line.contains(column) {
                return false;
            }
            consts::SIMD_TABLE_COLUMNS
                .iter()
                .filter(|key| line.contains(**key))
                .count()
                >= 2
        })
    }

    fn wheel(kind: crossterm::event::MouseEventKind) -> crossterm::event::Event {
        wheel_with(kind, crossterm::event::KeyModifiers::NONE)
    }

    fn wheel_shift(kind: crossterm::event::MouseEventKind) -> crossterm::event::Event {
        wheel_with(kind, crossterm::event::KeyModifiers::SHIFT)
    }

    fn wheel_with(
        kind: crossterm::event::MouseEventKind,
        modifiers: crossterm::event::KeyModifiers,
    ) -> crossterm::event::Event {
        crossterm::event::Event::Mouse(crossterm::event::MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers,
        })
    }

    #[test]
    fn dashboard_shows_tabs_and_status() {
        with_app(|app| {
            let dump = render_dump(app);
            assert!(dump.contains(consts::TAB_TITLES[consts::TAB_DASHBOARD]));
            assert!(dump.contains(consts::BINARY_SIMD));
            assert!(dump.contains(consts::DIAGRAM_TITLE_PIPELINE));
            assert!(dump.contains(consts::TAB_ACTIVE_LEFT));
            assert!(dump.contains(consts::LABEL_PROFILE), "{dump}");
        });
    }

    #[test]
    fn dashboard_shows_live_play_session_state() {
        with_app(|app| {
            let session_label = format!("{} ", consts::LABEL_PLAY_SESSION);
            assert!(!render_dump(app).contains(consts::LABEL_PLAY_OVERRUNS));
            app.play_status = Some(crate::control::PlayStatus {
                ok: true,
                state: "mapping".into(),
                devices: 2,
                overruns: 1,
                paused: true,
                ..Default::default()
            });
            let dump = render_dump(app);
            assert!(dump.contains(&format!("{session_label}mapping")), "{dump}");
            assert!(dump.contains(&format!("2 {}", consts::LABEL_PLAY_DEVICES)), "{dump}");
            assert!(dump.contains(&format!("1 {}", consts::LABEL_PLAY_OVERRUNS)), "{dump}");
            assert!(dump.contains(consts::LABEL_PLAY_PAUSED), "{dump}");
        });
    }

    #[test]
    fn devices_and_settings_render() {
        with_app(|app| {
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::Devices;
            let dump = render_dump(app);
            assert!(
                dump.contains(consts::TITLE_PROFILE) || dump.contains(consts::TITLE_NO_PROFILES)
            );
            assert!(dump.contains(consts::TITLE_DEVICES), "{dump}");
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::Settings;
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_SETTINGS));
            assert!(dump.contains("Play/test flags for the sim"));
            app.tab = consts::TAB_TELEMETRY;
            app.screen = Screen::Telemetry;
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_TELEMETRY_SESSION), "{dump}");
            assert!(dump.contains(consts::TITLE_TELEMETRY_CONTROLS), "{dump}");
            assert!(dump.contains(consts::LABEL_RPM), "{dump}");
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::DeviceForm;
            let dump = render_dump(app);
            assert!(dump.contains("Transport:"));
            assert!(dump.contains(consts::TITLE_DEVICE_TEST), "{dump}");
            assert!(dump.contains(consts::TEST_PANEL_IDLE), "{dump}");
        });
    }

    #[test]
    fn action_hints_match_actions() {
        assert_eq!(consts::ACTION_HINTS.len(), consts::DASHBOARD_ACTIONS.len());
    }

    #[test]
    fn pressing_t_stops_a_running_test() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var(consts::ENV_XDG_CONFIG_HOME, dir.path().join("config"));
        std::env::set_var(consts::ENV_XDG_CACHE_HOME, dir.path().join("cache"));
        std::env::set_var(consts::ENV_XDG_STATE_HOME, dir.path().join("state"));
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let stub = bin.join(consts::BINARY_CARGOPIT);
        std::fs::write(
            &stub,
            "#!/bin/sh\necho stub-test\ntrap 'exit 0' TERM\nexec sleep 30\n",
        )
        .unwrap();
        let mut perms = std::fs::metadata(&stub).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        std::fs::set_permissions(&stub, perms).unwrap();
        let previous_path = std::env::var_os(consts::ENV_PATH);
        std::env::set_var(consts::ENV_PATH, &bin);
        let mut app = App::new().unwrap();
        let key =
            |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
        app.handle_key(key(consts::KEY_TEST)).unwrap();
        assert!(app.test_running(), "{}", app.message);
        app.handle_key(key(consts::KEY_TEST)).unwrap();
        assert!(!app.test_running(), "{}", app.message);
        assert_eq!(app.message, consts::MSG_STOPPED_TEST);
        match previous_path {
            Some(path) => std::env::set_var(consts::ENV_PATH, path),
            None => std::env::remove_var(consts::ENV_PATH),
        }
    }

    #[test]
    fn test_logs_strip_ansi_and_keep_real_errors() {
        with_app(|app| {
            app.logs.push(
                "test".into(),
                "\u{1b}[32m<info>\u{1b}[0m running cargopit in test mode...".into(),
            );
            app.logs.push(
                "test".into(),
                "\u{1b}[31m<error>\u{1b}[0m Error opening serial port".into(),
            );
            app.logs.push("test".into(), "Green Flag!".into());
            app.tab = consts::TAB_LOGS;
            app.screen = Screen::Logs;
            let dump = render_dump(app);
            assert!(dump.contains(consts::SLOG_TAG_INFO), "{dump}");
            assert!(dump.contains(consts::SLOG_TAG_ERROR), "{dump}");
            assert!(dump.contains("Green Flag!"), "{dump}");
            assert!(!dump.contains('\u{1b}'), "{dump}");
        });
    }

    #[test]
    fn settings_help_matches_items() {
        assert_eq!(
            consts::SETTINGS_ITEMS.len(),
            consts::SETTINGS_ITEM_HELP.len()
        );
        assert_eq!(consts::FLAG_HELP.len(), consts::FLAG_FIELD_COUNT);
        assert_eq!(
            consts::FPS_FLAG_CHOICES.len(),
            consts::FPS_FLAG_CHOICE_COUNT
        );
        assert_eq!(
            consts::SIMD_TABLE_COLUMNS.len(),
            consts::SIMD_FIELD_HELP.len()
        );
        assert_eq!(consts::SIMD_TABLE_COLUMNS.len(), consts::SIMD_FIELD_COUNT);
    }

    #[test]
    fn flags_fps_can_return_to_unset() {
        with_app(|app| {
            app.screen = Screen::SettingsSub(SettingsSub::Flags);
            app.flags_field = consts::FLAG_FIELD_FPS;
            let key =
                |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            app.handle_key(key(consts::KEY_RIGHT)).unwrap();
            assert_eq!(app.tui_state.play_flags.fps, Some(consts::FPS_FLAG_30));
            app.handle_key(key(consts::KEY_LEFT)).unwrap();
            assert_eq!(app.tui_state.play_flags.fps, None);
            app.handle_key(key(consts::KEY_RIGHT)).unwrap();
            app.handle_key(key(consts::KEY_BACKSPACE)).unwrap();
            assert_eq!(app.tui_state.play_flags.fps, None);
        });
    }

    #[test]
    fn flags_fps_cycles_through_high_rates() {
        with_app(|app| {
            app.screen = Screen::SettingsSub(SettingsSub::Flags);
            app.flags_field = consts::FLAG_FIELD_FPS;
            let key =
                |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            for expected in consts::FPS_FLAG_CHOICES.iter().skip(1) {
                app.handle_key(key(consts::KEY_RIGHT)).unwrap();
                assert_eq!(app.tui_state.play_flags.fps, *expected);
            }
            assert_eq!(app.tui_state.play_flags.fps, Some(consts::FPS_FLAG_500));
            app.handle_key(key(consts::KEY_RIGHT)).unwrap();
            assert_eq!(app.tui_state.play_flags.fps, None);
        });
    }

    #[test]
    fn simd_renders_game_table() {
        with_app(|app| {
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::SettingsSub(SettingsSub::Simd);
            app.simd = simd_config::stub_from_bundled();
            assert!(!app.simd.sims.is_empty());
            for (width, height) in [
                (100u16, 28u16),
                (consts::MIN_TERMINAL_WIDTH, consts::MIN_TERMINAL_HEIGHT),
            ] {
                let dump = render_dump_size(app, width, height);
                assert!(dump.contains(consts::TITLE_SIMD), "{dump}");
                assert!(dump.contains(consts::SIMD_FIELD_NAME), "{dump}");
                assert!(dump.contains(consts::SIMD_FIELD_GAMEID), "{dump}");
                assert!(dump.contains(consts::SIMD_FIELD_LAUNCHEXE), "{dump}");
                assert!(dump.contains("AssettoCorsaCompetizione"), "{dump}");
                assert!(
                    !dump.contains("name = AssettoCorsa  ("),
                    "inline field help mixed into list:\n{dump}"
                );
            }
            let key =
                |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            app.handle_key(key(consts::KEY_RIGHT)).unwrap();
            assert_eq!(app.simd_field, 1);
            let dump = render_dump(app);
            assert!(dump.contains("Steam app id"), "{dump}");
        });
    }

    #[test]
    fn simd_table_keeps_selected_column_visible() {
        with_app(|app| {
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::SettingsSub(SettingsSub::Simd);
            app.simd = simd_config::stub_from_bundled();
            let columns = simd_config::table_columns(&app.simd);
            app.simd_field = columns
                .iter()
                .position(|column| column == consts::SIMD_FIELD_TELEMETRY)
                .expect("telemetry column");
            let dump =
                render_dump_size(app, consts::MIN_TERMINAL_WIDTH, consts::MIN_TERMINAL_HEIGHT);
            assert!(dump.contains(consts::SIMD_FIELD_TELEMETRY), "{dump}");
            assert!(
                !header_contains(&dump, consts::SIMD_FIELD_NAME),
                "name column should scroll off on a narrow terminal:\n{dump}"
            );
        });
    }

    #[test]
    fn simd_table_row_keys_do_not_wrap() {
        with_app(|app| {
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::SettingsSub(SettingsSub::Simd);
            app.simd = simd_config::stub_from_bundled();
            pad_simd_until(app, consts::SIMD_PAGE_ROWS.saturating_mul(3));
            app.simd.sims.push(simd_named(SIMD_SCROLL_TAIL));
            let last = app.simd.sims.len() - 1;
            let key =
                |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            app.handle_key(key(consts::KEY_END)).unwrap();
            assert_eq!(app.simd_index, last);
            app.handle_key(key(consts::KEY_DOWN)).unwrap();
            assert_eq!(app.simd_index, last);
            app.handle_key(key(consts::KEY_HOME)).unwrap();
            assert_eq!(app.simd_index, 0);
            app.handle_key(key(consts::KEY_UP)).unwrap();
            assert_eq!(app.simd_index, 0);
            app.handle_key(key(consts::KEY_END)).unwrap();
            let dump =
                render_dump_size(app, consts::MIN_TERMINAL_WIDTH, consts::MIN_TERMINAL_HEIGHT);
            assert!(dump.contains(SIMD_SCROLL_TAIL), "{dump}");
        });
    }

    #[test]
    fn simd_table_mouse_wheel_pans_columns() {
        with_app(|app| {
            app.screen = Screen::SettingsSub(SettingsSub::Simd);
            app.simd = simd_config::stub_from_bundled();
            assert!(app.simd.sims.len() > 1);
            app.handle_event(wheel(crossterm::event::MouseEventKind::ScrollDown))
                .unwrap();
            assert_eq!(app.simd_field, 1);
            assert_eq!(app.simd_index, 0);
            app.handle_event(wheel(crossterm::event::MouseEventKind::ScrollUp))
                .unwrap();
            assert_eq!(app.simd_field, 0);
            app.handle_event(wheel_shift(crossterm::event::MouseEventKind::ScrollDown))
                .unwrap();
            assert_eq!(app.simd_index, 1);
            assert_eq!(app.simd_field, 0);
        });
    }

    #[test]
    fn lua_lists_bundled_names_not_source_paths() {
        with_app(|app| {
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::SettingsSub(SettingsSub::Lua);
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_LUA), "{dump}");
            assert!(dump.contains(consts::BUNDLED_LUA[0]), "{dump}");
            assert!(
                !dump.contains("/conf/"),
                "lua list should not dump bundled file paths:\n{dump}"
            );
        });
    }

    #[test]
    fn diagnostics_does_not_reuse_raw_hint() {
        with_app(|app| {
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::SettingsSub(SettingsSub::Diagnostics);
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_DIAGNOSTICS), "{dump}");
            assert!(!dump.contains(consts::RAW_VIEW_HINT), "{dump}");
            app.screen = Screen::SettingsSub(SettingsSub::Raw);
            let dump = render_dump(app);
            assert!(dump.contains(consts::RAW_VIEW_HINT), "{dump}");
        });
    }

    #[test]
    #[ignore = "requires a live simd/cargopit play session"]
    fn live_session_dumps_settings_subs() {
        let out = std::env::temp_dir().join("cargopit-tui-live");
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        let mut app = App::new().unwrap();
        app.tick();
        assert!(
            app.status.simd_running,
            "expected live simd; start simd before this test"
        );
        assert!(
            app.status.cargopit_running,
            "expected live cargopit play; this test must not start or stop it"
        );
        assert!(
            !app.simd.sims.is_empty(),
            "expected ~/.config/simd/simd.config to parse"
        );

        let key =
            |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
        app.handle_key(key(consts::KEY_ENTER)).unwrap();
        assert_eq!(app.message, consts::MSG_PLAY_ALREADY_RUNNING);

        let mut dumps = Vec::new();
        let save = |name: &str, app: &App, dumps: &mut Vec<(String, String)>| {
            let dump = render_dump_size(app, 100, 28);
            std::fs::write(out.join(format!("{name}.txt")), &dump).unwrap();
            dumps.push((name.to_string(), dump));
        };

        save("dashboard", &app, &mut dumps);
        app.handle_key(key(consts::KEY_TAB3)).unwrap();
        save("settings", &app, &mut dumps);

        let subs = [
            ("flags", "Play / test flags"),
            ("simd", consts::TITLE_SIMD),
            ("lua", consts::TITLE_LUA),
            ("tach", consts::TITLE_TACH),
            ("tyres", consts::TITLE_TYRES),
            ("diagnostics", consts::TITLE_DIAGNOSTICS),
            ("raw", consts::TITLE_RAW),
        ];
        for (index, (name, title)) in subs.iter().enumerate() {
            app.settings_index = index;
            app.handle_key(key(consts::KEY_ENTER)).unwrap();
            save(name, &app, &mut dumps);
            let dump = &dumps.last().unwrap().1;
            assert!(dump.contains(title), "{name} missing {title}:\n{dump}");
            app.handle_key(key(consts::KEY_ESC)).unwrap();
            assert!(matches!(app.screen, Screen::Settings));
        }

        let simd = dumps
            .iter()
            .find(|(name, _)| name == "simd")
            .map(|(_, dump)| dump.as_str())
            .unwrap();
        assert!(simd.contains(&app.simd.sims[0].display_name()), "{simd}");
        assert!(simd.contains(consts::SIMD_FIELD_GAMEID), "{simd}");
        assert!(
            !simd.contains("name = "),
            "simd still dumps inline field rows:\n{simd}"
        );

        let lua = dumps
            .iter()
            .find(|(name, _)| name == "lua")
            .map(|(_, dump)| dump.as_str())
            .unwrap();
        assert!(lua.contains(consts::BUNDLED_LUA[0]), "{lua}");
        assert!(!lua.contains("/conf/"), "lua listed source paths:\n{lua}");

        for (name, dump) in &dumps {
            eprintln!("===== live {name} =====\n{dump}");
        }
    }

    #[test]
    fn devices_notice_that_profiles_are_shared() {
        with_app(|app| {
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::Devices;
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_PROFILE_SHARED), "{dump}");
            assert!(dump.contains("Profiles are shared"), "{dump}");
        });
    }

    #[test]
    fn settings_title_names_the_bound_game() {
        with_app(|app| {
            app.tab = consts::TAB_SETTINGS;
            app.screen = Screen::Settings;
            app.running_games.clear();
            app.tui_state.settings_game_id = consts::SETTINGS_GAME_IDLE;
            let idle = render_dump(app);
            assert!(idle.contains(consts::TITLE_SETTINGS), "{idle}");
            assert!(idle.contains(consts::SETTINGS_BOUND_IDLE), "{idle}");
            app.simd.sims = vec![simd_with_game("BoundSim", 42)];
            app.tui_state.settings_game_id = 42;
            let live = render_dump(app);
            assert!(live.contains("BoundSim"), "{live}");
        });
    }

    #[test]
    fn comma_and_period_cycle_profiles() {
        with_app(|app| {
            app.config
                .profiles
                .push(crate::config::SimProfile::default());
            app.profile_index = 0;
            app.tab = consts::TAB_DASHBOARD;
            app.screen = Screen::Dashboard;
            let key =
                |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            app.handle_key(key(consts::KEY_NEXT_PROFILE)).unwrap();
            assert_eq!(app.profile_index, 1);
            app.handle_key(key(consts::KEY_PREV_PROFILE)).unwrap();
            assert_eq!(app.profile_index, 0);
            app.handle_key(key(consts::KEY_RIGHT)).unwrap();
            assert_eq!(app.profile_index, 1);
            app.handle_key(key(consts::KEY_LEFT)).unwrap();
            assert_eq!(app.profile_index, 0);
        });
    }

    #[test]
    fn shift_arrows_reorder_devices() {
        with_app(|app| {
            let mut first = DeviceEntry::new();
            first.set_str(consts::KEY_DEVID, "sink-a");
            let mut second = DeviceEntry::new();
            second.set_str(consts::KEY_DEVID, "sink-b");
            if let Some(profile) = app.current_profile_mut() {
                profile.devices = vec![first, second];
            }
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::Devices;
            app.device_index = 0;
            let key = |code, modifiers| crossterm::event::KeyEvent::new(code, modifiers);
            app.handle_key(key(consts::KEY_DOWN, crossterm::event::KeyModifiers::SHIFT))
                .unwrap();
            assert_eq!(app.device_index, 1);
            let ids: Vec<_> = app
                .current_profile()
                .unwrap()
                .devices
                .iter()
                .map(|device| device.get_str(consts::KEY_DEVID))
                .collect();
            assert_eq!(ids, vec![Some("sink-b"), Some("sink-a")]);
        });
    }

    #[test]
    fn space_confirms_and_escape_cancels() {
        with_app(|app| {
            let mut device = DeviceEntry::new();
            device.set_str(consts::KEY_DEVID, "sink-a");
            if let Some(profile) = app.current_profile_mut() {
                profile.devices = vec![device];
            }
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::Devices;
            app.device_index = 0;
            let key =
                |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            app.handle_key(key(consts::KEY_DELETE)).unwrap();
            assert!(matches!(app.screen, Screen::Confirm(_)));
            app.handle_key(key(consts::KEY_ESC)).unwrap();
            assert!(matches!(app.screen, Screen::Devices));
            assert_eq!(app.current_profile().unwrap().devices.len(), 1);
            app.handle_key(key(consts::KEY_DELETE)).unwrap();
            app.handle_key(key(consts::KEY_SPACE)).unwrap();
            assert!(matches!(app.screen, Screen::Devices));
            assert!(app.current_profile().unwrap().devices.is_empty());
        });
    }

    #[test]
    fn profile_keys_do_not_cycle_inside_device_editor() {
        with_app(|app| {
            app.config
                .profiles
                .push(crate::config::SimProfile::default());
            app.profile_index = 0;
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::DeviceForm;
            let key =
                |code| crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE);
            app.handle_key(key(consts::KEY_NEXT_PROFILE)).unwrap();
            assert_eq!(app.profile_index, 0);
            assert!(matches!(app.screen, Screen::DeviceForm));
        });
    }

    #[test]
    fn device_editor_title_names_the_profile() {
        with_app(|app| {
            if let Some(profile) = app.current_profile_mut() {
                profile.name = "Garage".into();
            }
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::DeviceForm;
            let dump = render_dump(app);
            assert!(dump.contains(consts::TITLE_DEVICE_EDITOR), "{dump}");
            assert!(dump.contains("Garage"), "{dump}");
        });
    }

    #[test]
    fn device_editor_aligns_slider_widgets() {
        with_app(|app| {
            app.form = crate::form::DeviceForm::blank();
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::DeviceForm;
            let dump = render_dump_size(app, 100, 28);
            let volume = widget_column(&dump, FieldId::Volume.label());
            let amplitude = widget_column(&dump, FieldId::Amplitude.label());
            let frequency = widget_column(&dump, FieldId::Frequency.label());
            assert_eq!(volume, amplitude, "{dump}");
            assert_eq!(volume, frequency, "{dump}");
        });
    }

    #[test]
    fn tune_hint_pins_below_the_field_table() {
        with_app(|app| {
            app.form = crate::form::DeviceForm::blank();
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::DeviceTune;
            let dump = render_dump_size(app, 120, 28);
            let hint_lead = consts::TUNING_OFFLINE_HINT
                .split_once(':')
                .map(|(lead, _)| lead)
                .unwrap_or(consts::TUNING_OFFLINE_HINT);
            assert!(dump.contains(hint_lead), "{dump}");
            for line in dump.lines() {
                if line.contains(hint_lead) {
                    assert!(
                        !line.contains(consts::GAUGE_FILL),
                        "hint must not share a row with sliders:\n{line}\n{dump}"
                    );
                }
            }
        });
    }

    #[test]
    fn device_id_help_shows_full_processor_sink_label() {
        with_app(|app| {
            let sink = "cargopit_tactile";
            let tag = format!("[{}]", consts::LABEL_SINK_PROCESSOR);
            app.discovery.pulse_sinks = vec![crate::hardware::HardwareChoice {
                value: sink.into(),
                label: format!("Cargopit tactile correction ({sink}) {tag}"),
            }];
            let mut device = crate::config::DeviceEntry::new();
            schema::apply_defaults(&mut device, schema::DeviceClass::Sound, consts::TYPE_HAPTIC);
            device.set_str(consts::KEY_DEVID, sink);
            app.form = crate::form::DeviceForm::new(device, false);
            app.form.field_index = app
                .form
                .fields()
                .iter()
                .position(|field| *field == FieldId::Devid)
                .expect("sound devices have a device id");
            app.tab = consts::TAB_DEVICES;
            app.screen = Screen::DeviceForm;
            let dump = render_dump_size(app, 120, 34);
            assert!(dump.contains(&tag), "{dump}");
        });
    }

    #[test]
    fn settings_follow_live_game_without_switching_profile() {
        with_app(|app| {
            app.config
                .profiles
                .push(crate::config::SimProfile::default());
            app.profile_index = 1;
            app.simd.sims = vec![simd_with_game("One", 11), simd_with_game("Two", 22)];
            app.simd_index = 0;
            app.tui_state.play_flags.verbosity = 2;
            app.tui_state.store_flags_for(consts::SETTINGS_GAME_IDLE);
            app.running_games = vec![simapi_shm::RunningGame {
                name: "Two".into(),
                game_id: 22,
                sending: true,
                status_label: consts::LABEL_TELEMETRY_LIVE,
            }];
            app.bind_settings_to_live_game();
            assert_eq!(app.profile_index, 1);
            assert_eq!(app.simd_index, 1);
            assert_eq!(app.tui_state.settings_game_id, 22);
            app.tui_state.play_flags.verbosity = 1;
            app.running_games[0].name = "One".into();
            app.running_games[0].game_id = 11;
            app.bind_settings_to_live_game();
            assert_eq!(app.profile_index, 1);
            assert_eq!(app.simd_index, 0);
            assert_eq!(app.tui_state.play_flags.verbosity, 2);
            app.running_games[0].name = "Two".into();
            app.running_games[0].game_id = 22;
            app.bind_settings_to_live_game();
            assert_eq!(app.tui_state.play_flags.verbosity, 1);
            assert_eq!(app.simd_index, 1);
        });
    }
}
