//! ASCII diagrams used by the TUI (pipeline, LEDs, pan, motors, tyres).

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::config::{DeviceEntry, SimProfile};
use crate::consts;
use crate::hardware::Discovery;
use crate::schema::{self, DeviceClass, FieldId};
use crate::simapi_shm::RunningGame;
use crate::theme;
use crate::tyres::TyreCar;

pub fn led_span(name: &str, on: bool, on_label: &'static str, off_label: &'static str) -> Span<'static> {
    let (glyph, color, label) = if on {
        (consts::LED_ON, theme::COLOR_OK, on_label)
    } else {
        (consts::LED_OFF, theme::COLOR_MUTED, off_label)
    };
    Span::styled(
        format!(" {glyph} {name} {label} "),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

pub fn led_span_compact(name: &str, on: bool) -> Span<'static> {
    let (glyph, color) = if on {
        (consts::LED_ON, theme::COLOR_OK)
    } else {
        (consts::LED_OFF, theme::COLOR_MUTED)
    };
    Span::styled(
        format!(" {glyph} {name} "),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

pub fn simapi_span_compact(exists: bool, live: bool) -> Span<'static> {
    let (glyph, color) = if !exists {
        (consts::LED_OFF, theme::COLOR_MUTED)
    } else if live {
        (consts::LED_ON, theme::COLOR_OK)
    } else {
        (consts::LED_ON, theme::COLOR_MUTED)
    };
    Span::styled(
        format!(" {glyph} {} ", consts::LABEL_SIMAPI),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

pub fn simapi_span(exists: bool, live: bool) -> Span<'static> {
    let (glyph, color, label) = if !exists {
        (consts::LED_OFF, theme::COLOR_MUTED, consts::SIMAPI_MISSING)
    } else if live {
        (consts::LED_ON, theme::COLOR_OK, consts::SIMAPI_LIVE)
    } else {
        (consts::LED_ON, theme::COLOR_MUTED, consts::SIMAPI_MAPPED)
    };
    Span::styled(
        format!(" {glyph} {} {label} ", consts::LABEL_SIMAPI),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

pub fn presence_style(presence: &str) -> Style {
    match presence {
        consts::PRESENCE_CONNECTED => theme::style_ok(),
        consts::PRESENCE_MISSING => theme::style_muted(),
        _ => theme::style_warn(),
    }
}

pub fn class_style(class: DeviceClass) -> Style {
    match class {
        DeviceClass::Usb | DeviceClass::Sound | DeviceClass::Serial => Style::default()
            .fg(theme::COLOR_ACCENT)
            .add_modifier(Modifier::BOLD),
    }
}

pub fn pipeline_lines(
    simd: bool,
    games: &[RunningGame],
    simapi_exists: bool,
    simapi_live: bool,
    pit: bool,
    flow_frame: u64,
) -> Vec<Line<'static>> {
    let (primary, extras) = split_games(games);
    let mut lines = vec![pipeline_line(
        simd,
        primary,
        simapi_exists,
        simapi_live,
        pit,
        flow_frame,
    )];
    for game in extras {
        lines.push(game_overflow_line(game, flow_frame));
    }
    lines
}

fn split_games(games: &[RunningGame]) -> (Option<&RunningGame>, &[RunningGame]) {
    if games.is_empty() {
        return (None, games);
    }
    (games.first(), &games[1..])
}

pub fn pipeline_line(
    simd: bool,
    game: Option<&RunningGame>,
    simapi_exists: bool,
    simapi_live: bool,
    pit: bool,
    flow_frame: u64,
) -> Line<'static> {
    let game_flow = game.map(|item| item.sending).unwrap_or(false);
    let shm_flow = simapi_live;
    let into_game = game_flow || shm_flow;
    Line::from(vec![
        led_span(consts::BINARY_SIMD, simd, consts::STATUS_RUNNING, consts::STATUS_STOPPED),
        flow_arrow(into_game, flow_frame),
        game_span(game, flow_frame),
        flow_arrow(shm_flow, flow_frame),
        simapi_span(simapi_exists, simapi_live),
        flow_arrow(shm_flow && pit, flow_frame),
        led_span(
            consts::BINARY_CARGOPIT,
            pit,
            consts::STATUS_RUNNING,
            consts::STATUS_STOPPED,
        ),
    ])
}

fn game_overflow_line(game: &RunningGame, flow_frame: u64) -> Line<'static> {
    Line::from(vec![
        Span::raw(consts::MOTOR_ROW_INDENT),
        game_span(Some(game), flow_frame),
        flow_arrow(game.sending, flow_frame),
        Span::styled(game.status_label, overflow_style(game.sending)),
    ])
}

fn overflow_style(sending: bool) -> ratatui::style::Style {
    if sending {
        theme::style_ok()
    } else {
        theme::style_ok().add_modifier(Modifier::DIM)
    }
}

fn game_span(game: Option<&RunningGame>, flow_frame: u64) -> Span<'static> {
    let Some(game) = game else {
        return Span::styled(
            format!(" {} {} ", consts::LED_OFF, consts::LABEL_NO_SIM),
            theme::style_muted(),
        );
    };
    let (glyph, style) = game_link_look(game, flow_frame);
    Span::styled(
        format!(" {glyph} {} {} ", game.name, game.status_label),
        style.add_modifier(Modifier::BOLD),
    )
}

