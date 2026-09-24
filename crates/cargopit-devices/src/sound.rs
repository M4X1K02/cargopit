//! Shaker DSP. PCM matches the C capture goldens with noise disabled.

use std::cell::{Cell, RefCell};

use crate::clock::{Clock, VirtualClock};
use crate::haptic::{chassis_is_rolling, HapticEffect, HapticSettings, TyreId, VibrationEffect};
use crate::telemetry::Telemetry;

const ORIGIN_US: u64 = 1_000_000;
const TICK_US: u64 = 16_000;
const CLOCK_MONOTONIC: i32 = 1;
const SINK_NAME: &str = "parity-shaker";
const APP_NAME: &str = "Cargopit";
const STREAM_INDEX: u32 = 1;
const SINK_UNMUTED: i32 = 0;
const CHANNELS: usize = 2;
const SAMPLE_RATE: f64 = 48_000.0;
const PCM_FRAMES: usize = 48;
const BYTES_PER_SAMPLE: usize = 2;
const HAPTIC_HZ: u32 = 40;
const HAPTIC_AMP: u32 = 100;
const HAPTIC_THRESHOLD: f64 = 0.2;
const HAPTIC_DURATION_S: f64 = 0.10;
const AMPLITUDE_UNITY: u32 = 100;
const TONE_MIN_HZ: f64 = 32.0;
const TONE_MAX_HZ: f64 = 120.0;
const OUTPUT_LP_HZ: f64 = TONE_MAX_HZ;
const ENGINE_CYLINDERS: f64 = 4.0;
const ENGINE_STROKE_CYCLES: f64 = 2.0;
const SECONDS_PER_MINUTE: f64 = 60.0;
const ENGINE_IDLE_AMP: f64 = 0.20;
const ENGINE_LOAD_WEIGHT: f64 = 0.75;
const ENGINE_RPM_WEIGHT: f64 = 0.12;
const GEAR_DECAY_K: f64 = 3.0;
const GEAR_DURATION_S: f64 = 0.10;
const GEAR_NEUTRAL: u32 = 1;
const ATTACK_S: f64 = 0.008;
const RELEASE_S: f64 = 0.150;
const FREQ_SMOOTH_S: f64 = 0.018;
const AMP_SMOOTH_S: f64 = 0.025;
const SILENCE_GAIN: f64 = 0.001;
const MAX_DRIVE: f64 = 0.40;
const SLIP_PLAY_REF: f64 = 0.35;
const ABS_PLAY_REF: f64 = 0.25;
const ABS_PULSE_ON_S: f64 = 0.050;
const ABS_PULSE_PERIOD_S: f64 = 0.125;
const ABS_PULSE_HZ: f64 = 1.0 / ABS_PULSE_PERIOD_S;
const ABS_PULSE_DUTY: f64 = ABS_PULSE_ON_S / ABS_PULSE_PERIOD_S;
const ABS_PULSE_DEPTH: f64 = 1.0;
const SUSP_PLAY_REF: f64 = 8.0;
const SUSP_GAMMA: f64 = 0.50;
const HARMONIC2: f64 = 2.0;
const HARMONIC3: f64 = 3.0;
const I16_MAX: f64 = 32_767.0;
const I16_MIN: f64 = -32_768.0;
const CLOCK_OP: &str = concat!("clock_", "gettime");

pub const SOUND_DEVICES: &[&str] = &[
    "sound_engine",
    "sound_gear",
    "sound_slip",
    "sound_lock",
    "sound_abs",
    "sound_suspension",
];

struct Log {
    tick: Cell<u32>,
    lines: RefCell<String>,
    clock: RefCell<VirtualClock>,
}

struct Trace<'a> {
    log: &'a Log,
}

struct Coeff {
    rate: f64,
    attack: f64,
    release: f64,
    freq: f64,
    amp: f64,
}

struct Tone {
    effect: VibrationEffect,
    frequency: u32,
    frequency_max: u32,
    duration: f64,
    haptic: HapticEffect,
    curr_duration: f64,
    phase: f64,
    curr_amplitude: u32,
    curr_frequency: f64,
    play_frequency: f64,
    play_gain: f64,
    play_amplitude: f64,
    harmonic2_gain: f64,
    harmonic3_gain: f64,
    pulse_hz: f64,
    pulse_phase: f64,
    pulse_depth: f64,
    pulse_duty: f64,
    lp1: f64,
    lp2: f64,
    last_gear: u32,
}

