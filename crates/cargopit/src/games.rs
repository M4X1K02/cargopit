//! Game-path decisions that stay in the host: UDP ports, stale simd, ACR bridge.

use crate::acr;

pub const DAEMON_PROBE_US: u64 = 50_000;
pub const DR2_GAME_PORT: u16 = 20779;
pub const DR2_BIND_PORT: u16 = 20777;
pub const STATE_SEARCHING: &str = "searching";
pub const STATE_MAPPING: &str = "mapping";
pub const STATE_EXITING: &str = "exiting";
pub const MAP_API_SIMD: i32 = 0;
pub const STATUS_MENU: i32 = 1;
pub const STATUS_ACTIVE_PLAY: i32 = 2;
pub const CHECK_INTERVAL_MS: u64 = 1000;
pub const QUIT_KEY: u8 = b'q';
pub const MAPPING_START_MS: u64 = 2000;
const MS_PER_SECOND: f64 = 1000.0;
const HALF_MS: f64 = 0.5;
const MIN_MAP_INTERVAL_MS: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetrySource {
    Shm,
    Udp,
    Auto,
}

pub const UDP_BIND_ADDRESS: &str = "0.0.0.0";
pub const UDP_RECV_BYTES: usize = 65536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketRoute {
    Ignore,
    Acr,
    Datamap,
}

