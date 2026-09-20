use ratatui::layout::{Constraint, Direction, Layout, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Gauge, HighlightSpacing, List, ListItem, ListState, Paragraph, Row,
    Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Tabs, Wrap,
};
use ratatui::Frame;

use crate::app::{simd_field_pairs, tune_fields, App, ConfirmKind, Screen, SettingsSub};
use crate::consts;
use crate::device_table::{self, DeviceRow};
use crate::form::InputMode;
use crate::schema;
use crate::simd_config;
use crate::templates;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();
    if size.width < consts::MIN_TERMINAL_WIDTH || size.height < consts::MIN_TERMINAL_HEIGHT {
        draw_too_small(frame, size);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(consts::LAYOUT_HEADER_HEIGHT),
            Constraint::Length(consts::LAYOUT_TAB_HEIGHT),
            Constraint::Min(1),
            Constraint::Length(consts::LAYOUT_FOOTER_HEIGHT),
        ])
        .split(size);
    draw_header(frame, chunks[0], app);
    draw_tabs(frame, chunks[1], app);
    let screen = app.screen.clone();
    match screen {
        Screen::Dashboard => draw_dashboard(frame, chunks[2], app),
        Screen::Devices => draw_devices(frame, chunks[2], app),
        Screen::Settings => draw_settings(frame, chunks[2], app),
        Screen::Logs => draw_logs(frame, chunks[2], app),
        Screen::DeviceForm => draw_form(frame, chunks[2], app),
        Screen::DeviceTune => draw_tune(frame, chunks[2], app),
        Screen::ProfileEdit => draw_profile_edit(frame, chunks[2], app),
        Screen::TemplatePicker => draw_templates(frame, chunks[2], app),
        Screen::Confirm(kind) => {
            draw_devices(frame, chunks[2], app);
            draw_confirm(frame, size, &kind);
        }
        Screen::SettingsSub(sub) => draw_settings_sub(frame, chunks[2], app, sub),
    }
    draw_footer(frame, chunks[3], app);
}

fn draw_too_small(frame: &mut Frame, area: Rect) {
    let text = format!(
        "{} — minimum {}x{}",
        consts::TOO_SMALL_TITLE,
        consts::MIN_TERMINAL_WIDTH,
        consts::MIN_TERMINAL_HEIGHT
    );
    frame.render_widget(Paragraph::new(text), area);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let simd = status_span("simd", app.status.simd_running);
    let pit = status_span(consts::BINARY_CARGOPIT, app.status.cargopit_running);
    let line = Line::from(vec![
        Span::styled(consts::APP_TITLE, style_title()),
        Span::raw("   "),
        simd,
        Span::raw("  "),
        pit,
        Span::raw("  "),
        Span::raw(app.discovery_state.label()),
        Span::raw("  "),
        Span::raw(app.message.as_str()),
    ]);
    frame.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

fn status_span(name: &str, running: bool) -> Span<'static> {
    let (label, color) = if running {
        (format!("{name} {}", consts::STATUS_RUNNING), Color::Green)
    } else {
        (format!("{name} {}", consts::STATUS_STOPPED), Color::Red)
    };
    Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD))
}

fn style_title() -> Style {
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

fn style_selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
}

fn style_error() -> Style {
    Style::default().fg(Color::Red)
}

