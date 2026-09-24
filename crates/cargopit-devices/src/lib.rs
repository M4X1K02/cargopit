//! Device core: `SimDevice`, the injectable clock, transports, the Lua host, and haptic math.

pub mod clock;
pub mod device;
pub mod haptic;
pub mod lua_host;
pub mod telemetry;
pub mod transport;

pub use clock::{Clock, SystemClock, VirtualClock};
pub use device::{tick_interval_ms, DeviceKind, SimDevice, DEFAULT_DEVICE_FPS, MS_PER_SECOND};
pub use haptic::{measure_tyre_diameter, HapticEffect, HapticSettings, TyreId, VibrationEffect};
pub use lua_host::{LuaHost, LuaLedMode};
pub use transport::{FakeHid, FakePulse, FakeSerial, FakeSysfs, TransportError};
