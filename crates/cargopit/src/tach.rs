//! Tachometer wizard tables. Interactive pulse capture feeds `write_xml`.

pub const MIN_REVS: u32 = 2000;
pub const BASE_INCREMENT: u32 = 1000;
pub const GRANULARITY_FINE: u32 = 2;
pub const GRANULARITY_FINEST: u32 = 4;
pub const FINE_INCREMENT: u32 = 500;
pub const FINEST_INCREMENT: u32 = 250;
pub const FIRST_PULSE_COARSE: u32 = 250;
pub const SETTLE_SECS: u64 = 2;
pub const MSG_CONTINUE: &str = "Press Return to continue...";
pub const MSG_WRITE_FAILED: &str = "could not write tachometer file";
const MIN_REVS_MESSAGE: &str = "revs must be at least 2000";
const KEY_ACCEPT: u8 = b'\n';
const KEY_INCREASE: u8 = b'>';
const KEY_DECREASE: u8 = b'<';
const KEY_INCREASE_SMALL: u8 = b'c';
const KEY_DECREASE_SMALL: u8 = b'z';
const KEY_INCREASE_LARGE: u8 = b'm';
const KEY_DECREASE_LARGE: u8 = b'n';
const PULSE_UNIT: i32 = 1;
const PULSE_SMALL: i32 = 100;
const PULSE_LARGE: i32 = 1000;
const PULSE_START: i32 = 0;
const FIRST_NODE: usize = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TachNode {
    pub rpm: u32,
    pub pulses: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WizardEvent {
    Line(String),
    Settle,
    Show(i32),
}

pub fn rpm_targets(max_revs: u32, granularity: u32) -> Option<Vec<u32>> {
    if max_revs < MIN_REVS {
        return None;
    }
    let increment = increment_for(granularity);
    let mut nodes = ((max_revs / BASE_INCREMENT) * granularity) + 1;
    if granularity >= GRANULARITY_FINEST {
        nodes = nodes.saturating_sub(1);
    }
    let mut values = vec![0u32; nodes as usize];
    values[0] = FIRST_PULSE_COARSE;
    if nodes > 1 {
        values[1] = increment;
    }
    if granularity >= GRANULARITY_FINEST {
        values[0] = increment;
        if nodes > 1 {
            values[1] = increment * 2;
        }
    }
    let mut index = 2usize;
    while index < values.len() {
        values[index] = values[index - 1] + increment;
        index += 1;
    }
    Some(values)
}

pub fn min_revs_message() -> &'static str {
    MIN_REVS_MESSAGE
}

pub fn set_revs_message(target: u32) -> String {
    format!(
        "Set tachometer revs to {target}: Press > to increase, < to decrease, and Return to accept (m increases by 1000, n decreases by 1000, c increases by 100, z decreases by 100..."
    )
}

pub fn set_pulses_message(pulses: i32) -> String {
    format!("set pulses to {pulses}")
}

/// Reads the C key script. Pulses carry from one rev target to the next.
/// `Err` means input ended before every target was accepted, so no file should be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureStopped;

pub fn capture<I, F>(
    targets: &[u32],
    keys: &mut I,
    mut on_event: F,
) -> Result<Vec<i32>, CaptureStopped>
where
    I: Iterator<Item = u8>,
    F: FnMut(WizardEvent),
{
    let mut pulses = PULSE_START;
    let mut saved = Vec::with_capacity(targets.len());
    for (index, target) in targets.iter().enumerate() {
        if index == FIRST_NODE && !await_continue(keys, &mut on_event) {
            return Err(CaptureStopped);
        }
        on_event(WizardEvent::Settle);
        on_event(WizardEvent::Line(set_revs_message(*target)));
        if !adjust_until_accept(keys, &mut pulses, &mut on_event) {
            return Err(CaptureStopped);
        }
        saved.push(pulses);
        on_event(WizardEvent::Line(set_pulses_message(pulses)));
    }
    Ok(saved)
}

fn await_continue<I, F>(keys: &mut I, on_event: &mut F) -> bool
where
    I: Iterator<Item = u8>,
    F: FnMut(WizardEvent),
{
    on_event(WizardEvent::Line(MSG_CONTINUE.to_string()));
    keys.next().is_some()
}

fn adjust_until_accept<I, F>(keys: &mut I, pulses: &mut i32, on_event: &mut F) -> bool
where
    I: Iterator<Item = u8>,
    F: FnMut(WizardEvent),
{
    loop {
        on_event(WizardEvent::Show(*pulses));
        let Some(key) = keys.next() else {
            return false;
        };
        if key == KEY_ACCEPT {
            return true;
        }
        *pulses = apply_key(*pulses, key);
    }
}

