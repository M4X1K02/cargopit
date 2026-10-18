//! Phyphox seat sweep and PipeWire tactile-filter apply, driven from the TUI.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use anyhow::{anyhow, Context, Result};

use crate::config::DeviceEntry;
use crate::consts;
use crate::hardware::{Discovery, HardwareChoice};
use crate::paths;
use crate::schema::DeviceClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Check,
    Run,
}

struct Job {
    child: Child,
    kind: JobKind,
    rx: Receiver<String>,
}

pub struct SeatSweep {
    pub phyphox_url: String,
    pub sink: String,
    pub field: usize,
    pub edit_buffer: Option<String>,
    pub status: String,
    pub lines: Vec<String>,
    pub csv_path: PathBuf,
    pub amp_sink: String,
    pub device_index: Option<usize>,
    pub parent: crate::app::Screen,
    job: Option<Job>,
}

impl Default for SeatSweep {
    fn default() -> Self {
        Self {
            phyphox_url: String::new(),
            sink: String::new(),
            field: consts::SEAT_SWEEP_FIELD_URL,
            edit_buffer: None,
            status: consts::SEAT_SWEEP_STATUS_IDLE.to_string(),
            lines: Vec::new(),
            csv_path: default_csv_path(),
            amp_sink: String::new(),
            device_index: None,
            parent: crate::app::Screen::Settings,
            job: None,
        }
    }
}