fn game_link_look(game: &RunningGame, flow_frame: u64) -> (&'static str, ratatui::style::Style) {
    if game.sending {
        return (pulse_glyph(flow_frame), theme::style_ok());
    }
    if game.status_label == consts::LABEL_TELEMETRY_RUNNING {
        return (consts::LED_ON, theme::style_ok());
    }
    (consts::LED_OFF, theme::style_warn())
}

fn flow_arrow(sending: bool, flow_frame: u64) -> Span<'static> {
    if !sending {
        return Span::styled(consts::PIPELINE_ARROW, theme::style_muted());
    }
    let frames = consts::TELEMETRY_FLOW_FRAMES;
    let index = (flow_frame as usize) % frames.len();
    Span::styled(frames[index], theme::style_ok())
}

fn pulse_glyph(flow_frame: u64) -> &'static str {
    if flow_frame % 2 == 0 {
        consts::TELEMETRY_PULSE_ON
    } else {
        consts::TELEMETRY_PULSE_OFF
    }
}

pub fn class_presence_line(profile: &SimProfile, class: DeviceClass, discovery: &Discovery) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            format!("{:<width$}", class.as_str(), width = consts::CLASS_COLUMN_WIDTH),
            class_style(class),
        ),
        Span::raw(" "),
    ];
    let mut any = false;
    for device in profile.devices.iter() {
        if device.class() != class {
            continue;
        }
        any = true;
        let presence = discovery.presence(
            class,
            device.get_str(consts::KEY_DEVID).unwrap_or(""),
            device.get_str(consts::KEY_DEVPATH).unwrap_or(""),
        );
        let glyph = if presence == consts::PRESENCE_CONNECTED {
            consts::LED_ON
        } else {
            consts::LED_OFF
        };
        let mut style = presence_style(presence);
        if !device.enabled() {
            style = style.patch(theme::style_dim());
        }
        spans.push(Span::styled(format!("{glyph} "), style));
    }
    if !any {
        spans.push(Span::styled(consts::LED_OFF.to_string(), theme::style_muted()));
    }
    Line::from(spans)
}

pub fn led_strip(count: i64, start: i64, end: i64) -> Line<'static> {
    if count <= 0 {
        return Line::from(Span::styled(consts::LED_OFF, theme::style_muted()));
    }
    let mut spans = Vec::new();
    for index in 1..=count {
        let on = index >= start && index <= end;
        let glyph = if on { consts::LED_ON } else { consts::LED_OFF };
        let style = if on {
            theme::style_ok()
        } else {
            theme::style_muted()
        };
        spans.push(Span::styled(format!("{glyph} "), style));
    }
    Line::from(spans)
}

pub fn pan_line(pan: i64, channels: i64) -> Line<'static> {
    let slots = channels.max(2);
    let marker = pan.clamp(0, slots.saturating_sub(1));
    let mut spans = vec![Span::styled(
        format!("{} ", consts::LABEL_PAN_LEFT),
        theme::style_muted(),
    )];
    for index in 0..slots {
        let on = index == marker;
        let glyph = if on {
            consts::LED_ON.to_string()
        } else {
            consts::GAUGE_EMPTY.to_string()
        };
        let style = if on {
            theme::style_ok()
        } else {
            theme::style_muted()
        };
        spans.push(Span::styled(format!("{glyph} "), style));
    }
    spans.push(Span::styled(consts::LABEL_PAN_RIGHT, theme::style_muted()));
    Line::from(spans)
}