impl Log {
    fn new() -> Self {
        let mut clock = VirtualClock::new();
        clock.advance_us(ORIGIN_US);
        Self {
            tick: Cell::new(0),
            lines: RefCell::new(String::new()),
            clock: RefCell::new(clock),
        }
    }

    fn text(&self) -> String {
        self.lines.borrow().clone()
    }

    fn set_tick(&self, tick: u32) {
        self.tick.set(tick);
    }

    fn advance_tick(&self) {
        self.clock.borrow_mut().advance_us(TICK_US);
    }

    fn op(&self, name: &str, detail: &str) {
        let ms = self.clock.borrow().monotonic_ms();
        let line = format!("tick {} t_ms {ms} {name} {detail}\n", self.tick.get());
        self.lines.borrow_mut().push_str(&line);
    }

    fn bytes(&self, data: &[u8]) {
        let mut hex = String::with_capacity(data.len() * 2);
        for byte in data {
            hex.push_str(&format!("{byte:02x}"));
        }
        self.op("pa_stream_write", &hex);
    }

    fn open(&self, stream: &str, sink: &str) {
        self.op("pa_threaded_mainloop_new", "");
        self.op("pa_context_new", APP_NAME);
        self.op("pa_threaded_mainloop_start", "");
        self.op("pa_context_connect", "");
        self.op("pa_stream_new", stream);
        self.op("pa_stream_connect_playback", sink);
        self.op(
            "pa_context_set_sink_input_mute",
            &format!("index={STREAM_INDEX} mute={SINK_UNMUTED}"),
        );
    }

    fn close(&self) {
        self.op("pa_stream_disconnect", "");
        self.op("pa_context_unref", "");
        self.op("pa_threaded_mainloop_free", "");
    }
}

impl Clock for Trace<'_> {
    fn monotonic_ms(&self) -> u64 {
        self.log.clock.borrow().monotonic_ms()
    }

    fn monotonic_us(&self) -> u64 {
        self.log.clock.borrow().monotonic_us()
    }

    fn monotonic_ns(&self) -> u64 {
        let ns = self.log.clock.borrow().monotonic_ns();
        self.log
            .op(CLOCK_OP, &format!("clk={CLOCK_MONOTONIC} ns={ns}"));
        ns
    }

    fn wall_ms(&self) -> u64 {
        self.log.clock.borrow().wall_ms()
    }
}

pub fn capture_sound(name: &str, frames: &[Telemetry]) -> Option<String> {
    let effect = effect_for(name)?;
    let log = Log::new();
    log.open(stream_name(effect), SINK_NAME);
    let mut tone = Tone::new(effect);
    let clock = Trace { log: &log };
    for (index, frame) in frames.iter().enumerate() {
        log.set_tick(index as u32);
        tone.update(frame, &clock);
        log.bytes(&tone.render());
        log.advance_tick();
    }
    log.close();
    Some(log.text())
}

fn effect_for(name: &str) -> Option<VibrationEffect> {
    match name {
        "sound_engine" => Some(VibrationEffect::EngineRpm),
        "sound_gear" => Some(VibrationEffect::GearShift),
        "sound_slip" => Some(VibrationEffect::TyreSlip),
        "sound_lock" => Some(VibrationEffect::TyreLock),
        "sound_abs" => Some(VibrationEffect::AbsBrakes),
        "sound_suspension" => Some(VibrationEffect::Suspension),
        _ => None,
    }
}

fn stream_name(effect: VibrationEffect) -> &'static str {
    match effect {
        VibrationEffect::EngineRpm => "Engine",
        VibrationEffect::GearShift => "Gear",
        VibrationEffect::AbsBrakes => "ABS",
        VibrationEffect::TyreSlip => "TyreSlip",
        VibrationEffect::TyreLock => "TyreLock",
        VibrationEffect::Suspension => "Suspension",
    }
}