pub fn route_udp(packet: &[u8]) -> PacketRoute {
    if packet.is_empty() {
        return PacketRoute::Ignore;
    }
    if acr::packet_ok(packet) {
        return PacketRoute::Acr;
    }
    PacketRoute::Datamap
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeenSim {
    pub is_sim_on: bool,
    pub sim_status: i32,
    pub map_api: i32,
    pub uses_udp: bool,
    pub sim_exe: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayPhase {
    Searching,
    Mapping,
    Exiting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayAction {
    Wait,
    StartMapping { use_udp: bool },
    Release,
}

pub fn simd_map_is_stale(map_api: i32, daemon_advancing: bool) -> bool {
    map_api == MAP_API_SIMD && !daemon_advancing
}

pub fn bridged_acr(seen: SeenSim) -> SeenSim {
    SeenSim {
        is_sim_on: true,
        sim_status: STATUS_ACTIVE_PLAY,
        uses_udp: true,
        ..seen
    }
}

pub fn search_tick(seen: SeenSim, force_udp: bool) -> PlayAction {
    if !seen.is_sim_on || seen.sim_status < STATUS_ACTIVE_PLAY {
        return PlayAction::Wait;
    }
    PlayAction::StartMapping {
        use_udp: force_udp || seen.uses_udp,
    }
}

pub fn map_interval_ms(fps: i32) -> u64 {
    let clamped = crate::scheduler::clamp_fps(fps) as f64;
    let interval = (MS_PER_SECOND / clamped + HALF_MS) as u64;
    if interval == 0 {
        return MIN_MAP_INTERVAL_MS;
    }
    interval
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitAction {
    Continue,
    Release,
    Exit,
}

pub fn quit_action(key: Option<u8>, signalled: bool, mapping: bool) -> QuitAction {
    if signalled {
        return QuitAction::Exit;
    }
    if key != Some(QUIT_KEY) {
        return QuitAction::Continue;
    }
    if mapping {
        return QuitAction::Release;
    }
    QuitAction::Exit
}

pub fn mapping_should_stop(seen: SeenSim) -> bool {
    !seen.is_sim_on || seen.sim_status <= STATUS_MENU
}

pub fn mapping_tick(seen: SeenSim) -> PlayAction {
    if seen.map_api != MAP_API_SIMD {
        return PlayAction::Wait;
    }
    if !seen.is_sim_on {
        return PlayAction::Release;
    }
    PlayAction::Wait
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSnapshot {
    frame: Vec<u8>,
    sequence: u64,
}

impl FrameSnapshot {
    pub fn new() -> Self {
        Self {
            frame: Vec::new(),
            sequence: 0,
        }
    }

    pub fn publish(&mut self, frame: &[u8]) {
        self.frame.clear();
        self.frame.extend_from_slice(frame);
        self.sequence = self.sequence.wrapping_add(1);
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn frame(&self) -> &[u8] {
        &self.frame
    }
}

impl Default for FrameSnapshot {
    fn default() -> Self {
        Self::new()
    }
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
    fn udp_packets_follow_the_c_routes() {
        assert_eq!(route_udp(&[]), PacketRoute::Ignore);
        assert_eq!(route_udp(&[0, 1]), PacketRoute::Datamap);
        let mut acr = vec![0u8; 48];
        acr[..4].copy_from_slice(b"ACRM");
        assert_eq!(route_udp(&acr), PacketRoute::Acr);
        assert_eq!(UDP_BIND_ADDRESS, "0.0.0.0");
        assert_eq!(UDP_RECV_BYTES, 65536);
    }

    #[test]
    fn quit_key_releases_while_mapping_and_exits_while_searching() {
        assert_eq!(quit_action(None, true, true), QuitAction::Exit);
        assert_eq!(quit_action(Some(b'x'), false, true), QuitAction::Continue);
        assert_eq!(
            quit_action(Some(QUIT_KEY), false, true),
            QuitAction::Release
        );
        assert_eq!(quit_action(Some(QUIT_KEY), false, false), QuitAction::Exit);
    }

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

    fn live(map_api: i32, uses_udp: bool) -> SeenSim {
        SeenSim {
            is_sim_on: true,
            sim_status: STATUS_ACTIVE_PLAY,
            map_api,
            uses_udp,
            sim_exe: 0,
        }
    }

    #[test]
    fn search_waits_until_the_sim_is_in_active_play() {
        let off = SeenSim {
            is_sim_on: false,
            sim_status: STATUS_ACTIVE_PLAY,
            map_api: MAP_API_SIMD,
            uses_udp: false,
            sim_exe: 0,
        };
        let menu = SeenSim {
            sim_status: STATUS_MENU,
            ..live(MAP_API_SIMD, false)
        };
        assert_eq!(search_tick(off, false), PlayAction::Wait);
        assert_eq!(search_tick(menu, false), PlayAction::Wait);
        assert_eq!(
            search_tick(live(1, false), false),
            PlayAction::StartMapping { use_udp: false }
        );
        assert_eq!(
            search_tick(live(1, false), true),
            PlayAction::StartMapping { use_udp: true }
        );
    }

    #[test]
    fn stale_simd_map_asks_for_a_direct_remap() {
        assert!(simd_map_is_stale(MAP_API_SIMD, false));
        assert!(!simd_map_is_stale(MAP_API_SIMD, true));
        assert!(!simd_map_is_stale(1, false));
    }

    #[test]
    fn mapping_releases_only_a_stopped_simd_session() {
        assert_eq!(mapping_tick(live(1, true)), PlayAction::Wait);
        assert_eq!(
            mapping_tick(SeenSim {
                is_sim_on: false,
                ..live(1, true)
            }),
            PlayAction::Wait
        );
        assert_eq!(mapping_tick(live(MAP_API_SIMD, false)), PlayAction::Wait);
        assert_eq!(
            mapping_tick(SeenSim {
                is_sim_on: false,
                ..live(MAP_API_SIMD, false)
            }),
            PlayAction::Release
        );
    }

    #[test]
    fn map_interval_matches_the_c_rounding() {
        assert_eq!(map_interval_ms(60), 17);
        assert_eq!(map_interval_ms(1), 1000);
        assert_eq!(map_interval_ms(0), 1000);
        assert_eq!(MAPPING_START_MS, 2000);
    }

    #[test]
    fn mapping_stops_when_the_sim_leaves_active_play() {
        let live = SeenSim {
            is_sim_on: true,
            sim_status: STATUS_ACTIVE_PLAY,
            map_api: 1,
            uses_udp: false,
            sim_exe: 0,
        };
        assert!(!mapping_should_stop(live));
        assert!(mapping_should_stop(SeenSim {
            is_sim_on: false,
            ..live
        }));
        assert!(mapping_should_stop(SeenSim {
            sim_status: STATUS_MENU,
            ..live
        }));
    }

    #[test]
    fn publish_copies_the_frame_and_bumps_sequence() {
        let mut snapshot = FrameSnapshot::new();
        snapshot.publish(&[1, 2, 3]);
        assert_eq!(snapshot.sequence(), 1);
        assert_eq!(snapshot.frame(), &[1, 2, 3]);
        snapshot.publish(&[9]);
        assert_eq!(snapshot.sequence(), 2);
        assert_eq!(snapshot.frame(), &[9]);
    }

    #[test]
    fn acr_bridge_starts_udp_mapping() {
        let seen = bridged_acr(SeenSim {
            is_sim_on: false,
            sim_status: 0,
            map_api: MAP_API_SIMD,
            uses_udp: false,
            sim_exe: acr::SIMEXE_ACR,
        });
        assert_eq!(
            search_tick(seen, false),
            PlayAction::StartMapping { use_udp: true }
        );
    }
}
