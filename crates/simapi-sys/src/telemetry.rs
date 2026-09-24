use crate::{
    CAR_BYTES, F64_SIZE, GEARC_BYTES, OFF_ABS, OFF_BRAKE, OFF_CAR, OFF_CLUTCH, OFF_FUEL,
    OFF_FUELCAPACITY, OFF_GAS, OFF_GEAR, OFF_GEARC, OFF_IDLERPM, OFF_LAP, OFF_MAXRPM, OFF_MTICK,
    OFF_NUMLAPS, OFF_POSITION, OFF_RPMS, OFF_SIMAPI, OFF_SIMAPIVERSION, OFF_SIMEXE, OFF_SIMON,
    OFF_SIMSTATUS, OFF_STEER, OFF_TRACK, OFF_VELOCITY, OFF_XVELOCITY, OFF_YVELOCITY, OFF_ZVELOCITY,
    SIMAPI_VERSION_VALUE, SIMDATA_SIZE, TRACK_BYTES,
};

pub const TELEMETRY_GEARC_LEN: usize = 4;
pub const TELEMETRY_NAME_LEN: usize = 32;
const NAME_COPY_LEN: usize = TELEMETRY_NAME_LEN - 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelemetrySnapshot {
    pub mtick: u64,
    pub simexe: u64,
    pub simstatus: u32,
    pub velocity: u32,
    pub rpms: u32,
    pub gear: u32,
    pub maxrpm: u32,
    pub idlerpm: u32,
    pub lap: u32,
    pub position: u32,
    pub numlaps: u32,
    pub simapi: u8,
    pub simon: u8,
    pub simapiversion: u8,
    pub valid: u8,
    pub gearc: [u8; TELEMETRY_GEARC_LEN],
    pub car: [u8; TELEMETRY_NAME_LEN],
    pub track: [u8; TELEMETRY_NAME_LEN],
    pub gas: f64,
    pub brake: f64,
    pub clutch: f64,
    pub steer: f64,
    pub fuel: f64,
    pub fuelcapacity: f64,
    pub abs: f64,
    pub xvelocity: f64,
    pub yvelocity: f64,
    pub zvelocity: f64,
}

impl Default for TelemetrySnapshot {
    fn default() -> Self {
        Self {
            mtick: 0,
            simexe: 0,
            simstatus: 0,
            velocity: 0,
            rpms: 0,
            gear: 0,
            maxrpm: 0,
            idlerpm: 0,
            lap: 0,
            position: 0,
            numlaps: 0,
            simapi: 0,
            simon: 0,
            simapiversion: 0,
            valid: 0,
            gearc: [0; TELEMETRY_GEARC_LEN],
            car: [0; TELEMETRY_NAME_LEN],
            track: [0; TELEMETRY_NAME_LEN],
            gas: 0.0,
            brake: 0.0,
            clutch: 0.0,
            steer: 0.0,
            fuel: 0.0,
            fuelcapacity: 0.0,
            abs: 0.0,
            xvelocity: 0.0,
            yvelocity: 0.0,
            zvelocity: 0.0,
        }
    }
}

pub fn read_telemetry(bytes: &[u8]) -> Option<TelemetrySnapshot> {
    if bytes.len() < SIMDATA_SIZE {
        return None;
    }
    let mut view = TelemetrySnapshot {
        mtick: read_u64(bytes, OFF_MTICK),
        simexe: read_u64(bytes, OFF_SIMEXE),
        simstatus: read_u32(bytes, OFF_SIMSTATUS),
        velocity: read_u32(bytes, OFF_VELOCITY),
        rpms: read_u32(bytes, OFF_RPMS),
        gear: read_u32(bytes, OFF_GEAR),
        maxrpm: read_u32(bytes, OFF_MAXRPM),
        idlerpm: read_u32(bytes, OFF_IDLERPM),
        lap: read_u32(bytes, OFF_LAP),
        position: read_u32(bytes, OFF_POSITION),
        numlaps: read_u32(bytes, OFF_NUMLAPS),
        simapi: bytes[OFF_SIMAPI],
        simon: u8::from(bytes[OFF_SIMON] != 0),
        simapiversion: bytes[OFF_SIMAPIVERSION],
        valid: 1,
        gas: read_f64(bytes, OFF_GAS),
        brake: read_f64(bytes, OFF_BRAKE),
        clutch: read_f64(bytes, OFF_CLUTCH),
        steer: read_f64(bytes, OFF_STEER),
        fuel: read_f64(bytes, OFF_FUEL),
        fuelcapacity: read_f64(bytes, OFF_FUELCAPACITY),
        abs: read_f64(bytes, OFF_ABS),
        xvelocity: read_f64(bytes, OFF_XVELOCITY),
        yvelocity: read_f64(bytes, OFF_YVELOCITY),
        zvelocity: read_f64(bytes, OFF_ZVELOCITY),
        ..TelemetrySnapshot::default()
    };
    copy_c_chars(&mut view.gearc, &bytes[OFF_GEARC..OFF_GEARC + GEARC_BYTES]);
    copy_c_chars(&mut view.car, &bytes[OFF_CAR..OFF_CAR + CAR_BYTES]);
    copy_c_chars(&mut view.track, &bytes[OFF_TRACK..OFF_TRACK + TRACK_BYTES]);
    Some(view)
}