pub fn motor_active(index: i64) -> [bool; consts::MOTOR_SLOT_COUNT] {
    let label = schema::motor_label(index);
    [
        label.contains(consts::LABEL_MOTOR_FL),
        label.contains(consts::LABEL_MOTOR_FR),
        label.contains(consts::LABEL_MOTOR_RR),
        label.contains(consts::LABEL_MOTOR_RL),
    ]
}

fn motor_cell(label: &str, on: bool) -> Span<'static> {
    let glyph = if on { consts::LED_ON } else { consts::LED_OFF };
    let style = if on {
        theme::style_ok()
    } else {
        theme::style_muted()
    };
    Span::styled(format!("{label}{glyph}"), style)
}

pub fn motor_lines(index: i64) -> Vec<Line<'static>> {
    let on = motor_active(index);
    vec![
        Line::from(Span::styled(consts::DIAGRAM_TITLE_CHASSIS, theme::style_title())),
        Line::from(vec![
            Span::raw(consts::MOTOR_ROW_INDENT),
            motor_cell(consts::LABEL_MOTOR_FL, on[0]),
            Span::raw(consts::MOTOR_CELL_GAP),
            motor_cell(consts::LABEL_MOTOR_FR, on[1]),
        ]),
        Line::from(Span::styled(
            format!("{}{}", consts::MOTOR_AXIS_INDENT, consts::LABEL_FRONT),
            theme::style_muted(),
        )),
        Line::from(vec![
            Span::raw(consts::MOTOR_ROW_INDENT),
            motor_cell(consts::LABEL_MOTOR_RL, on[3]),
            Span::raw(consts::MOTOR_CELL_GAP),
            motor_cell(consts::LABEL_MOTOR_RR, on[2]),
        ]),
        Line::from(Span::styled(
            format!("{}{}", consts::MOTOR_AXIS_INDENT, consts::LABEL_REAR),
            theme::style_muted(),
        )),
    ]
}

pub fn tyre_lines(car: &TyreCar) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            format!("{} / {}", car.sim, car.car),
            theme::style_title(),
        )),
        Line::from(Span::styled(consts::LABEL_FRONT, theme::style_muted())),
        Line::from(format!(
            "  {} {:>width$.prec$}    {} {:>width$.prec$}",
            consts::TYRE_LABEL_FL,
            car.tyre0,
            consts::TYRE_LABEL_FR,
            car.tyre1,
            width = consts::TYRE_VALUE_WIDTH,
            prec = consts::TYRE_VALUE_PREC
        )),
        Line::from(Span::styled(consts::LABEL_REAR, theme::style_muted())),
        Line::from(format!(
            "  {} {:>width$.prec$}    {} {:>width$.prec$}",
            consts::TYRE_LABEL_RL,
            car.tyre2,
            consts::TYRE_LABEL_RR,
            car.tyre3,
            width = consts::TYRE_VALUE_WIDTH,
            prec = consts::TYRE_VALUE_PREC
        )),
    ]
}