fn capture_settings(effect: VibrationEffect) -> HapticSettings {
    HapticSettings {
        effect,
        tyre: TyreId::AllFour,
        frequency: HAPTIC_HZ,
        amplitude: HAPTIC_AMP,
        threshold: HAPTIC_THRESHOLD,
        duration: HAPTIC_DURATION_S,
        ..HapticSettings::default()
    }
}

impl Tone {
    fn new(effect: VibrationEffect) -> Self {
        let settings = capture_settings(effect);
        let duration = if effect == VibrationEffect::GearShift {
            if settings.duration > 0.0 {
                settings.duration
            } else {
                GEAR_DURATION_S
            }
        } else {
            0.0
        };
        Self {
            effect,
            frequency: settings.frequency,
            frequency_max: settings.frequency_max,
            duration,
            haptic: HapticEffect::new(&settings),
            curr_duration: 0.0,
            phase: 0.0,
            curr_amplitude: 0,
            curr_frequency: 0.0,
            play_frequency: 0.0,
            play_gain: 0.0,
            play_amplitude: 0.0,
            harmonic2_gain: 0.0,
            harmonic3_gain: 0.0,
            pulse_hz: 0.0,
            pulse_phase: 0.0,
            pulse_depth: 0.0,
            pulse_duty: 0.0,
            lp1: 0.0,
            lp2: 0.0,
            last_gear: GEAR_NEUTRAL,
        }
    }

    fn update(&mut self, frame: &Telemetry, clock: &impl Clock) {
        match self.effect {
            VibrationEffect::EngineRpm => self.update_engine(frame),
            VibrationEffect::GearShift => self.update_gear(frame),
            VibrationEffect::TyreSlip | VibrationEffect::TyreLock => {
                self.update_continuous(frame, clock, SLIP_PLAY_REF);
            }
            VibrationEffect::AbsBrakes => self.update_abs(frame, clock),
            VibrationEffect::Suspension => self.update_suspension(frame, clock),
        }
    }

    fn update_engine(&mut self, frame: &Telemetry) {
        let rpm = engine_play_rpm(frame);
        if rpm == 0 || frame.maxrpm() == 0 {
            self.silence_engine();
            return;
        }
        let amp = engine_amp_frac(frame.gas(), rpm, frame.maxrpm());
        self.duration = 0.0;
        self.curr_frequency = engine_tone_hz(
            rpm,
            frame.idlerpm(),
            frame.maxrpm(),
            f64::from(self.frequency),
            f64::from(self.frequency_max),
        );
        self.curr_amplitude = amplitude_from_level(amp);
        self.harmonic2_gain = 0.0;
        self.harmonic3_gain = 0.0;
        self.pulse_depth = 0.0;
        self.pulse_hz = 0.0;
    }

    fn silence_engine(&mut self) {
        self.curr_frequency = 0.0;
        self.curr_amplitude = 0;
        self.pulse_hz = 0.0;
    }

    fn update_gear(&mut self, frame: &Telemetry) {
        if self.last_gear == frame.gear() {
            return;
        }
        self.last_gear = frame.gear();
        self.curr_frequency = f64::from(self.frequency);
        self.curr_amplitude = AMPLITUDE_UNITY;
        self.curr_duration = 0.0;
        self.phase = 0.0;
        self.play_frequency = self.curr_frequency;
        self.play_amplitude = f64::from(self.curr_amplitude);
        self.play_gain = 1.0;
    }

    fn update_continuous(&mut self, frame: &Telemetry, clock: &impl Clock, play_ref: f64) {
        let play = self.haptic.play_with_clock(frame, clock);
        if play <= 0.0 || play_ref <= 0.0 {
            self.curr_frequency = 0.0;
            self.curr_amplitude = 0;
            self.curr_duration = 0.0;
            return;
        }
        let level = clamp_unit(play / play_ref);
        self.duration = 0.0;
        self.curr_frequency = f64::from(self.frequency);
        self.curr_amplitude = amplitude_from_level(level);
    }

