//! Tachometer wizard tables. Interactive pulse capture feeds `write_xml`.

pub const MIN_REVS: u32 = 2000;
pub const BASE_INCREMENT: u32 = 1000;
pub const GRANULARITY_FINE: u32 = 2;
pub const GRANULARITY_FINEST: u32 = 4;
pub const FINE_INCREMENT: u32 = 500;
pub const FINEST_INCREMENT: u32 = 250;
pub const FIRST_PULSE_COARSE: u32 = 250;
const MIN_REVS_MESSAGE: &str = "revs must be at least 2000";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TachNode {
    pub rpm: u32,
    pub pulses: u32,
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
                pulses: *rpm,
            })
            .collect();
        let xml = write_xml(2000, &nodes);
        let parsed = cargopit_config::tach::parse_tach_xml(&xml).expect("xml");
        assert_eq!(parsed.len(), nodes.len());
        assert_eq!(parsed[0].pulses, 250);
    }
}