impl SeatSweep {
    pub fn prepare(
        &mut self,
        discovery: &Discovery,
        devices: &[DeviceEntry],
        selected: usize,
        parent: crate::app::Screen,
    ) {
        self.abort_job();
        self.lines.clear();
        self.edit_buffer = None;
        self.field = consts::SEAT_SWEEP_FIELD_URL;
        self.status = consts::SEAT_SWEEP_STATUS_IDLE.to_string();
        self.csv_path = default_csv_path();
        self.parent = parent;
        self.device_index = sound_index(devices, selected);
        self.sink = default_amp_sink(discovery, devices, self.device_index);
        self.amp_sink = self.sink.clone();
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn start_check(&mut self) -> Result<()> {
        self.start_job(JobKind::Check)
    }

    pub fn start_run(&mut self) -> Result<()> {
        self.amp_sink = self.sink.clone();
        self.start_job(JobKind::Run)
    }

    fn start_job(&mut self, kind: JobKind) -> Result<()> {
        if self.busy() {
            return Err(anyhow!(consts::MSG_SEAT_SWEEP_BUSY));
        }
        let url = self.phyphox_url.trim();
        if url.is_empty() {
            return Err(anyhow!(consts::MSG_SEAT_SWEEP_NEED_URL));
        }
        if kind == JobKind::Run && self.sink.trim().is_empty() {
            return Err(anyhow!(consts::MSG_SEAT_SWEEP_NEED_SINK));
        }
        if let Some(parent) = self.csv_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut cmd = Command::new(python_bin());
        cmd.arg(seat_sweep_script())
            .arg(consts::FLAG_PHYPOX)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match kind {
            JobKind::Check => {
                cmd.arg(consts::FLAG_CHECK);
            }
            JobKind::Run => {
                cmd.arg(consts::FLAG_SINK)
                    .arg(&self.sink)
                    .arg(consts::FLAG_OUTPUT)
                    .arg(&self.csv_path);
            }
        }
        let mut child = cmd.spawn().context(consts::MSG_SEAT_SWEEP_SPAWN)?;
        let (tx, rx) = mpsc::channel();
        spawn_line_reader(child.stdout.take(), tx.clone());
        spawn_line_reader(child.stderr.take(), tx);
        self.lines.clear();
        self.status = match kind {
            JobKind::Check => consts::SEAT_SWEEP_STATUS_CHECK,
            JobKind::Run => consts::SEAT_SWEEP_STATUS_RUN,
        }
        .to_string();
        self.job = Some(Job { child, kind, rx });
        Ok(())
    }

    pub fn poll(&mut self) -> Option<JobKind> {
        self.drain_lines();
        let outcome = {
            let job = self.job.as_mut()?;
            match job.child.try_wait() {
                Ok(Some(status)) => Some(Ok((job.kind, status.success()))),
                Ok(None) => None,
                Err(err) => Some(Err(err.to_string())),
            }
        };
        let result = outcome?;
        self.drain_lines();
        self.job = None;
        match result {
            Ok((kind, true)) => {
                self.status = match kind {
                    JobKind::Check => consts::SEAT_SWEEP_STATUS_CHECKED,
                    JobKind::Run => consts::SEAT_SWEEP_STATUS_MEASURED,
                }
                .to_string();
                Some(kind)
            }
            Ok((_, false)) => {
                self.status = consts::SEAT_SWEEP_STATUS_FAILED.to_string();
                None
            }
            Err(err) => {
                self.status = format!("{}: {err}", consts::SEAT_SWEEP_STATUS_FAILED);
                None
            }
        }
    }

    pub fn abort_job(&mut self) {
        if let Some(mut job) = self.job.take() {
            let _ = job.child.kill();
            let _ = job.child.wait();
            self.status = consts::SEAT_SWEEP_STATUS_STOPPED.to_string();
        }
    }

    pub fn cycle_sink(&mut self, discovery: &Discovery, delta: i32) {
        let sinks = amplifier_sinks(discovery);
        if sinks.is_empty() {
            return;
        }
        let current = sinks.iter().position(|sink| sink.value == self.sink);
        let len = sinks.len() as i32;
        let next = match current {
            Some(index) => (index as i32 + delta).rem_euclid(len) as usize,
            None => 0,
        };
        self.sink = sinks[next].value.clone();
    }

    pub fn begin_edit(&mut self) {
        let value = match self.field {
            consts::SEAT_SWEEP_FIELD_URL => self.phyphox_url.clone(),
            consts::SEAT_SWEEP_FIELD_SINK => self.sink.clone(),
            _ => return,
        };
        self.edit_buffer = Some(value);
    }

    pub fn commit_edit(&mut self) {
        let Some(buffer) = self.edit_buffer.take() else {
            return;
        };
        let value = buffer.trim().to_string();
        match self.field {
            consts::SEAT_SWEEP_FIELD_URL => self.phyphox_url = value,
            consts::SEAT_SWEEP_FIELD_SINK => self.sink = value,
            _ => {}
        }
    }

    pub fn cancel_edit(&mut self) {
        self.edit_buffer = None;
    }

    fn drain_lines(&mut self) {
        loop {
            let next = match self.job.as_mut() {
                Some(job) => job.rx.try_recv(),
                None => return,
            };
            match next {
                Ok(line) => self.push_line(line),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return,
            }
        }
    }

    fn push_line(&mut self, line: String) {
        if self.lines.len() >= consts::CHILD_OUTPUT_MAX_LINES {
            self.lines.remove(0);
        }
        self.lines.push(line);
    }
}

fn spawn_line_reader<R: std::io::Read + Send + 'static>(pipe: Option<R>, tx: mpsc::Sender<String>) {
    let Some(pipe) = pipe else {
        return;
    };
    thread::spawn(move || {
        let reader = BufReader::new(pipe);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

pub fn default_csv_path() -> PathBuf {
    paths::config_home()
        .join(consts::CONFIG_DIR_NAME)
        .join(consts::SEAT_SWEEP_CSV_NAME)
}

pub fn tactile_conf_path() -> PathBuf {
    paths::config_home()
        .join(consts::PIPEWIRE_DIRNAME)
        .join(consts::PIPEWIRE_CONF_D)
        .join(consts::TACTILE_CONF_NAME)
}

pub fn python_bin() -> PathBuf {
    if let Some(path) = which_bin(consts::PYTHON_BIN) {
        return path;
    }
    let fallback = PathBuf::from(consts::PYTHON_FALLBACK);
    if fallback.is_file() {
        return fallback;
    }
    PathBuf::from(consts::PYTHON_BIN)
}

pub fn systemctl_bin() -> PathBuf {
    env_path(consts::ENV_CARGOPIT_SYSTEMCTL).unwrap_or_else(|| PathBuf::from(consts::SYSTEMCTL_BIN))
}

pub fn seat_sweep_script() -> PathBuf {
    env_path(consts::ENV_CARGOPIT_SEAT_SWEEP).unwrap_or_else(haptics_script_seat_sweep)
}

pub fn filterchain_script() -> PathBuf {
    env_path(consts::ENV_CARGOPIT_FILTERCHAIN).unwrap_or_else(haptics_script_filterchain)
}

fn haptics_script_seat_sweep() -> PathBuf {
    haptics_dir().join(consts::SEAT_SWEEP_PY)
}

fn haptics_script_filterchain() -> PathBuf {
    haptics_dir().join(consts::FILTERCHAIN_PY)
}

fn haptics_dir() -> PathBuf {
    paths::source_root()
        .join(consts::TOOLS_DIRNAME)
        .join(consts::HAPTICS_DIRNAME)
}

fn env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

fn which_bin(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os(consts::ENV_PATH)?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn amplifier_sinks(discovery: &Discovery) -> Vec<HardwareChoice> {
    let hardware: Vec<HardwareChoice> = discovery
        .pulse_sinks
        .iter()
        .filter(|sink| is_amplifier_sink(sink))
        .cloned()
        .collect();
    if !hardware.is_empty() {
        return hardware;
    }
    discovery
        .pulse_sinks
        .iter()
        .filter(|sink| sink.value != consts::TACTILE_SINK_NAME)
        .cloned()
        .collect()
}

fn is_amplifier_sink(sink: &HardwareChoice) -> bool {
    if sink.value == consts::TACTILE_SINK_NAME {
        return false;
    }
    !sink.label.contains(consts::LABEL_SINK_PROCESSOR_TAG)
}

pub fn default_amp_sink(
    discovery: &Discovery,
    devices: &[DeviceEntry],
    selected: Option<usize>,
) -> String {
    if let Some(devid) = usable_amp_devid_at(devices, selected) {
        return devid;
    }
    if let Some(devid) = devices.iter().find_map(usable_amp_devid) {
        return devid;
    }
    amplifier_sinks(discovery)
        .first()
        .map(|sink| sink.value.clone())
        .unwrap_or_default()
}

fn usable_amp_devid_at(devices: &[DeviceEntry], selected: Option<usize>) -> Option<String> {
    usable_amp_devid(devices.get(selected?)?)
}

fn usable_amp_devid(device: &DeviceEntry) -> Option<String> {
    if device.class() != DeviceClass::Sound {
        return None;
    }
    let devid = device.get_str(consts::KEY_DEVID)?;
    if devid.is_empty() || devid == consts::TACTILE_SINK_NAME {
        return None;
    }
    Some(devid.to_string())
}

fn sound_index(devices: &[DeviceEntry], selected: usize) -> Option<usize> {
    if devices
        .get(selected)
        .is_some_and(|device| device.class() == DeviceClass::Sound)
    {
        return Some(selected);
    }
    devices
        .iter()
        .position(|device| device.class() == DeviceClass::Sound)
}

pub fn write_filter_chain(csv: &Path, target_sink: &str, channels: i64, dest: &Path) -> Result<()> {
    if !csv.is_file() {
        return Err(anyhow!(consts::MSG_SEAT_SWEEP_NO_CSV));
    }
    if target_sink.is_empty() {
        return Err(anyhow!(consts::MSG_SEAT_SWEEP_NEED_SINK));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let output = Command::new(python_bin())
        .arg(filterchain_script())
        .arg(csv)
        .arg(consts::FLAG_TARGET_SINK)
        .arg(target_sink)
        .arg(consts::FLAG_CHANNELS)
        .arg(channels.to_string())
        .output()
        .context(consts::MSG_FILTERCHAIN_SPAWN)?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("{}: {err}", consts::MSG_FILTERCHAIN_FAILED));
    }
    std::fs::write(dest, output.stdout)?;
    Ok(())
}

pub fn retarget_sound_devices(
    devices: &mut [DeviceEntry],
    selected: Option<usize>,
    amp_sink: &str,
) -> usize {
    let mut changed = 0;
    for (index, device) in devices.iter_mut().enumerate() {
        if device.class() != DeviceClass::Sound {
            continue;
        }
        let current = device.get_str(consts::KEY_DEVID).unwrap_or("");
        let matches_amp = !amp_sink.is_empty() && current == amp_sink;
        let is_selected = selected == Some(index);
        if !matches_amp && !is_selected {
            continue;
        }
        device.set_str(consts::KEY_DEVID, consts::TACTILE_SINK_NAME);
        changed += 1;
    }
    changed
}

pub fn device_channels(devices: &[DeviceEntry], selected: Option<usize>) -> i64 {
    let Some(index) = selected else {
        return consts::DEFAULT_CHANNELS;
    };
    devices
        .get(index)
        .and_then(|device| device.get_i64(consts::KEY_CHANNELS))
        .unwrap_or(consts::DEFAULT_CHANNELS)
}

pub fn restart_pipewire() -> Result<()> {
    let status = Command::new(systemctl_bin())
        .args([
            consts::SYSTEMCTL_USER,
            consts::SYSTEMCTL_RESTART,
            consts::PIPEWIRE_UNIT,
        ])
        .status()
        .context(consts::MSG_PIPEWIRE_RESTART_FAILED)?;
    if status.success() {
        return Ok(());
    }
    Err(anyhow!(consts::MSG_PIPEWIRE_RESTART_FAILED))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;

    fn sound_device(devid: &str) -> DeviceEntry {
        let mut device = DeviceEntry::new();
        schema::apply_defaults(&mut device, DeviceClass::Sound, consts::TYPE_HAPTIC);
        device.set_str(consts::KEY_DEVID, devid);
        device
    }

    #[test]
    fn amplifier_sinks_skip_tactile_and_processors() {
        let discovery = Discovery {
            pulse_sinks: vec![
                HardwareChoice {
                    value: "alsa_output.amp".into(),
                    label: "Amp (alsa_output.amp)".into(),
                },
                HardwareChoice {
                    value: consts::TACTILE_SINK_NAME.into(),
                    label: format!(
                        "Cargopit tactile correction ({}) [{}]",
                        consts::TACTILE_SINK_NAME,
                        consts::LABEL_SINK_PROCESSOR
                    ),
                },
                HardwareChoice {
                    value: "easyeffects_sink".into(),
                    label: format!(
                        "Easy Effects [easyeffects_sink] [{}]",
                        consts::LABEL_SINK_PROCESSOR
                    ),
                },
            ],
            ..Discovery::default()
        };
        let sinks = amplifier_sinks(&discovery);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].value, "alsa_output.amp");
    }

    #[test]
    fn retarget_updates_selected_and_matching_amp() {
        let mut devices = vec![
            sound_device("alsa_output.amp"),
            sound_device("easyeffects_sink"),
            sound_device("alsa_output.amp"),
        ];
        let changed = retarget_sound_devices(&mut devices, Some(1), "alsa_output.amp");
        assert_eq!(changed, 3);
        assert_eq!(
            devices[0].get_str(consts::KEY_DEVID),
            Some(consts::TACTILE_SINK_NAME)
        );
        assert_eq!(
            devices[1].get_str(consts::KEY_DEVID),
            Some(consts::TACTILE_SINK_NAME)
        );
        assert_eq!(
            devices[2].get_str(consts::KEY_DEVID),
            Some(consts::TACTILE_SINK_NAME)
        );
    }

    #[test]
    fn retarget_leaves_unrelated_sound_devices() {
        let mut devices = vec![sound_device("other_sink")];
        let changed = retarget_sound_devices(&mut devices, None, "alsa_output.amp");
        assert_eq!(changed, 0);
        assert_eq!(devices[0].get_str(consts::KEY_DEVID), Some("other_sink"));
    }

    #[test]
    fn default_amp_uses_selected_sound_devid() {
        let devices = vec![sound_device(AMP_SINK)];
        let sink = default_amp_sink(&Discovery::default(), &devices, Some(0));
        assert_eq!(sink, AMP_SINK);
    }

    #[test]
    fn default_amp_skips_empty_selected_sound() {
        let devices = vec![sound_device(""), sound_device(AMP_SINK)];
        let sink = default_amp_sink(&Discovery::default(), &devices, Some(0));
        assert_eq!(sink, AMP_SINK);
    }

    #[test]
    fn write_filter_chain_emits_tactile_sink() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join(consts::SEAT_SWEEP_CSV_NAME);
        std::fs::write(&csv, synthetic_sweep_csv()).unwrap();
        let dest = dir.path().join(consts::TACTILE_CONF_NAME);
        write_filter_chain(&csv, AMP_SINK, consts::DEFAULT_CHANNELS, &dest).unwrap();
        let text = std::fs::read_to_string(&dest).unwrap();
        assert!(text.contains(consts::TACTILE_SINK_NAME), "{text}");
        assert!(text.contains(AMP_SINK), "{text}");
    }

