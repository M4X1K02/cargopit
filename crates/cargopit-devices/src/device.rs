//! Shared device record. Concrete USB, serial, and sound encoders stay in later phases.

pub const MS_PER_SECOND: u64 = 1000;
pub const DEFAULT_DEVICE_FPS: u32 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Unknown = 0,
    Serial = 1,
    Usb = 2,
    Sound = 3,
}

#[derive(Clone, Debug)]
pub struct SimDevice {
    id: i32,
    fps: u32,
    initialized: bool,
    config_file: Option<String>,
    kind: DeviceKind,
}

impl SimDevice {
    pub fn new(id: i32, fps: u32, kind: DeviceKind) -> Self {
        Self {
            id,
            fps,
            initialized: false,
            config_file: None,
            kind,
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn kind(&self) -> DeviceKind {
        self.kind
    }

    pub fn initialized(&self) -> bool {
        self.initialized
    }

    pub fn set_initialized(&mut self, initialized: bool) {
        self.initialized = initialized;
    }

    pub fn config_file(&self) -> Option<&str> {
        self.config_file.as_deref()
    }

    pub fn set_config_file(&mut self, path: Option<String>) {
        self.config_file = path;
    }

    /// Integer `1000 / fps` milliseconds. At 60 fps the interval is 16 ms.
    pub fn interval_ms(&self) -> u64 {
        tick_interval_ms(self.fps)
    }
}

pub fn tick_interval_ms(fps: u32) -> u64 {
    let fps = if fps == 0 { DEFAULT_DEVICE_FPS } else { fps };
    MS_PER_SECOND / u64::from(fps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fps_interval_is_sixteen_milliseconds() {
        let device = SimDevice::new(1, DEFAULT_DEVICE_FPS, DeviceKind::Sound);
        assert_eq!(device.interval_ms(), 16);
        assert_eq!(tick_interval_ms(0), 16);
    }
}