    fn update_abs(&mut self, frame: &Telemetry, clock: &impl Clock) {
        let play = self.haptic.play_with_clock(frame, clock);
        if play <= 0.0 {
            self.curr_frequency = 0.0;
            self.curr_amplitude = 0;
            self.curr_duration = 0.0;
            self.pulse_hz = 0.0;
            self.pulse_depth = 0.0;
            self.pulse_duty = 0.0;
            return;
        }
        let level = clamp_unit(play / ABS_PLAY_REF);
        self.duration = 0.0;
        self.curr_frequency = f64::from(self.frequency);
        self.curr_amplitude = amplitude_from_level(level);
        self.pulse_hz = ABS_PULSE_HZ;
        self.pulse_depth = ABS_PULSE_DEPTH;
        self.pulse_duty = ABS_PULSE_DUTY;
    }

    fn update_suspension(&mut self, frame: &Telemetry, clock: &impl Clock) {
        if !chassis_is_rolling(frame) {
            self.curr_frequency = 0.0;
            self.curr_amplitude = 0;
            self.curr_duration = 0.0;
            self.duration = 0.0;
            self.play_gain = 0.0;
            self.play_frequency = 0.0;
            self.play_amplitude = 0.0;
            return;
        }
        let effect = self.haptic.play_with_clock(frame, clock);
        if effect <= 0.0 {
            self.curr_frequency = 0.0;
            self.curr_amplitude = 0;
            return;
        }
        let mut level = clamp_unit(effect / SUSP_PLAY_REF);
        level = level.powf(SUSP_GAMMA);
        let fmin = self.frequency;
        let mut fmax = self.frequency_max;
        if fmax < fmin {
            fmax = fmin;
        }
        self.duration = 0.0;
        self.curr_frequency = f64::from(fmin) + (f64::from(fmax) - f64::from(fmin)) * level;
        self.curr_amplitude = amplitude_from_level(level);
    }