    const AMP_SINK: &str = "alsa_output.amp";
    const SWEEP_START_HZ: f64 = 10.0;
    const SWEEP_END_HZ: f64 = 120.0;
    const SWEEP_POINTS: usize = 24;
    const SWEEP_FLAT_DB: f64 = 3.0;
    const SWEEP_PEAK_HZ: f64 = 45.0;
    const SWEEP_PEAK_DB: f64 = 12.0;
    const SWEEP_PEAK_SIGMA_HZ: f64 = 4.0;
    const SWEEP_GAUSSIAN_EXP_SCALE: f64 = 0.5;

    fn synthetic_sweep_csv() -> String {
        let gaps = (SWEEP_POINTS - 1) as f64;
        let ratio = (SWEEP_END_HZ / SWEEP_START_HZ).powf(1.0 / gaps);
        let mut lines = vec![format!(
            "{},{}",
            consts::CSV_FREQUENCY,
            consts::CSV_TRANSFER
        )];
        for index in 0..SWEEP_POINTS {
            let hz = SWEEP_START_HZ * ratio.powi(index as i32);
            let offset = (hz - SWEEP_PEAK_HZ) / SWEEP_PEAK_SIGMA_HZ;
            let db =
                SWEEP_FLAT_DB + SWEEP_PEAK_DB * (-SWEEP_GAUSSIAN_EXP_SCALE * offset * offset).exp();
            lines.push(format!("{hz:.6},{db:.6}"));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}
