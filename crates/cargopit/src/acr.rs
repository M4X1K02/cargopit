//! Assetto Corsa Rally 48-byte `ACRM` UDP bridge.

use cargopit_devices::clock::Clock;
use cargopit_devices::telemetry::Telemetry;

const PACKET_BYTES: usize = 48;
const MAGIC: &[u8; 4] = b"ACRM";
const REDLINE_FLOOR: u32 = 5000;
const IDLE_DEFAULT: u32 = 900;
const WHEEL_COUNT: usize = 4;
const FLOAT_BYTES: usize = 4;
const STATUS_ACTIVE: u32 = 2;
const SIM_ASSETTO_CORSA: u8 = 1;
pub const SIMEXE_ACR: u64 = 3_917_090;
const RPM_OFFSET: usize = 4;
const REDLINE_OFFSET: usize = 8;
const IDLE_OFFSET: usize = 12;
const BRAKE_TEMP_OFFSET: usize = 16;
const BRAKE_OFFSET: usize = 44;

pub const UDP_PORT: u16 = 20999;
pub const PHYSICS_PROBE_BYTES: usize = 64;
pub const PHYSICS_SHM_PATH: &str = "/dev/shm/acpmf_physics";
pub const SIMEXE_DIRT_RALLY_2: u64 = 690_790;

pub fn packet_ok(buf: &[u8]) -> bool {
    buf.len() >= PACKET_BYTES && buf.starts_with(MAGIC)
}

pub fn physics_shm_is_blank(bytes: Option<&[u8]>) -> bool {
    let Some(buf) = bytes else {
        return true;
    };
    if buf.is_empty() {
        return true;
    }
    let probe = buf.len().min(PHYSICS_PROBE_BYTES);
    buf[..probe].iter().all(|byte| *byte == 0)
}

pub fn needs_udp_bridge(simexe: u64, physics: Option<&[u8]>) -> bool {
    simexe == SIMEXE_ACR && physics_shm_is_blank(physics)
}

pub fn publish(dest: &mut [u8], sim: &Telemetry) -> bool {
    sim.copy_into(dest)
}

pub fn apply(sim: &mut Telemetry, buf: &[u8], clock: &impl Clock) {
    if !packet_ok(buf) {
        return;
    }
    let mut redline = rpm_to_u32(read_f32(buf, REDLINE_OFFSET));
    if redline == 0 {
        redline = REDLINE_FLOOR;
    }
    let mut idle = rpm_to_u32(read_f32(buf, IDLE_OFFSET));
    if idle == 0 {
        idle = IDLE_DEFAULT;
    }
    sim.set_mtick(clock.monotonic_ms());
    sim.set_rpms(rpm_to_u32(read_f32(buf, RPM_OFFSET)));
    sim.set_maxrpm(redline);
    sim.set_idlerpm(idle);
    let mut gas = 0.0;
    if redline > 0 {
        gas = f64::from(sim.rpms()) / f64::from(redline);
    }
    sim.set_gas(gas.clamp(0.0, 1.0));
    sim.set_brake(read_f32(buf, BRAKE_OFFSET) as f64);
    if sim.brake() < 0.0 {
        sim.set_brake(0.0);
    }
    if sim.brake() > 1.0 {
        sim.set_brake(1.0);
    }
    for index in 0..WHEEL_COUNT {
        let offset = BRAKE_TEMP_OFFSET + index * FLOAT_BYTES;
        sim.set_brake_temp(index, f64::from(read_f32(buf, offset)));
    }
    sim.set_simon(true);
    sim.set_simstatus(STATUS_ACTIVE);
    sim.set_simapi(SIM_ASSETTO_CORSA);
    sim.set_simexe(SIMEXE_ACR);
}

fn rpm_to_u32(rpm: f32) -> u32 {
    if rpm <= 0.0 || rpm.is_nan() {
        return 0;
    }
    if rpm > u32::MAX as f32 {
        return u32::MAX;
    }
    (rpm + 0.5) as u32
}

fn read_f32(buf: &[u8], offset: usize) -> f32 {
    let bytes: [u8; FLOAT_BYTES] = buf[offset..offset + FLOAT_BYTES]
        .try_into()
        .unwrap_or([0; 4]);
    f32::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargopit_devices::VirtualClock;

    fn sample_packet() -> [u8; PACKET_BYTES] {
        let mut buf = [0u8; PACKET_BYTES];
        buf[..4].copy_from_slice(MAGIC);
        buf[RPM_OFFSET..RPM_OFFSET + 4].copy_from_slice(&4200.4f32.to_le_bytes());
        buf[REDLINE_OFFSET..REDLINE_OFFSET + 4].copy_from_slice(&7500.0f32.to_le_bytes());
        buf[IDLE_OFFSET..IDLE_OFFSET + 4].copy_from_slice(&900.0f32.to_le_bytes());
        buf[BRAKE_TEMP_OFFSET..BRAKE_TEMP_OFFSET + 4].copy_from_slice(&410.0f32.to_le_bytes());
        buf[BRAKE_OFFSET..BRAKE_OFFSET + 4].copy_from_slice(&0.35f32.to_le_bytes());
        buf
    }

    #[test]
    fn acrm_packet_matches_the_c_mapping() {
        assert_eq!(PACKET_BYTES, 48);
        assert!(!packet_ok(&[0, 0, 0, 0]));
        let packet = sample_packet();
        assert!(packet_ok(&packet));
        let mut sim = Telemetry::new();
        let mut clock = VirtualClock::new();
        clock.advance_us(1_000_000);
        apply(&mut sim, &packet, &clock);
        assert_eq!(sim.rpms(), 4200);
        assert_eq!(sim.maxrpm(), 7500);
        assert_eq!(sim.idlerpm(), 900);
        assert!((0.55..0.57).contains(&sim.gas()));
        assert!((0.34..0.36).contains(&sim.brake()));
        assert!((409.0..411.0).contains(&sim.brake_temp(0)));
        assert_eq!(sim.simexe(), SIMEXE_ACR);
        assert_eq!(sim.simstatus(), STATUS_ACTIVE);
        assert_eq!(sim.mtick(), 1000);
        assert!(sim.simon());
        assert!(!needs_udp_bridge(SIMEXE_DIRT_RALLY_2, None));
        assert!(needs_udp_bridge(SIMEXE_ACR, None));
        assert!(!needs_udp_bridge(SIMEXE_ACR, Some(&[1, 0, 0, 0])));
        assert!(!publish(&mut [0u8; 4], &sim));
        let mut published = vec![0u8; sim.byte_len()];
        assert!(publish(&mut published, &sim));
    }
}