    fn render(&mut self) -> Vec<u8> {
        let bytes_per_frame = BYTES_PER_SAMPLE * CHANNELS;
        let length = PCM_FRAMES * bytes_per_frame;
        let mut samples = vec![0i16; length / BYTES_PER_SAMPLE];
        let coeff = coeffs();
        let frames = length / bytes_per_frame;
        let gear = self.effect == VibrationEffect::GearShift;
        for index in 0..frames {
            let fade = self.amplitude_scale(gear);
            let sample = self.sine_frame(fade, &coeff);
            for channel in 0..CHANNELS {
                samples[index * CHANNELS + channel] = sample;
            }
            self.advance_gear(&coeff);
        }
        let mut bytes = Vec::with_capacity(length);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    fn amplitude_scale(&self, gear: bool) -> f64 {
        if gear || self.duration > 0.0 {
            if self.curr_frequency <= 0.0 {
                return 0.0;
            }
            return gear_envelope(self.curr_duration, self.duration);
        }
        1.0
    }

    fn advance_gear(&mut self, coeff: &Coeff) {
        if self.duration <= 0.0 || self.curr_frequency <= 0.0 {
            return;
        }
        self.curr_duration += 1.0 / coeff.rate;
        if self.curr_duration >= self.duration {
            self.curr_duration = 0.0;
            self.curr_frequency = 0.0;
            self.curr_amplitude = 0;
        }
    }

    fn sine_frame(&mut self, amplitude_scale: f64, coeff: &Coeff) -> i16 {
        let audible = self.curr_frequency > 0.0 && self.curr_amplitude > 0 && amplitude_scale > 0.0;
        let target = if audible { 1.0 } else { 0.0 };
        let gain_coeff = if audible { coeff.attack } else { coeff.release };
        self.play_gain += (target - self.play_gain) * gain_coeff;
        self.play_gain = clamp_unit(self.play_gain);
        self.smooth(audible, coeff);
        if self.play_gain <= SILENCE_GAIN {
            self.reset_lowpass();
            if !audible {
                self.play_frequency = 0.0;
                self.play_amplitude = 0.0;
            }
            return 0;
        }
        self.play_frequency = clamp_hz(self.play_frequency);
        if self.play_frequency <= 0.0 {
            self.reset_lowpass();
            return 0;
        }
        let mut drive =
            (self.play_amplitude / f64::from(AMPLITUDE_UNITY)) * amplitude_scale * self.play_gain;
        drive = drive.clamp(0.0, MAX_DRIVE);
        let mut sample = drive * self.harmonic_wave() * self.firing_envelope();
        sample = self.lowpass(sample, coeff.rate);
        let stepped = clamp_hz(self.play_frequency);
        advance_cycle(&mut self.phase, stepped, coeff.rate);
        advance_cycle(&mut self.pulse_phase, self.pulse_hz, coeff.rate);
        clamp_i16(sample * I16_MAX)
    }

    fn smooth(&mut self, audible: bool, coeff: &Coeff) {
        if !audible {
            return;
        }
        if self.play_frequency <= 0.0 {
            self.play_frequency = self.curr_frequency;
        } else {
            self.play_frequency += (self.curr_frequency - self.play_frequency) * coeff.freq;
        }
        let target = f64::from(self.curr_amplitude);
        if self.play_amplitude <= 0.0 {
            self.play_amplitude = target;
        } else {
            self.play_amplitude += (target - self.play_amplitude) * coeff.amp;
        }
    }

    fn harmonic_wave(&self) -> f64 {
        let h2 = harmonic_gain(self.play_frequency, HARMONIC2, self.harmonic2_gain);
        let h3 = harmonic_gain(self.play_frequency, HARMONIC3, self.harmonic3_gain);
        let fund = (std::f64::consts::TAU * self.phase).sin();
        let second = h2 * (std::f64::consts::TAU * HARMONIC2 * self.phase).sin();
        let third = h3 * (std::f64::consts::TAU * HARMONIC3 * self.phase).sin();
        let norm = 1.0 + h2 + h3;
        if norm <= 0.0 {
            return 0.0;
        }
        (fund + second + third) / norm
    }

    fn firing_envelope(&self) -> f64 {
        if self.pulse_hz <= 0.0 || self.pulse_depth <= 0.0 {
            return 1.0;
        }
        let trough = (1.0 - self.pulse_depth).max(0.0);
        if self.pulse_duty <= 0.0 {
            let lift = 0.5 * ((std::f64::consts::TAU * self.pulse_phase).sin() + 1.0);
            return trough + (1.0 - trough) * lift;
        }
        let phase = self.pulse_phase - self.pulse_phase.floor();
        if phase < self.pulse_duty {
            return 1.0;
        }
        trough
    }

    fn lowpass(&mut self, sample: f64, rate: f64) -> f64 {
        let alpha = lowpass_alpha(OUTPUT_LP_HZ, rate);
        let first = lowpass_pole(sample, &mut self.lp1, alpha);
        lowpass_pole(first, &mut self.lp2, alpha)
    }

    fn reset_lowpass(&mut self) {
        self.lp1 = 0.0;
        self.lp2 = 0.0;
    }
}

fn coeffs() -> Coeff {
    Coeff {
        rate: SAMPLE_RATE,
        attack: 1.0 - (-1.0 / (SAMPLE_RATE * ATTACK_S)).exp(),
        release: 1.0 - (-1.0 / (SAMPLE_RATE * RELEASE_S)).exp(),
        freq: 1.0 - (-1.0 / (SAMPLE_RATE * FREQ_SMOOTH_S)).exp(),
        amp: 1.0 - (-1.0 / (SAMPLE_RATE * AMP_SMOOTH_S)).exp(),
    }
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn clamp_hz(hz: f64) -> f64 {
    if hz <= 0.0 {
        return 0.0;
    }
    hz.min(TONE_MAX_HZ)
}

fn amplitude_from_level(level: f64) -> u32 {
    let scaled = f64::from(AMPLITUDE_UNITY) * clamp_unit(level);
    lrint_u32(scaled)
}

fn lrint_u32(value: f64) -> u32 {
    if value < 0.0 {
        return 0;
    }
    value.round_ties_even() as u32
}

fn engine_amp_frac(throttle: f64, rpm: u32, maxrpm: u32) -> f64 {
    let load = clamp_unit(throttle);
    let rpm_term = if maxrpm == 0 {
        0.0
    } else {
        clamp_unit(f64::from(rpm) / f64::from(maxrpm))
    };
    clamp_unit(ENGINE_IDLE_AMP + ENGINE_LOAD_WEIGHT * load + ENGINE_RPM_WEIGHT * rpm_term)
}

fn engine_firing_hz(rpm: u32) -> f64 {
    if rpm == 0 {
        return 0.0;
    }
    f64::from(rpm) * ENGINE_CYLINDERS / (SECONDS_PER_MINUTE * ENGINE_STROKE_CYCLES)
}

fn engine_rpm_frac(rpm: u32, idle: u32, maxrpm: u32) -> f64 {
    if rpm == 0 || maxrpm == 0 {
        return 0.0;
    }
    let idle = f64::from(idle);
    let maxr = f64::from(maxrpm);
    let rpm = f64::from(rpm);
    if idle <= 0.0 || idle >= maxr {
        return clamp_unit(rpm / maxr);
    }
    clamp_unit((rpm - idle) / (maxr - idle))
}

fn engine_band_min(configured: f64) -> f64 {
    if configured < TONE_MIN_HZ || configured >= TONE_MAX_HZ {
        return TONE_MIN_HZ;
    }
    configured
}

fn engine_band_max(configured: f64, band_min: f64) -> f64 {
    if configured <= band_min || configured > TONE_MAX_HZ {
        return TONE_MAX_HZ;
    }
    configured
}

fn engine_idle_hz(idle: u32, band_min: f64, band_max: f64) -> f64 {
    let hz = engine_firing_hz(idle);
    if hz < band_min || hz >= band_max {
        return band_min;
    }
    hz
}

fn engine_tone_hz(rpm: u32, idle: u32, maxrpm: u32, min_hz: f64, max_hz: f64) -> f64 {
    let band_min = engine_band_min(min_hz);
    let band_max = engine_band_max(max_hz, band_min);
    let idle_hz = engine_idle_hz(idle, band_min, band_max);
    let frac = engine_rpm_frac(rpm, idle, maxrpm);
    idle_hz + (band_max - idle_hz) * frac
}

fn engine_play_rpm(frame: &Telemetry) -> u32 {
    if frame.maxrpm() == 0 {
        return 0;
    }
    if frame.rpms() > 0 {
        if frame.idlerpm() > 0 && frame.rpms() < frame.idlerpm() {
            return frame.idlerpm();
        }
        return frame.rpms();
    }
    if frame.gear() == GEAR_NEUTRAL && frame.idlerpm() > 0 {
        return frame.idlerpm();
    }
    0
}

fn gear_envelope(elapsed: f64, duration: f64) -> f64 {
    if duration <= 0.0 {
        return 0.0;
    }
    let t = (elapsed / duration).clamp(0.0, 1.0);
    (-GEAR_DECAY_K * t).exp()
}

fn harmonic_gain(fundamental: f64, order: f64, requested: f64) -> f64 {
    if fundamental <= 0.0 || requested <= 0.0 || order <= 0.0 {
        return 0.0;
    }
    if fundamental * order > TONE_MAX_HZ {
        return 0.0;
    }
    requested
}

fn lowpass_alpha(cutoff: f64, rate: f64) -> f64 {
    if cutoff <= 0.0 || rate <= cutoff {
        return 1.0;
    }
    (1.0 - (-std::f64::consts::TAU * cutoff / rate).exp()).clamp(0.0, 1.0)
}

fn lowpass_pole(sample: f64, state: &mut f64, alpha: f64) -> f64 {
    *state += alpha * (sample - *state);
    *state
}

fn advance_cycle(phase: &mut f64, hz: f64, rate: f64) {
    if hz <= 0.0 || rate <= 0.0 {
        return;
    }
    *phase = wrap_cycle(*phase + hz / rate);
}

fn wrap_cycle(mut phase: f64) -> f64 {
    if !(0.0..1.0).contains(&phase) {
        phase -= phase.floor();
    }
    phase
}

fn clamp_i16(sample: f64) -> i16 {
    if sample > I16_MAX {
        return i16::MAX;
    }
    if sample < I16_MIN {
        return i16::MIN;
    }
    sample.round_ties_even() as i16
}
