//! Hardware-test announcements. The embedded loop ticks every 16 ms.

pub const TICK_US: u64 = 16_000;
pub const STEP_PREFIX: &str = "test step: ";
pub const MSG_PREPARING: &str = "preparing test with";
pub const MSG_STARTING: &str = "Starting";
pub const MSG_FINISHED: &str = "Finished";
pub const MSG_STOPPED: &str = "Stopped";
pub const MSG_REV: &str = "Revving rpm from idle to redline and back";
pub const MSG_RPM_IDLE: &str = "Setting rpms to idle";
pub const MSG_COAST: &str = "Returning to idle";
const US_PER_MS: u64 = 1000;

pub fn preparing(device_count: usize) -> String {
    format!("{STEP_PREFIX}{MSG_PREPARING} {device_count} devices...")
}

pub fn named(action: &str, label: &str) -> String {
    format!("{STEP_PREFIX}{action} {label}")
}

pub fn step(message: &str) -> String {
    format!("{STEP_PREFIX}{message}")
}

pub fn tick_count(hold_us: u64) -> u64 {
    hold_us / TICK_US
}

pub fn embedded_tick_ms() -> u64 {
    TICK_US / US_PER_MS
}

pub fn light_script(label: &str) -> Vec<String> {
    let mut lines = vec![
        named(MSG_STARTING, label),
        step(MSG_REV),
        step(MSG_RPM_IDLE),
    ];
    lines.push(step(MSG_COAST));
    lines.push(named(MSG_FINISHED, label));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announcements_use_the_c_prefix_and_sixteen_millisecond_tick() {
        assert_eq!(embedded_tick_ms(), 16);
        assert_eq!(tick_count(TICK_US), 1);
        let lines = light_script("USB lights");
        assert!(lines[0].starts_with(STEP_PREFIX));
        assert!(lines.iter().any(|line| line.contains(MSG_REV)));
        assert_eq!(preparing(0), "test step: preparing test with 0 devices...");
    }
}
