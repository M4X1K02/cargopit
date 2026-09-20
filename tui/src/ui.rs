use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{simd_field_pairs, tune_fields, App, ConfirmKind, Screen, SettingsSub};
use crate::consts;
use crate::diagnostics;
use crate::schema;
use crate::simd_config;
use crate::templates;

pub fn draw(frame: &mut Frame, app: &App) {
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
    match &app.screen {
        Screen::Dashboard => draw_dashboard(frame, chunks[2], app),
        Screen::Devices => draw_devices(frame, chunks[2], app),
        Screen::Settings => draw_settings(frame, chunks[2], app),
        Screen::Logs => draw_logs(frame, chunks[2], app),
        Screen::DeviceForm => draw_form(frame, chunks[2], app, false),
        Screen::DeviceTune => draw_form(frame, chunks[2], app, true),
        Screen::ProfileEdit => draw_profile_edit(frame, chunks[2], app),
        Screen::TemplatePicker => draw_templates(frame, chunks[2], app),
        Screen::Confirm(kind) => {
            draw_devices(frame, chunks[2], app);
            draw_confirm(frame, size, kind);
        }
        Screen::SettingsSub(sub) => draw_settings_sub(frame, chunks[2], app, *sub),
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

fn draw_devices(frame: &mut Frame, area: Rect, app: &App) {
    let profile = app.current_profile();
    let title = match profile {
        Some(p) => format!(
            "Profile {}/{}  {} / {}  [ ] switch  s edit  a add  D delete",
            app.profile_index + 1,
            app.config.profiles.len(),
            p.sim,
            p.car
        ),
        None => "No profiles".into(),
    };
    let mut items = Vec::new();
    if let Some(profile) = profile {
        if profile.devices.is_empty() {
            items.push(ListItem::new("No devices. Press a to add, T for a template."));
        }
        for device in profile.devices.iter() {
            let enabled = if device.enabled() { "*" } else { " " };
            let presence = app.discovery.presence(
                device.class(),
                device.get_str(consts::KEY_DEVID).unwrap_or(""),
                device.get_str(consts::KEY_DEVPATH).unwrap_or(""),
            );
            let row = format!(
                "{enabled} {}  {}  [{presence}]",
                device.summary(),
                device.identity()
            );
            items.push(ListItem::new(row));
        }
    }
    render_selectable_list(frame, area, title, items, app.device_index);
}

fn draw_form(frame: &mut Frame, area: Rect, app: &App, tune: bool) {
    let fields = if tune {
        tune_fields(&app.form)
    } else {
        app.form.fields()
    };
    let selected = if tune { app.tune_index } else { app.form.field_index };
    let mut items = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        let mut value = schema::display_value(&app.form.device, *field);
        if !tune && app.form.edit_buffer.is_some() && index == app.form.field_index {
            value = format!("{}█", app.form.edit_buffer.as_deref().unwrap_or(""));
        }
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
    let popup = centered(area, 50, 5);
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
    names.extend(diagnostics::lua_scripts());
    let items: Vec<ListItem> = names.iter().map(|name| ListItem::new(name.clone())).collect();
    render_selectable_list(
        frame,
        area,
        "Lua (Enter copies bundled template into ~/.config/cargopit)",
        items,
        app.lua_index,
    );
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

fn draw_tyres(frame: &mut Frame, area: Rect, app: &App) {
    let mut items = Vec::new();
    if app.tyres.cars.is_empty() {
        items.push(ListItem::new("No cars. Press a to add."));
    }
    for car in app.tyres.cars.iter() {
        let row = format!(
            "{} / {}  {} {} {} {}",
            car.sim, car.car, car.tyre0, car.tyre1, car.tyre2, car.tyre3
        );
        items.push(ListItem::new(row));
    }
    render_selectable_list(frame, area, "Tyre diameters", items, app.tyre_index);
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
    let start = visible.len().saturating_sub(area.height.saturating_sub(2) as usize + app.logs.scroll);
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
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let text = match app.screen {
        Screen::Dashboard => "Tab/1-4 pages  j/k select  Enter run  q quit",
        Screen::Devices => {
            "j/k list  a add  e edit  y dup  d del  space enable  J/K move  T template  Enter tune  [ ] profile"
        }
        Screen::Settings => "j/k  Enter open  Esc back  q quit",
        Screen::Logs => "j/k scroll  space filter  q quit",
        Screen::DeviceForm => "j/k field  h/l cycle  Enter type  s save  t test  Esc cancel",
        Screen::DeviceTune => "h/l nudge  s save  A apply/restart  Esc back",
        Screen::Confirm(_) => "y confirm  n/Esc cancel",
        _ => "Esc back  q quit",
    };
    frame.render_widget(
        Paragraph::new(text).style(style_footer()),
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