pub fn field_ratio(device: &DeviceEntry, field: FieldId) -> Option<f64> {
    match field {
        FieldId::Volume => device
            .get_i64(consts::KEY_STREAM_VOLUME)
            .or_else(|| device.get_i64(consts::KEY_VOLUME))
            .map(|v| int_ratio(v, consts::VOLUME_MAX)),
        FieldId::Amplitude => device
            .get_i64(consts::KEY_AMPLITUDE)
            .map(|v| int_ratio(v, consts::DEFAULT_AMPLITUDE_MAX)),
        FieldId::AmplitudeMax => device
            .get_i64(consts::KEY_AMPLITUDE_MAX)
            .map(|v| int_ratio(v, consts::DEFAULT_AMPLITUDE_MAX)),
        FieldId::Frequency => device
            .get_i64(consts::KEY_FREQUENCY)
            .map(|v| int_ratio(v, consts::GAUGE_FREQ_MAX as i64)),
        FieldId::FrequencyMax => device
            .get_i64(consts::KEY_FREQUENCY_MAX)
            .map(|v| int_ratio(v, consts::GAUGE_FREQ_MAX as i64)),
        FieldId::Fps => device
            .get_i64(consts::KEY_FPS)
            .map(|v| int_ratio(v, consts::GAUGE_FPS_MAX as i64)),
        FieldId::Noise => device
            .get_i64(consts::KEY_NOISE)
            .map(|v| int_ratio(v, consts::GAUGE_NOISE_MAX as i64)),
        FieldId::Channels => device
            .get_i64(consts::KEY_CHANNELS)
            .map(|v| int_ratio(v, consts::GAUGE_CHANNELS_MAX as i64)),
        FieldId::Baud => device
            .get_i64(consts::KEY_BAUD)
            .map(|v| int_ratio(v, consts::GAUGE_BAUD_MAX as i64)),
        FieldId::Fanpower => device.get_f64(consts::KEY_FANPOWER).map(|v| v.clamp(0.0, 1.0)),
        FieldId::Threshold => device.get_f64(consts::KEY_THRESHOLD).map(|v| v.clamp(0.0, 1.0)),
        FieldId::Duration => device
            .get_f64(consts::KEY_DURATION)
            .map(|v| v / consts::GAUGE_DURATION_MAX),
        FieldId::Ampfactor => device
            .get_f64(consts::KEY_AMPFACTOR)
            .map(|v| v / consts::GAUGE_AMPFACTOR_MAX),
        FieldId::Pan => {
            let pan = device.get_i64(consts::KEY_PAN)?;
            let channels = device
                .get_i64(consts::KEY_CHANNELS)
                .unwrap_or(consts::DEFAULT_CHANNELS)
                .max(1);
            Some(int_ratio(pan, channels.saturating_sub(1).max(1)))
        }
        _ => None,
    }
}

fn int_ratio(value: i64, max: i64) -> f64 {
    if max <= 0 {
        return 0.0;
    }
    value as f64 / max as f64
}

pub fn device_diagram_lines(device: &DeviceEntry) -> Vec<Line<'static>> {
    let class = device.class();
    let type_name = schema::normalize_type_name(class, device.type_name());
    if type_name == consts::TYPE_TACHOMETER
        || type_name == consts::TYPE_SHIFT_LIGHTS
        || type_name == consts::TYPE_SIMLEDS
    {
        return vec![
            Line::from(Span::styled(consts::DIAGRAM_TITLE_LEDS, theme::style_title())),
            led_device_line(device, type_name),
        ];
    }
    if type_name == consts::TYPE_SIM_WIND {
        let power = device
            .get_f64(consts::KEY_FANPOWER)
            .unwrap_or(consts::DEFAULT_FANPOWER);
        return vec![
            Line::from(Span::styled(consts::KEY_FANPOWER, theme::style_title())),
            Line::from(format!("{}  {:.2}", theme::bar(power, consts::GAUGE_WIDTH), power)),
        ];
    }
    if type_name == consts::TYPE_HAPTIC && class == DeviceClass::Serial {
        let mut lines = motor_lines(device.get_i64(consts::KEY_MOTORS).unwrap_or(0));
        lines.extend(sound_gauge_lines(device));
        return lines;
    }
    if class == DeviceClass::Sound || type_name == consts::TYPE_HAPTIC {
        return sound_gauge_lines(device);
    }
    vec![Line::from(Span::styled(device.summary(), theme::style_muted()))]
}

fn led_device_line(device: &DeviceEntry, type_name: &str) -> Line<'static> {
    if type_name == consts::TYPE_SHIFT_LIGHTS {
        let count = device
            .get_i64(consts::KEY_NUMLIGHTS)
            .unwrap_or(consts::DEFAULT_NUMLIGHTS);
        return led_strip(count, 1, count);
    }
    let count = device
        .get_i64(consts::KEY_NUMLEDS)
        .unwrap_or(consts::DEFAULT_NUMLEDS);
    if type_name == consts::TYPE_TACHOMETER {
        return led_strip(count.max(consts::DEFAULT_NUMLEDS), 1, count.max(1));
    }
    let start = device
        .get_i64(consts::KEY_STARTLED)
        .unwrap_or(consts::DEFAULT_STARTLED);
    let end = device
        .get_i64(consts::KEY_ENDLED)
        .unwrap_or(consts::DEFAULT_ENDLED);
    led_strip(count, start, end)
}