pub fn write_telemetry(bytes: &mut [u8], view: &TelemetrySnapshot) -> bool {
    if bytes.len() < SIMDATA_SIZE {
        return false;
    }
    bytes[..SIMDATA_SIZE].fill(0);
    write_u64(bytes, OFF_MTICK, view.mtick);
    write_u64(bytes, OFF_SIMEXE, view.simexe);
    write_u32(bytes, OFF_SIMSTATUS, view.simstatus);
    write_u32(bytes, OFF_VELOCITY, view.velocity);
    write_u32(bytes, OFF_RPMS, view.rpms);
    write_u32(bytes, OFF_GEAR, view.gear);
    write_u32(bytes, OFF_MAXRPM, view.maxrpm);
    write_u32(bytes, OFF_IDLERPM, view.idlerpm);
    write_u32(bytes, OFF_LAP, view.lap);
    write_u32(bytes, OFF_POSITION, view.position);
    write_u32(bytes, OFF_NUMLAPS, view.numlaps);
    bytes[OFF_SIMAPI] = view.simapi;
    bytes[OFF_SIMON] = u8::from(view.simon != 0);
    bytes[OFF_SIMAPIVERSION] = if view.simapiversion == 0 {
        SIMAPI_VERSION_VALUE as u8
    } else {
        view.simapiversion
    };
    let gear_n = GEARC_BYTES.min(view.gearc.len());
    bytes[OFF_GEARC..OFF_GEARC + gear_n].copy_from_slice(&view.gearc[..gear_n]);
    copy_name(bytes, OFF_CAR, &view.car);
    copy_name(bytes, OFF_TRACK, &view.track);
    write_f64(bytes, OFF_GAS, view.gas);
    write_f64(bytes, OFF_BRAKE, view.brake);
    write_f64(bytes, OFF_CLUTCH, view.clutch);
    write_f64(bytes, OFF_STEER, view.steer);
    write_f64(bytes, OFF_FUEL, view.fuel);
    write_f64(bytes, OFF_FUELCAPACITY, view.fuelcapacity);
    write_f64(bytes, OFF_ABS, view.abs);
    write_f64(bytes, OFF_XVELOCITY, view.xvelocity);
    write_f64(bytes, OFF_YVELOCITY, view.yvelocity);
    write_f64(bytes, OFF_ZVELOCITY, view.zvelocity);
    true
}

fn copy_c_chars(dest: &mut [u8], src: &[u8]) {
    if dest.is_empty() {
        return;
    }
    let last = dest.len() - 1;
    let n = last.min(src.len());
    for (index, ch) in src.iter().take(n).copied().enumerate() {
        dest[index] = ch;
        if ch == 0 {
            return;
        }
    }
    dest[n] = 0;
}

fn copy_name(bytes: &mut [u8], offset: usize, src: &[u8]) {
    let n = NAME_COPY_LEN.min(src.len());
    bytes[offset..offset + n].copy_from_slice(&src[..n]);
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64"))
}

fn read_f64(bytes: &[u8], offset: usize) -> f64 {
    f64::from_le_bytes(bytes[offset..offset + F64_SIZE].try_into().expect("f64"))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_f64(bytes: &mut [u8], offset: usize, value: f64) {
    bytes[offset..offset + F64_SIZE].copy_from_slice(&value.to_le_bytes());
}
