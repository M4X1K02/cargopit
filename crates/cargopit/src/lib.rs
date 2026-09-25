//! Host scheduler, CLI, ACR bridge, and play/test startup.

pub mod acr;
pub mod cli;
pub mod control;
pub mod games;
pub mod log;
pub mod scheduler;
pub mod simd;
pub mod tach;
pub mod testmode;

pub use cargopit_devices::clock::{Clock, VirtualClock};
