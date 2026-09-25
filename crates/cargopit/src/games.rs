//! Game-path decisions that stay in the host: UDP ports, stale simd, ACR bridge.

use crate::acr;

pub const DAEMON_PROBE_US: u64 = 50_000;
pub const DR2_GAME_PORT: u16 = 20779;
pub const DR2_BIND_PORT: u16 = 20777;
pub const STATE_SEARCHING: &str = "searching";
pub const STATE_MAPPING: &str = "mapping";
pub const STATE_EXITING: &str = "exiting";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetrySource {
    Shm,
    Udp,
    Auto,
}

pub fn bind_port(requested: u16) -> u16 {
    if requested == DR2_GAME_PORT {
        return DR2_BIND_PORT;
    }
    requested
}

pub fn daemon_is_stale(before_mtick: u64, after_mtick: u64) -> bool {
    before_mtick == after_mtick
}

pub fn use_acr_bridge(simexe: u64, source: TelemetrySource, physics: Option<&[u8]>) -> bool {
    if simexe != acr::SIMEXE_ACR {
        return false;
    }
    if source == TelemetrySource::Shm {
        return false;
    }
    source == TelemetrySource::Udp || acr::physics_shm_is_blank(physics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dr2_bind_and_stale_mtick_match_the_c_host() {
        assert_eq!(bind_port(DR2_GAME_PORT), DR2_BIND_PORT);
        assert_eq!(bind_port(acr::UDP_PORT), acr::UDP_PORT);
        assert!(daemon_is_stale(10, 10));
        assert!(!daemon_is_stale(10, 11));
        assert_eq!(DAEMON_PROBE_US, 50_000);
    }

    #[test]
    fn only_blank_acr_uses_the_udp_bridge() {
        assert!(!use_acr_bridge(
            acr::SIMEXE_DIRT_RALLY_2,
            TelemetrySource::Auto,
            None
        ));
        assert!(!use_acr_bridge(acr::SIMEXE_ACR, TelemetrySource::Shm, None));
        assert!(use_acr_bridge(acr::SIMEXE_ACR, TelemetrySource::Auto, None));
        assert!(use_acr_bridge(
            acr::SIMEXE_ACR,
            TelemetrySource::Udp,
            Some(&[9])
        ));
    }
}
