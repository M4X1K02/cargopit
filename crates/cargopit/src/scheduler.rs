//! Due-time scheduler. Equal deadlines run in config order.

use cargopit_config::keys;
use cargopit_devices::tick_interval_ms;

const TYRE_CHECK_MS: u64 = 1000;
const MISSED_PERIODS_NONE: u64 = 0;
const PERIOD_DIVISOR_MIN: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKind {
    Device,
    TyreDiameter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Due {
    pub kind: TimerKind,
    pub device_index: usize,
    pub due_ms: u64,
    /// Whole periods skipped before this fire (`0` when on time).
    pub missed_periods: u64,
}

struct Timer {
    due_ms: u64,
    sequence: u64,
    period_ms: u64,
    kind: TimerKind,
    device_index: usize,
}

pub struct Scheduler {
    now_ms: u64,
    next_sequence: u64,
    timers: Vec<Timer>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            now_ms: 0,
            next_sequence: 0,
            timers: Vec::new(),
        }
    }

    pub fn add_device(&mut self, device_index: usize, fps: i32) {
        let period = tick_interval_ms(clamp_fps(fps));
        self.push(TimerKind::Device, device_index, self.now_ms, period);
    }

    pub fn add_tyre_check(&mut self) {
        self.push(TimerKind::TyreDiameter, 0, self.now_ms, TYRE_CHECK_MS);
    }

    pub fn clear(&mut self) {
        self.timers.clear();
    }

    /// One clock sample. Overdue timers fire once, in `(due_ms, insertion)` order.
    pub fn poll(&mut self, now_ms: u64) -> Vec<Due> {
        self.now_ms = now_ms;
        let mut due: Vec<usize> = self
            .timers
            .iter()
            .enumerate()
            .filter(|(_, timer)| timer.due_ms <= now_ms)
            .map(|(index, _)| index)
            .collect();
        due.sort_by(|&left, &right| {
            let a = &self.timers[left];
            let b = &self.timers[right];
            (a.due_ms, a.sequence).cmp(&(b.due_ms, b.sequence))
        });
        let mut fired = Vec::with_capacity(due.len());
        for index in due {
            let timer = &self.timers[index];
            let missed = missed_periods(timer.due_ms, timer.period_ms, now_ms);
            fired.push(Due {
                kind: timer.kind,
                device_index: timer.device_index,
                due_ms: timer.due_ms,
                missed_periods: missed,
            });
            let mut next = timer.due_ms.saturating_add(timer.period_ms);
            if next <= now_ms {
                next = now_ms.saturating_add(timer.period_ms);
            }
            self.timers[index].due_ms = next;
        }
        fired
    }

    fn push(&mut self, kind: TimerKind, device_index: usize, due_ms: u64, period_ms: u64) {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.timers.push(Timer {
            due_ms,
            sequence,
            period_ms,
            kind,
            device_index,
        });
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

pub fn clamp_fps(fps: i32) -> u32 {
    if fps < keys::FPS_MIN {
        return keys::FPS_MIN as u32;
    }
    if fps > keys::FPS_MAX {
        return keys::FPS_MAX as u32;
    }
    fps as u32
}

fn missed_periods(due_ms: u64, period_ms: u64, now_ms: u64) -> u64 {
    if period_ms < PERIOD_DIVISOR_MIN {
        return MISSED_PERIODS_NONE;
    }
    let overdue = now_ms.saturating_sub(due_ms);
    if overdue < period_ms {
        return MISSED_PERIODS_NONE;
    }
    overdue / period_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_tick_immediately_in_config_order() {
        let mut scheduler = Scheduler::new();
        scheduler.add_device(0, 60);
        scheduler.add_device(1, 60);
        let due = scheduler.poll(0);
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].device_index, 0);
        assert_eq!(due[1].device_index, 1);
        assert_eq!(due[0].missed_periods, MISSED_PERIODS_NONE);
        assert!(scheduler.poll(15).is_empty());
        let next = scheduler.poll(16);
        assert_eq!(next.len(), 2);
    }

    #[test]
    fn tyre_check_fires_immediately_then_every_second() {
        let mut scheduler = Scheduler::new();
        scheduler.add_tyre_check();
        assert_eq!(scheduler.poll(0)[0].kind, TimerKind::TyreDiameter);
        assert!(scheduler.poll(999).is_empty());
        assert_eq!(scheduler.poll(1000)[0].kind, TimerKind::TyreDiameter);
        scheduler.clear();
        assert!(scheduler.poll(2000).is_empty());
    }

    #[test]
    fn below_minimum_fps_clamps_to_one() {
        assert_eq!(clamp_fps(0), keys::FPS_MIN as u32);
        assert_eq!(clamp_fps(-4), keys::FPS_MIN as u32);
        assert_eq!(clamp_fps(1001), keys::FPS_MAX as u32);
        assert_eq!(tick_interval_ms(clamp_fps(144)), 6);
    }

    #[test]
    fn late_device_tick_counts_missed_periods() {
        let mut scheduler = Scheduler::new();
        scheduler.add_device(0, 100);
        assert_eq!(scheduler.poll(0)[0].missed_periods, MISSED_PERIODS_NONE);
        let late = scheduler.poll(35);
        assert_eq!(late.len(), 1);
        assert_eq!(late[0].kind, TimerKind::Device);
        assert_eq!(late[0].missed_periods, 2);
    }
}