fn style_footer() -> Style {
    Style::default().fg(Color::Gray)
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
        .block(Block::default().borders(Borders::ALL).title(title.into()))
        .highlight_style(style_selected())
        .highlight_symbol(consts::LIST_HIGHLIGHT_SYMBOL);
    let mut state = ListState::default();
    if count > 0 {
        state.select(Some(selected.min(count.saturating_sub(1))));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = consts::TAB_TITLES.iter().map(|t| Line::from(*t)).collect();
    let tabs = Tabs::new(titles)
        .select(app.tab)
        .highlight_style(style_selected())
        .block(Block::default().borders(Borders::ALL).title("Tabs"));
    frame.render_widget(tabs, area);
}

fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    let diag = app.diagnostics();
    let simapi = if !diag.simapi_exists {
        "SIMAPI.DAT missing"
    } else if diag.simapi_nonzero {
        "SIMAPI.DAT live"
    } else {
        "SIMAPI.DAT zeroed"
    };
    let info = vec![
        Line::from(format!(
            "configured {}  connected {}  missing {}",
            app.current_profile().map(|p| p.devices.len()).unwrap_or(0),
            diag.connected,
            diag.missing
        )),
        Line::from(simapi),
        Line::from(""),
        Line::from("Actions (Enter):"),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(consts::LAYOUT_DASHBOARD_INFO_MIN),
            Constraint::Length(consts::DASHBOARD_ACTIONS.len() as u16 + consts::LAYOUT_BORDER_LINES),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(info).block(Block::default().borders(Borders::ALL).title("Dashboard")),
        chunks[0],
    );
    let items: Vec<ListItem> = consts::DASHBOARD_ACTIONS
        .iter()
        .map(|action| ListItem::new(*action))
        .collect();
    render_selectable_list(frame, chunks[1], "Select", items, app.dashboard_index);
}

fn draw_devices(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(profile) = app.current_profile() else {
        frame.render_widget(
            Paragraph::new("No profiles").block(Block::default().borders(Borders::ALL).title("Devices")),
            area,
        );
        return;
    };
    let title = format!(
        "Profile {}/{}  {} / {}  discovery {}  [ ] switch  s edit  a add  D delete",
        app.profile_index + 1,
        app.config.profiles.len(),
        profile.sim,
        profile.car,
        app.discovery_state.label()
    );
    if profile.devices.is_empty() {
        frame.render_widget(
            Paragraph::new("No devices. Press a to add, T for a template.")
                .block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(consts::LAYOUT_DEVICE_DETAIL_HEIGHT),
        ])
        .split(area);
    let rows: Vec<DeviceRow> = profile
        .devices
        .iter()
        .map(|device| {
            let presence = app.discovery.presence(
                device.class(),
                device.get_str(consts::KEY_DEVID).unwrap_or(""),
                device.get_str(consts::KEY_DEVPATH).unwrap_or(""),
            );
            device_table::device_row(device, presence)
        })
        .collect();
    draw_device_table(frame, chunks[0], app, title, &rows);
    draw_device_detail(frame, chunks[1], app, &rows);
}

fn draw_device_table(
    frame: &mut Frame,
    area: Rect,
    app: &mut App,
    title: String,
    rows: &[DeviceRow],
) {
    let header = Row::new([
        Cell::from(consts::DEVICE_COL_ENABLED),
        Cell::from(consts::DEVICE_COL_SUMMARY),
        Cell::from(consts::DEVICE_COL_IDENTITY),
        Cell::from(consts::DEVICE_COL_PRESENCE),
    ])
    .style(style_title())
    .height(1);
    let table_rows = rows.iter().map(|row| {
        Row::new([
            Cell::from(row.enabled),
            Cell::from(row.summary.clone()),
            Cell::from(row.identity.clone()),
            Cell::from(row.presence.clone()),
        ])
    });
    let table = Table::new(
        table_rows,
        [
            Constraint::Length(consts::DEVICE_COL_ENABLED_WIDTH),
            Constraint::Min(consts::DEVICE_COL_SUMMARY_MIN),
            Constraint::Min(consts::DEVICE_COL_IDENTITY_MIN),
            Constraint::Length(consts::DEVICE_COL_PRESENCE_WIDTH),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(style_selected())
    .highlight_symbol(consts::LIST_HIGHLIGHT_SYMBOL)
    .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(table, area, &mut app.device_table);
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
        area.inner(Margin {
            vertical: consts::LAYOUT_SCROLLBAR_MARGIN,
            horizontal: 0,
        }),
        &mut app.device_scroll,
    );
}

fn draw_device_detail(frame: &mut Frame, area: Rect, app: &App, rows: &[DeviceRow]) {
    let Some(row) = rows.get(app.device_index) else {
        frame.render_widget(
            Paragraph::new("Nothing selected...").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(consts::DEVICE_DETAIL_TITLE),
            ),
            area,
        );
        return;
    };
    let lines = vec![
        Line::from(format!("{}  {}", row.enabled, row.summary)),
        Line::from(format!("identity  {}", row.identity)),
        Line::from(format!("present   {}", row.presence)),
        Line::from(""),
        Line::from("space enable  e edit  y dup  d del  Enter tune  J/K move"),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(consts::DEVICE_DETAIL_TITLE),
        ),
        area,
    );
}

