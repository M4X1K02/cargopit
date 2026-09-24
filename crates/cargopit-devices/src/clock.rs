//! The only module in the Rust workspace that may read a clock.
//! Device and runtime code will call this trait. Direct `std::time` reads
//! outside this file fail the parity clock check.

pub trait Clock {
    fn monotonic_ms(&self) -> u64;
    fn monotonic_us(&self) -> u64;
    fn monotonic_ns(&self) -> u64;
    fn wall_ms(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VirtualClock {
    monotonic_us: u64,
    wall_ms: u64,
}

impl VirtualClock {
    pub const fn new() -> Self {
        Self {
            monotonic_us: 0,
            wall_ms: 0,
        }
    }

    pub fn advance_us(&mut self, delta_us: u64) {
        self.monotonic_us = self.monotonic_us.saturating_add(delta_us);
        self.wall_ms = self.wall_ms.saturating_add(delta_us / US_PER_MS);
    }
}

impl Clock for VirtualClock {
    fn monotonic_ms(&self) -> u64 {
        self.monotonic_us / US_PER_MS
    }

    fn monotonic_us(&self) -> u64 {
        self.monotonic_us
    }

    fn monotonic_ns(&self) -> u64 {
        self.monotonic_us.saturating_mul(NS_PER_US)
    }

    fn wall_ms(&self) -> u64 {
        self.wall_ms
    }
}

const US_PER_MS: u64 = 1000;
const NS_PER_US: u64 = 1000;
const NS_UNSET: u64 = 0;

/// Monotonic time since this clock was created, and wall time since the Unix epoch.
/// A zero monotonic reading would look like the haptic filter's "never sampled" sentinel,
/// so a brand-new clock reports one nanosecond.
pub struct SystemClock {
    started: std::time::Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn monotonic_ms(&self) -> u64 {
        self.monotonic_us() / US_PER_MS
    }

    fn monotonic_us(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    fn monotonic_ns(&self) -> u64 {
        let ns = u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        if ns == NS_UNSET {
            1
        } else {
            ns
        }
    }

    fn wall_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_clock_advances_in_milliseconds() {
        let mut clock = VirtualClock::new();
        clock.advance_us(16_000);
        assert_eq!(clock.monotonic_us(), 16_000);
        assert_eq!(clock.monotonic_ms(), 16);
        assert_eq!(clock.monotonic_ns(), 16_000_000);
        assert_eq!(clock.wall_ms(), 16);
    }

    #[test]
    fn system_clock_is_nonzero() {
        let clock = SystemClock::new();
        assert!(clock.monotonic_ns() > 0);
        assert!(clock.wall_ms() > 0);
    }
}