fn sound_gauge_lines(device: &DeviceEntry) -> Vec<Line<'static>> {
    let volume = field_ratio(device, FieldId::Volume).unwrap_or(0.0);
    let channels = device
        .get_i64(consts::KEY_CHANNELS)
        .unwrap_or(consts::DEFAULT_CHANNELS);
    let pan = device.get_i64(consts::KEY_PAN).unwrap_or(consts::DEFAULT_PAN);
    vec![
        Line::from(Span::styled(FieldId::Volume.label(), theme::style_title())),
        Line::from(theme::bar(volume, consts::GAUGE_WIDTH)),
        Line::from(Span::styled(FieldId::Pan.label(), theme::style_title())),
        pan_line(pan, channels),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn led_strip_marks_range() {
        let line = led_strip(6, 2, 4);
        assert_eq!(line.spans.len(), 6);
    }

    #[test]
    fn motor_all_corners() {
        let all = (0..consts::MOTOR_COUNT).find(|&index| {
            motor_active(index) == [true, true, true, true]
        });
        assert!(all.is_some());
    }

    #[test]
    fn live_pipeline_animates_and_idle_stays_still() {
        let live = RunningGame {
            name: "Assetto Corsa".into(),
            game_id: 244210,
            sending: true,
            status_label: consts::LABEL_TELEMETRY_LIVE,
        };
        let idle = RunningGame {
            name: "Assetto Corsa".into(),
            game_id: 244210,
            sending: false,
            status_label: consts::LABEL_TELEMETRY_IDLE,
        };
        let live0 = pipeline_lines(true, &[live.clone()], true, true, true, 0);
        let live1 = pipeline_lines(true, &[live], true, true, true, 1);
        assert_ne!(format!("{:?}", live0), format!("{:?}", live1));
        let idle0 = pipeline_lines(true, &[idle.clone()], true, false, true, 0);
        let idle1 = pipeline_lines(true, &[idle], true, false, true, 1);
        assert_eq!(format!("{:?}", idle0), format!("{:?}", idle1));
        let empty = pipeline_lines(false, &[], false, false, false, 0);
        let dump = format!("{:?}", empty);
        assert!(dump.contains(consts::LABEL_NO_SIM));
    }

    #[test]
    fn running_game_is_lit_without_flow_animation() {
        let running = RunningGame {
            name: "DirtRally2".into(),
            game_id: 690790,
            sending: false,
            status_label: consts::LABEL_TELEMETRY_RUNNING,
        };
        let first = pipeline_lines(true, &[running.clone()], true, false, true, 0);
        let second = pipeline_lines(true, &[running], true, false, true, 1);
        assert_eq!(format!("{:?}", first), format!("{:?}", second));
        let dump = format!("{:?}", first);
        assert!(dump.contains(consts::LABEL_TELEMETRY_RUNNING));
        assert!(dump.contains(consts::SIMAPI_MAPPED));
        assert!(dump.contains(consts::LED_ON));
    }

    #[test]
    fn simapi_to_cargopit_animates_when_live() {
        let running = RunningGame {
            name: "DirtRally2".into(),
            game_id: 690790,
            sending: false,
            status_label: consts::LABEL_TELEMETRY_RUNNING,
        };
        let first = pipeline_lines(true, &[running.clone()], true, true, true, 0);
        let second = pipeline_lines(true, &[running], true, true, true, 1);
        assert_ne!(format!("{:?}", first), format!("{:?}", second));
        let dump = format!("{:?}", first);
        assert!(dump.contains(consts::TELEMETRY_FLOW_FRAMES[0]) || dump.contains("►"));
    }

    #[test]
    fn extra_games_wrap_to_overflow_line() {
        let games = vec![
            RunningGame {
                name: "Assetto Corsa".into(),
                game_id: 1,
                sending: true,
                status_label: consts::LABEL_TELEMETRY_LIVE,
            },
            RunningGame {
                name: "rFactor 2".into(),
                game_id: 2,
                sending: false,
                status_label: consts::LABEL_TELEMETRY_IDLE,
            },
        ];
        let lines = pipeline_lines(true, &games, true, true, true, 0);
        assert!(lines.len() >= 2);
        let dump = format!("{:?}", lines);
        assert!(dump.contains("rFactor 2"));
    }
}