fn draw_form(frame: &mut Frame, area: Rect, app: &App) {
    if !app.form.is_editing() {
        draw_form_fields(frame, area, app, false);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(consts::LAYOUT_FORM_INPUT_HEIGHT)])
        .split(area);
    draw_form_fields(frame, chunks[0], app, false);
    draw_form_input(frame, chunks[1], app);
}

fn draw_form_fields(frame: &mut Frame, area: Rect, app: &App, tune: bool) {
    let fields = if tune {
        tune_fields(&app.form)
    } else {
        app.form.fields()
    };
    let selected = if tune { app.tune_index } else { app.form.field_index };
    let mut items = Vec::new();
    for field in fields.iter() {
        let mut value = schema::display_value(&app.form.device, *field);
        if schema::is_identity(*field) {
            let choices = app
                .discovery
                .choices_for(app.form.device.class(), *field == crate::schema::FieldId::Devpath);
            if let Some(choice) = choices.iter().find(|c| c.value == value) {
                value = choice.label.clone();
            }
        }
        items.push(ListItem::new(format!("{:16} {}", field.label(), value)));
    }
    if let Some(err) = &app.form.error {
        items.push(ListItem::new(err.clone()).style(style_error()));
    }
    if tune {
        items.push(ListItem::new(consts::TUNING_OFFLINE_HINT));
    }
    let title = if tune { "Tune (offline)" } else { "Device editor" };
    render_selectable_list(frame, area, title, items, selected);
}

fn draw_form_input(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.saturating_sub(consts::LAYOUT_BORDER_LINES + consts::INPUT_CURSOR_PADDING);
    let scroll = app.form.input.visual_scroll(width as usize);
    let input = Paragraph::new(app.form.input.value())
        .style(Style::default().fg(Color::Yellow))
        .scroll((0, scroll as u16))
        .block(Block::default().borders(Borders::ALL).title(consts::FORM_INPUT_TITLE));
    frame.render_widget(input, area);
    if app.form.input_mode != InputMode::Editing {
        return;
    }
    let cursor = app.form.input.visual_cursor().saturating_sub(scroll) as u16;
    frame.set_cursor_position(Position::new(
        area.x + consts::INPUT_CURSOR_PADDING + cursor,
        area.y + consts::INPUT_CURSOR_PADDING,
    ));
}

fn draw_tune(frame: &mut Frame, area: Rect, app: &App) {
    let fields = tune_fields(&app.form);
    let gauges = device_table::tune_gauge_specs(&app.form.device, &fields);
    if gauges.is_empty() {
        draw_form_fields(frame, area, app, true);
        return;
    }
    let gauge_height = (gauges.len() as u16).saturating_mul(consts::LAYOUT_TUNE_GAUGE_HEIGHT);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(gauge_height)])
        .split(area);
    draw_form_fields(frame, chunks[0], app, true);
    draw_tune_gauges(frame, chunks[1], &gauges);
}

fn draw_tune_gauges(frame: &mut Frame, area: Rect, gauges: &[device_table::GaugeSpec]) {
    if gauges.is_empty() {
        return;
    }
    let constraints: Vec<Constraint> = gauges
        .iter()
        .map(|_| Constraint::Length(consts::LAYOUT_TUNE_GAUGE_HEIGHT))
        .collect();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    for (index, spec) in gauges.iter().enumerate() {
        let Some(chunk) = chunks.get(index) else {
            break;
        };
        frame.render_widget(
            Gauge::default()
                .block(Block::default().borders(Borders::ALL).title(spec.title))
                .gauge_style(Style::default().fg(Color::Cyan))
                .ratio(spec.ratio)
                .label(spec.label.clone())
                .use_unicode(true),
            *chunk,
        );
    }
}

fn draw_profile_edit(frame: &mut Frame, area: Rect, app: &App) {
    let items = vec![
        ListItem::new(format!("sim:  {}  (h/l cycle)", app.profile_edit.sim)),
        ListItem::new(format!("car:  {}", app.profile_edit.car)),
        ListItem::new("a add empty profile   y duplicate current   Enter/s save   Esc cancel"),
    ];
    render_selectable_list(frame, area, "Profile", items, app.profile_field);
}

