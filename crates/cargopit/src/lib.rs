//! Host scheduler, CLI, ACR bridge, and play/test startup.

pub mod acr;
pub mod cli;
pub mod control;
pub mod devices;
pub mod games;
pub mod log;
pub mod scheduler;
pub mod simd;
pub mod sound_host;
pub mod tach;
pub mod testmode;
pub mod tyres;
pub mod udp;

pub use cargopit_devices::clock::{Clock, VirtualClock};
