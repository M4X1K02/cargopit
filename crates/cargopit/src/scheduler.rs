//! Due-time scheduler. Equal deadlines run in config order.

use cargopit_devices::tick_interval_ms;

const DISCOVERY_MS: u64 = 1000;
const TYRE_CHECK_MS: u64 = 1000;
const MAPPING_START_MS: u64 = 2000;
const FPS_MIN: u32 = 1;
const FPS_MAX: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKind {
    Device,
    Discovery,
    TyreDiameter,
    Mapping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Due {
    pub kind: TimerKind,
    pub device_index: usize,
    pub due_ms: u64,
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

    pub fn add_discovery(&mut self) {
        self.push(TimerKind::Discovery, 0, self.now_ms, DISCOVERY_MS);
    }

    pub fn add_tyre_check(&mut self) {
        self.push(TimerKind::TyreDiameter, 0, self.now_ms, TYRE_CHECK_MS);
    }

    pub fn add_mapping(&mut self, fps: i32) {
        let period = tick_interval_ms(clamp_fps(fps));
        self.push(
            TimerKind::Mapping,
            0,
            self.now_ms.saturating_add(MAPPING_START_MS),
            period,
        );
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
            fired.push(Due {
                kind: timer.kind,
                device_index: timer.device_index,
                due_ms: timer.due_ms,
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
    if fps < FPS_MIN as i32 {
        return FPS_MIN;
    }
    if fps > FPS_MAX as i32 {
        return FPS_MAX;
    }
    fps as u32
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
        assert!(scheduler.poll(15).is_empty());
        let next = scheduler.poll(16);
        assert_eq!(next.len(), 2);
    }

    #[test]
    fn mapping_waits_and_discovery_repeats() {
        let mut scheduler = Scheduler::new();
        scheduler.add_discovery();
        scheduler.add_mapping(60);
        let start = scheduler.poll(0);
        assert_eq!(start.len(), 1);
        assert_eq!(start[0].kind, TimerKind::Discovery);
        assert!(scheduler
            .poll(1999)
            .iter()
            .all(|due| due.kind != TimerKind::Mapping));
        let mapped = scheduler.poll(2000);
        assert!(mapped.iter().any(|due| due.kind == TimerKind::Mapping));
    }

    #[test]
    fn discovery_repeats_on_its_own_interval() {
        let mut scheduler = Scheduler::new();
        scheduler.add_discovery();
        assert_eq!(scheduler.poll(0)[0].kind, TimerKind::Discovery);
        assert!(scheduler.poll(999).is_empty());
        assert_eq!(scheduler.poll(1000)[0].kind, TimerKind::Discovery);
    }

    #[test]
    fn below_minimum_fps_clamps_to_one() {
        assert_eq!(clamp_fps(0), FPS_MIN);
        assert_eq!(clamp_fps(-4), FPS_MIN);
        assert_eq!(clamp_fps(1001), FPS_MAX);
        assert_eq!(tick_interval_ms(clamp_fps(144)), 6);
    }
}
