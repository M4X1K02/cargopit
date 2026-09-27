//! UDP receive path. ACR packets update telemetry; other packets go to simapi.

use std::net::UdpSocket;
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicU16, Ordering};

use cargopit_devices::clock::Clock;
use cargopit_devices::telemetry::Telemetry;
use simapi_sys::{GameSession, SimDataBuf};

use crate::acr;
use crate::games::{self, PacketRoute};

static REQUESTED_PORT: AtomicU16 = AtomicU16::new(0);
const REUSE_ON: libc::c_int = 1;
const SOCKET_PROTOCOL_DEFAULT: libc::c_int = 0;

pub fn requested_port() -> u16 {
    REQUESTED_PORT.load(Ordering::Relaxed)
}

pub fn remember_port(port: i32) -> i32 {
    if port < 0 || port > i32::from(u16::MAX) {
        return games::UDP_BIND_FAILED;
    }
    REQUESTED_PORT.store(games::bind_port(port as u16), Ordering::Relaxed);
    games::UDP_BIND_OK
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
    bind_reuse(port)
}

fn bind_reuse(port: u16) -> Option<UdpSocket> {
    let fd = open_udp_socket()?;
    if !prepare_udp_socket(fd, port) {
        unsafe { libc::close(fd) };
        return None;
    }
    Some(unsafe { UdpSocket::from_raw_fd(fd) })
}

fn open_udp_socket() -> Option<libc::c_int> {
    let fd = unsafe {
        libc::socket(
            libc::AF_INET,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            SOCKET_PROTOCOL_DEFAULT,
        )
    };
    if fd < 0 {
        return None;
    }
    Some(fd)
}

fn prepare_udp_socket(fd: libc::c_int, port: u16) -> bool {
    if !set_reuse_address(fd) || !bind_ipv4(fd, port) {
        return false;
    }
    set_nonblocking(fd)
}

fn set_reuse_address(fd: libc::c_int) -> bool {
    let enabled = REUSE_ON;
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &enabled as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        ) == 0
    }
}

fn bind_ipv4(fd: libc::c_int, port: u16) -> bool {
    let Ok(ip) = games::UDP_BIND_ADDRESS.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    let mut addr: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    addr.sin_family = libc::AF_INET as libc::sa_family_t;
    addr.sin_port = port.to_be();
    addr.sin_addr = libc::in_addr {
        s_addr: u32::from(ip).to_be(),
    };
    unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        ) == 0
    }
}

fn set_nonblocking(fd: libc::c_int) -> bool {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return false;
    }
    unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == 0 }
}

pub fn start(port: i32, socket: &mut Option<UdpSocket>) -> i32 {
    let remembered = remember_port(port);
    if remembered != games::UDP_BIND_OK {
        return remembered;
    }
    ensure_socket(socket)
}

pub fn ensure_socket(socket: &mut Option<UdpSocket>) -> i32 {
    let port = requested_port();
    if port == 0 {
        return games::UDP_BIND_FAILED;
    }
    if socket_port(socket.as_ref()) == Some(port) {
        return games::UDP_BIND_OK;
    }
    *socket = bind_requested();
    if socket.is_some() {
        games::UDP_BIND_OK
    } else {
        games::UDP_BIND_FAILED
    }
}

fn socket_port(socket: Option<&UdpSocket>) -> Option<u16> {
    socket?.local_addr().ok().map(|addr| addr.port())
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
        assert_eq!(
            remember_port(crate::games::DR2_GAME_PORT as i32),
            games::UDP_BIND_OK
        );
        assert_eq!(requested_port(), crate::games::DR2_BIND_PORT);
        assert_eq!(remember_port(-1), games::UDP_BIND_FAILED);
        assert_eq!(games::UDP_BIND_OK, 0);
    }

    const EPHEMERAL_PORT: u16 = 0;

    #[test]
    fn reuse_binds_a_second_socket_on_the_same_port() {
        let first = bind_reuse(EPHEMERAL_PORT).expect("first socket");
        let port = first.local_addr().expect("first port").port();
        let second = bind_reuse(port).expect("second socket");
        assert_eq!(second.local_addr().expect("second port").port(), port);
    }
}