fn apply_key(pulses: i32, key: u8) -> i32 {
    if key == KEY_INCREASE {
        return pulses.saturating_add(PULSE_UNIT);
    }
    if key == KEY_DECREASE {
        return pulses.saturating_sub(PULSE_UNIT);
    }
    if key == KEY_INCREASE_SMALL {
        return pulses.saturating_add(PULSE_SMALL);
    }
    if key == KEY_DECREASE_SMALL {
        return pulses.saturating_sub(PULSE_SMALL);
    }
    if key == KEY_INCREASE_LARGE {
        return pulses.saturating_add(PULSE_LARGE);
    }
    if key == KEY_DECREASE_LARGE {
        return pulses.saturating_sub(PULSE_LARGE);
    }
    pulses
}

pub fn write_xml(max_revs: u32, nodes: &[TachNode]) -> String {
    let mut body = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<TachometerSettings>\n  <SettingsValues>\n",
    );
    for node in nodes {
        body.push_str(&format!(
            "    <SettingsItem>\n      <Value>{}</Value>\n      <TimeValue>{}</TimeValue>\n    </SettingsItem>\n",
            node.rpm, node.pulses
        ));
    }
    body.push_str(&format!(
        "  </SettingsValues>\n  <MaxDisplayValue>{max_revs}</MaxDisplayValue>\n</TachometerSettings>\n"
    ));
    body
}

fn increment_for(granularity: u32) -> u32 {
    if granularity == GRANULARITY_FINE {
        return FINE_INCREMENT;
    }
    if granularity >= GRANULARITY_FINEST {
        return FINEST_INCREMENT;
    }
    BASE_INCREMENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_table_and_xml_round_trip() {
        assert!(rpm_targets(1000, 1).is_none());
        let targets = rpm_targets(2000, 1).expect("nodes");
        assert_eq!(targets, vec![250, 1000, 2000]);
        let nodes: Vec<TachNode> = targets
            .iter()
            .map(|rpm| TachNode {
                rpm: *rpm,
                pulses: i32::try_from(*rpm).unwrap_or(i32::MAX),
            })
            .collect();
        let xml = write_xml(2000, &nodes);
        let parsed = cargopit_config::tach::parse_tach_xml(&xml).expect("xml");
        assert_eq!(parsed.len(), nodes.len());
        assert_eq!(parsed[0].pulses, 250);
    }

    #[test]
    fn keys_carry_between_targets_and_the_continue_key_is_not_applied() {
        let targets = rpm_targets(2000, 1).expect("nodes");
        let mut keys = [
            KEY_INCREASE,
            KEY_INCREASE_LARGE,
            KEY_ACCEPT,
            KEY_DECREASE,
            KEY_ACCEPT,
            b'q',
            KEY_ACCEPT,
        ]
        .into_iter();
        let mut events = Vec::new();
        let pulses = capture(&targets, &mut keys, |event| events.push(event)).expect("captured");
        assert_eq!(
            pulses,
            vec![
                PULSE_LARGE,
                PULSE_LARGE - PULSE_UNIT,
                PULSE_LARGE - PULSE_UNIT
            ]
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == WizardEvent::Settle)
                .count(),
            targets.len()
        );
        assert!(events
            .iter()
            .any(|event| matches!(event, WizardEvent::Line(line) if line == MSG_CONTINUE)));
        assert!(events.iter().any(|event| matches!(event, WizardEvent::Line(line) if line == &set_revs_message(FIRST_PULSE_COARSE))));
        assert!(events
            .iter()
            .any(|event| matches!(event, WizardEvent::Show(shown) if *shown == PULSE_START)));
        let nodes: Vec<TachNode> = targets
            .iter()
            .zip(pulses)
            .map(|(rpm, pulses)| TachNode { rpm: *rpm, pulses })
            .collect();
        let xml = write_xml(2000, &nodes);
        assert!(xml.contains("<Value>250</Value>"));
        assert!(xml.contains("<TimeValue>999</TimeValue>"));
    }

    #[test]
    fn input_that_ends_early_does_not_return_pulses() {
        let targets = rpm_targets(2000, 1).expect("nodes");
        let mut keys = [KEY_ACCEPT].into_iter();
        let result = capture(&targets, &mut keys, |_| {});
        assert!(result.is_err());
        let mut negative = [KEY_ACCEPT, KEY_DECREASE, KEY_ACCEPT].into_iter();
        let one = [targets[0]];
        let pulses = capture(&one, &mut negative, |_| {}).expect("one node");
        assert_eq!(pulses, vec![-PULSE_UNIT]);
        let xml = write_xml(
            MIN_REVS,
            &[TachNode {
                rpm: targets[0],
                pulses: pulses[0],
            }],
        );
        assert!(xml.contains("<TimeValue>-1</TimeValue>"));
    }
}
