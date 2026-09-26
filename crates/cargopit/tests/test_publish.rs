//! Test mode copies each telemetry frame into `/dev/shm/SIMAPI.DAT`.

use cargopit::testmode::{self, RunOptions, TestSubject};
use cargopit_devices::DeviceKind;
use simapi_sys::bindings::SimAPIError_SIMAPI_ERROR_NONE;
use simapi_sys::{read_telemetry, GameSession};

const SHM_DIR: &str = "/dev/shm";
const SHM_NAME: &str = "SIMAPI.DAT";
const OPEN_FRAME: usize = 0;
const CLOSE_FRAME: usize = 1;

#[test]
fn test_mode_publishes_open_and_close_frames_to_simapi_dat() {
    let mut session = GameSession::new();
    let open_error = session.try_open_publish_map();
    assert_eq!(open_error, SimAPIError_SIMAPI_ERROR_NONE as i32);
    assert!(session.map_open());

    let mut subject = TestSubject::lights(DeviceKind::Usb);
    subject.active = false;
    let subjects = [subject];
    let options = RunOptions {
        subjects: &subjects,
        device_index: None,
        trace: false,
        publish: true,
    };
    let mut frames = Vec::new();
    let run = testmode::run_frames(&options, &mut |_| false, &mut |bytes| {
        assert!(session.publish_bytes(bytes));
        frames.push(bytes.to_vec());
    });

    assert_eq!(frames.len(), CLOSE_FRAME + 1);
    assert_eq!(run.tick_count, 1);
    assert_eq!(run.mtick, 1);
    assert!(!run.simon);
    let open = read_telemetry(&frames[OPEN_FRAME]).expect("open frame");
    let close = read_telemetry(&frames[CLOSE_FRAME]).expect("close frame");
    assert_eq!(open.mtick, 0);
    assert_eq!(open.simon, 1);
    assert_eq!(close.mtick, run.mtick);
    assert_eq!(close.simon, 0);
    assert_eq!(close.simstatus, run.simstatus);

    let published = session.published_bytes().expect("mapped bytes");
    assert_eq!(published, frames[CLOSE_FRAME].as_slice());
    let path = std::path::Path::new(SHM_DIR).join(SHM_NAME);
    let file = std::fs::read(&path).expect("SIMAPI.DAT");
    assert_eq!(file, frames[CLOSE_FRAME]);
}