fn draw_templates(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = templates::all()
        .iter()
        .map(|template| ListItem::new(template.name))
        .collect();
    render_selectable_list(frame, area, "Templates (insert)", items, app.template_index);
}

fn draw_confirm(frame: &mut Frame, area: Rect, kind: &ConfirmKind) {
    let text = match kind {
        ConfirmKind::DeleteDevice => "Delete this device? y/n",
        ConfirmKind::DeleteProfile => "Delete this profile? y/n",
        ConfirmKind::RestartAfterSave => "Restart play to apply? y/n",
        ConfirmKind::ApplyTemplate(_) => "Insert this template into the current profile? y/n",
    };
    let popup = centered(area, consts::CONFIRM_POPUP_WIDTH, consts::CONFIRM_POPUP_HEIGHT);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Confirm")
                .style(style_selected()),
        ),
        popup,
    );
}

fn draw_settings(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = consts::SETTINGS_ITEMS
        .iter()
        .map(|label| ListItem::new(*label))
        .collect();
    render_selectable_list(frame, area, "Settings", items, app.settings_index);
}

fn draw_settings_sub(frame: &mut Frame, area: Rect, app: &mut App, sub: SettingsSub) {
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
        format!("verbosity      {}", flags.verbosity),
        format!("disable_audio  {}", flags.disable_audio),
        format!("udp            {}", flags.udp),
        format!("fps            {}", flags.fps.unwrap_or(consts::DEFAULT_FPS)),
        format!(
            "log file       {}",
            flags.log_file.as_deref().unwrap_or("(default)")
        ),
    ];
    let items: Vec<ListItem> = rows.iter().map(|row| ListItem::new(row.clone())).collect();
    render_selectable_list(frame, area, "Play / test flags", items, app.flags_field);
}

fn draw_simd(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![Line::from("a add from stub / d delete / s save / Esc back")];
    if app.simd.sims.is_empty() {
        lines.push(Line::from("No simd.config — press a to create from the simapi example."));
    }
    for (index, sim) in app.simd.sims.iter().enumerate() {
        let marker = if index == app.simd_index { ">" } else { " " };
        lines.push(Line::from(format!("{marker} {}", sim.name())));
        if index == app.simd_index {
            for (key, value) in simd_field_pairs(sim) {
                lines.push(Line::from(format!(
                    "    {key} = {value}  ({})",
                    simd_config::field_help(&key)
                )));
            }
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("simd.config")),
        area,
    );
}

fn draw_lua(frame: &mut Frame, area: Rect, app: &App) {
    let mut names: Vec<String> = consts::BUNDLED_LUA.iter().map(|s| (*s).to_string()).collect();
    names.extend(diagnostics_lua_names());
    let items: Vec<ListItem> = names.iter().map(|name| ListItem::new(name.clone())).collect();
    render_selectable_list(
        frame,
        area,
        "Lua (Enter copies bundled template into ~/.config/cargopit)",
        items,
        app.lua_index,
    );
}

fn diagnostics_lua_names() -> Vec<String> {
    crate::diagnostics::lua_scripts()
}

fn draw_tach(frame: &mut Frame, area: Rect, app: &App) {
    let rows = [
        format!("max revs     {}", app.tach_max_revs),
        format!("granularity  {}", app.tach_granularity),
        format!("output XML   {}", app.tach_path),
    ];
    let items: Vec<ListItem> = rows.iter().map(|row| ListItem::new(row.clone())).collect();
    render_selectable_list(
        frame,
        area,
        "Tachometer calibration (Enter runs cargopit config tachometer)",
        items,
        app.tach_field,
    );
}

