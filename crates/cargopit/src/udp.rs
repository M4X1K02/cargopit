//! UDP receive path. ACR packets update telemetry; other packets go to simapi.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicU16, Ordering};

use cargopit_devices::clock::Clock;
use cargopit_devices::telemetry::Telemetry;
use simapi_sys::{GameSession, SimDataBuf};

use crate::acr;
use crate::games::{self, PacketRoute};

static REQUESTED_PORT: AtomicU16 = AtomicU16::new(0);

pub fn requested_port() -> u16 {
    REQUESTED_PORT.load(Ordering::Relaxed)
}

pub fn remember_port(port: i32) -> i32 {
    if port < 0 || port > u16::MAX as i32 {
        return -1;
    }
    REQUESTED_PORT.store(games::bind_port(port as u16), Ordering::Relaxed);
    0
}

/// # Safety
/// `port` is the integer simapi passes to its UDP setup callback. The function
/// only stores the remapped port and does not dereference pointers.
pub unsafe extern "C" fn setup_udp(port: i32) -> i32 {
    remember_port(port)
}

pub fn bind_requested() -> Option<UdpSocket> {
    let port = requested_port();
    if port == 0 {
        return None;
    }
    let socket = UdpSocket::bind((games::UDP_BIND_ADDRESS, port)).ok()?;
    socket.set_nonblocking(true).ok()?;
    Some(socket)
}

pub fn recv_packet(socket: &UdpSocket) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; games::UDP_RECV_BYTES];
    let size = socket.recv(&mut buf).ok()?;
    if size == 0 {
        return None;
    }
    buf.truncate(size);
    Some(buf)
}

pub fn ingest(
    session: &mut GameSession,
    packet: &mut [u8],
    map_api: i32,
    clock: &impl Clock,
) -> bool {
    match games::route_udp(packet) {
        PacketRoute::Ignore => false,
        PacketRoute::Acr => apply_acr(session, packet, clock),
        PacketRoute::Datamap => {
            session.map_packet(map_api, packet);
            true
        }
    }
}

fn apply_acr(session: &mut GameSession, packet: &[u8], clock: &impl Clock) -> bool {
    let Some(buf) = SimDataBuf::from_bytes(session.frame_bytes()) else {
        return false;
    };
    let mut sim = Telemetry::from_buf(buf);
    acr::apply(&mut sim, packet, clock);
    let mut bytes = vec![0u8; sim.byte_len()];
    if !sim.copy_into(&mut bytes) {
        return false;
    }
    session.write_frame(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dr2_port_is_remembered_on_the_native_bind_port() {
        assert_eq!(remember_port(crate::games::DR2_GAME_PORT as i32), 0);
        assert_eq!(requested_port(), crate::games::DR2_BIND_PORT);
        assert_eq!(remember_port(-1), -1);
    }
}
