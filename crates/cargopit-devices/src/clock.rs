//! The only module in the Rust workspace that may read a clock.
//! Device and runtime code will call this trait. Direct `std::time` reads
//! outside this file fail the parity clock check.

pub trait Clock {
    fn monotonic_ms(&self) -> u64;
    fn monotonic_us(&self) -> u64;
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

    fn wall_ms(&self) -> u64 {
        self.wall_ms
    }
}

const US_PER_MS: u64 = 1000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_clock_advances_in_milliseconds() {
        let mut clock = VirtualClock::new();
        clock.advance_us(16_000);
        assert_eq!(clock.monotonic_us(), 16_000);
        assert_eq!(clock.monotonic_ms(), 16);
        assert_eq!(clock.wall_ms(), 16);
    }
}
