//! The only module in the Rust workspace that may read a clock.
//! Device and runtime code will call this trait. Direct `std::time` reads
//! outside this file fail the parity clock check.

pub trait Clock {
    fn monotonic_ms(&self) -> u64;
    fn monotonic_us(&self) -> u64;
    fn monotonic_ns(&self) -> u64;
    fn wall_ms(&self) -> u64;

    fn wall_stamp(&self) -> WallStamp {
        WallStamp::from_unix_ms(self.wall_ms())
    }
}

/// Local civil time. `millis` is the C `slog` field: microseconds divided by 1000.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WallStamp {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millis: u16,
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

    pub fn from_monotonic_ns(now_ns: u64) -> Self {
        let mut clock = Self::new();
        clock.advance_us(now_ns / NS_PER_US);
        clock
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
const MS_PER_SEC: u64 = 1000;
const SEC_PER_MIN: u64 = 60;
const MIN_PER_HOUR: u64 = 60;
const HOUR_PER_DAY: u64 = 24;
const MS_PER_DAY: u64 = MS_PER_SEC * SEC_PER_MIN * MIN_PER_HOUR * HOUR_PER_DAY;
const UNIX_TO_CIVIL_DAYS: i64 = 719_468;
const DAYS_PER_ERA: i64 = 146_097;
const DAYS_PER_YEAR: u64 = 365;
const LEAP_DOE_DIVISOR: u64 = 1_460;
const CENTURY_DOE_DIVISOR: u64 = 36_524;
const ERA_DOE_DIVISOR: u64 = 146_096;
const MONTH_DAY_SCALE: u64 = 153;
const MONTH_FROM_MARCH: u64 = 10;
const MARCH_MONTH: u8 = 3;
const YEAR_ORIGIN: i32 = 1900;
const USEC_PER_MS: i64 = 1000;

impl WallStamp {
    pub fn from_unix_ms(ms: u64) -> Self {
        let days = i64::try_from(ms / MS_PER_DAY).unwrap_or(i64::MAX);
        let tod = ms % MS_PER_DAY;
        let hour = u8::try_from(tod / (MS_PER_SEC * SEC_PER_MIN * MIN_PER_HOUR)).unwrap_or(0);
        let minute = u8::try_from((tod / (MS_PER_SEC * SEC_PER_MIN)) % MIN_PER_HOUR).unwrap_or(0);
        let second = u8::try_from((tod / MS_PER_SEC) % SEC_PER_MIN).unwrap_or(0);
        let millis = u16::try_from(tod % MS_PER_SEC).unwrap_or(0);
        let (year, month, day) = civil_ymd(days);
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millis,
        }
    }
}

fn civil_ymd(unix_days: i64) -> (i32, u8, u8) {
    let z = unix_days + UNIX_TO_CIVIL_DAYS;
    let era = if z >= 0 {
        z / DAYS_PER_ERA
    } else {
        (z - (DAYS_PER_ERA - 1)) / DAYS_PER_ERA
    };
    let doe = u64::try_from(z - era * DAYS_PER_ERA).unwrap_or(0);
    let yoe = (doe - doe / LEAP_DOE_DIVISOR + doe / CENTURY_DOE_DIVISOR - doe / ERA_DOE_DIVISOR)
        / DAYS_PER_YEAR;
    let mut year = i32::try_from(i64::from(yoe as u32) + era * 400).unwrap_or(0);
    let doy = doe
        - (DAYS_PER_YEAR * u64::from(yoe as u32) + u64::from(yoe as u32) / 4
            - u64::from(yoe as u32) / 100);
    let month_index = (5 * doy + 2) / MONTH_DAY_SCALE;
    let day = u8::try_from(doy - (MONTH_DAY_SCALE * month_index + 2) / 5 + 1).unwrap_or(1);
    let month = if month_index < MONTH_FROM_MARCH {
        u8::try_from(month_index).unwrap_or(0) + MARCH_MONTH
    } else {
        u8::try_from(month_index - 9).unwrap_or(1)
    };
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

fn local_wall_stamp() -> WallStamp {
    let mut tv = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    if unsafe { libc::gettimeofday(&mut tv, std::ptr::null_mut()) } != 0 {
        return WallStamp::from_unix_ms(0);
    }
    let mut broken = unsafe { std::mem::zeroed::<libc::tm>() };
    if unsafe { libc::localtime_r(&tv.tv_sec, &mut broken) }.is_null() {
        return WallStamp::from_unix_ms(0);
    }
    WallStamp {
        year: broken.tm_year + YEAR_ORIGIN,
        month: u8::try_from(broken.tm_mon + 1).unwrap_or(1),
        day: u8::try_from(broken.tm_mday).unwrap_or(1),
        hour: u8::try_from(broken.tm_hour).unwrap_or(0),
        minute: u8::try_from(broken.tm_min).unwrap_or(0),
        second: u8::try_from(broken.tm_sec).unwrap_or(0),
        millis: u16::try_from(tv.tv_usec / USEC_PER_MS).unwrap_or(0),
    }
}

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

    fn wall_stamp(&self) -> WallStamp {
        local_wall_stamp()
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
        let stamp = clock.wall_stamp();
        assert!(stamp.year >= YEAR_ORIGIN);
        assert!(stamp.month >= 1 && stamp.month <= 12);
        assert!(u64::from(stamp.millis) < MS_PER_SEC);
    }

    #[test]
    fn unix_epoch_is_the_civil_start() {
        let stamp = WallStamp::from_unix_ms(0);
        assert_eq!(stamp.year, 1970);
        assert_eq!(stamp.month, 1);
        assert_eq!(stamp.day, 1);
        assert_eq!(stamp.hour, 0);
        assert_eq!(stamp.millis, 0);
        let next = WallStamp::from_unix_ms(MS_PER_DAY + 1);
        assert_eq!(next.day, 2);
        assert_eq!(next.millis, 1);
    }
}