fn draw_tyres(frame: &mut Frame, area: Rect, app: &mut App) {
    if app.tyres.cars.is_empty() {
        frame.render_widget(
            Paragraph::new("No cars. Press a to add.")
                .block(Block::default().borders(Borders::ALL).title("Tyre diameters")),
            area,
        );
        return;
    }
    let header = Row::new([
        Cell::from(consts::TYRE_COL_SIM),
        Cell::from(consts::TYRE_COL_CAR),
        Cell::from(consts::TYRE_COL_T0),
        Cell::from(consts::TYRE_COL_T1),
        Cell::from(consts::TYRE_COL_T2),
        Cell::from(consts::TYRE_COL_T3),
    ])
    .style(style_title())
    .height(1);
    let rows = app.tyres.cars.iter().map(|car| {
        let cells = device_table::tyre_cells(car);
        Row::new(cells.into_iter().map(Cell::from).collect::<Vec<_>>())
    });
    let table = Table::new(
        rows,
        [
            Constraint::Min(consts::TYRE_COL_NAME_MIN),
            Constraint::Min(consts::TYRE_COL_NAME_MIN),
            Constraint::Length(consts::TYRE_COL_VALUE_WIDTH),
            Constraint::Length(consts::TYRE_COL_VALUE_WIDTH),
            Constraint::Length(consts::TYRE_COL_VALUE_WIDTH),
            Constraint::Length(consts::TYRE_COL_VALUE_WIDTH),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Tyre diameters"))
    .row_highlight_style(style_selected())
    .highlight_symbol(consts::LIST_HIGHLIGHT_SYMBOL);
    frame.render_stateful_widget(table, area, &mut app.tyre_table);
}

fn draw_diagnostics(frame: &mut Frame, area: Rect, app: &App) {
    let d = app.diagnostics();
    let lines = vec![
        Line::from(format!("groups: {}", d.groups)),
        Line::from(format!("input {}  dialout {}  uucp {}", d.in_input, d.in_dialout, d.in_uucp)),
        Line::from(format!("udev rules: {}", d.udev_present)),
        Line::from(format!(
            "cargopit: {}",
            d.cargopit_bin.as_deref().unwrap_or("missing")
        )),
        Line::from(format!("simd: {}", d.simd_bin.as_deref().unwrap_or("missing"))),
        Line::from(format!(
            "SIMAPI.DAT exists={} nonzero={}",
            d.simapi_exists, d.simapi_nonzero
        )),
        Line::from(format!("devices connected={} missing={}", d.connected, d.missing)),
        Line::from(format!("config {}", app.config_path.display())),
        Line::from(format!("docs {}", consts::DOCS_SIMAPI_URL)),
        Line::from(consts::RAW_VIEW_HINT),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Diagnostics")),
        area,
    );
}

fn draw_raw(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(
        Paragraph::new(app.raw_on_disk.clone())
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("On-disk cargopit.config")),
        area,
    );
}

fn draw_logs(frame: &mut Frame, area: Rect, app: &App) {
    let visible = app.logs.visible();
    let start = visible
        .len()
        .saturating_sub(area.height.saturating_sub(2) as usize + app.logs.scroll);
    let lines: Vec<Line> = visible
        .iter()
        .skip(start)
        .map(|line| Line::from(format!("[{}] {}", line.source, line.text)))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Logs filter={}", app.logs.filter.as_str())),
        ),
        area,
    );
    let mut scroll = ScrollbarState::new(visible.len().max(1)).position(start);
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
        area.inner(Margin {
            vertical: consts::LAYOUT_SCROLLBAR_MARGIN,
            horizontal: 0,
        }),
        &mut scroll,
    );
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let text = match app.screen {
        Screen::Dashboard => "Tab/1-4 pages  j/k select  Enter run  q quit",
        Screen::Devices => {
            "j/k list  a add  e edit  y dup  d del  space enable  J/K move  T template  Enter tune  [ ] profile"
        }
        Screen::Settings => "j/k  Enter open  Esc back  q quit",
        Screen::Logs => "j/k scroll  space filter  q quit",
        Screen::DeviceForm if app.form.is_editing() => {
            "type to edit  Enter commit  Esc cancel"
        }
        Screen::DeviceForm => "j/k field  h/l cycle  Enter type  s save  t test  Esc cancel",
        Screen::DeviceTune => "h/l nudge  s save  A apply/restart  Esc back",
        Screen::Confirm(_) => "y confirm  n/Esc cancel",
        _ => "Esc back  q quit",
    };
    frame.render_widget(Paragraph::new(text).style(style_footer()), area);
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
