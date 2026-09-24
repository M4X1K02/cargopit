//! Haptic intensity math from `hapticeffect.c`. USB and audio encoders stay in later phases.

use crate::clock::Clock;
use crate::telemetry::{Telemetry, PROXIMITY_CARS};
use simapi_sys::WHEEL_COUNT;

const KM_H_TO_M_S: f64 = 0.277778;
const MIN_SPEED_M_S: f64 = 0.5;
const MIN_VELOCITY: u32 = 50;
const MAX_BRAKE: f64 = 0.0;
const MAX_THROTTLE: f64 = 0.0;
const MAX_X_VELOCITY: f64 = 0.001;
const MIN_Y_VELOCITY: f64 = 0.0;
const MAX_Z_VELOCITY: f64 = 1.0;
const BRAKE_APPLIED_FRAC: f64 = 0.05;
const THROTTLE_APPLIED_FRAC: f64 = 0.08;
const ABS_PUMP_ALPHA: f64 = 0.35;
const ABS_MIN_PUMP: f64 = 0.04;
const SLIP_WHEELSPIN: i32 = 0;
const SLIP_LOCKUP: i32 = 1;
const SUSP_MIN_SPEED_KMH: u32 = 1;
const SUSP_FROZEN_TICKS: f64 = 12.0;
const SUSP_VEL_EMA_ALPHA: f64 = 0.25;
const SUSP_ENV_ALPHA: f64 = 0.06;
const SUSP_IMPACT_RATIO: f64 = 2.8;
const FILTER_REFERENCE_HZ: f64 = 60.0;
const FILTER_MIN_DT_S: f64 = 0.0001;
const FILTER_MAX_DT_S: f64 = 0.25;
const SUSP_FROZEN_SECONDS: f64 = SUSP_FROZEN_TICKS / FILTER_REFERENCE_HZ;
const NS_PER_S: f64 = 1_000_000_000.0;

/// `SIMULATORAPI_DIRT_RALLY_2` in the simapi submodule.
pub const SIMULATOR_DIRT_RALLY_2: u8 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum VibrationEffect {
    EngineRpm = 0,
    GearShift = 1,
    AbsBrakes = 2,
    TyreSlip = 3,
    TyreLock = 4,
    Suspension = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TyreId {
    FrontLeft = 0,
    FrontRight = 1,
    RearLeft = 2,
    RearRight = 3,
    Fronts = 4,
    Rears = 5,
    AllFour = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Modulation {
    None = 0,
    Frequency = 1,
    Amplify = 2,
}

#[derive(Clone, Debug)]
pub struct HapticSettings {
    pub effect: VibrationEffect,
    pub tyre: TyreId,
    pub modulation: Modulation,
    pub threshold: f64,
    pub frequency: u32,
    pub frequency_max: u32,
    pub amplitude: u32,
    pub amplitude_max: u32,
    pub motor_position: u32,
    pub duration: f64,
}

impl Default for HapticSettings {
    fn default() -> Self {
        Self {
            effect: VibrationEffect::TyreSlip,
            tyre: TyreId::AllFour,
            modulation: Modulation::None,
            threshold: 0.0,
            frequency: 0,
            frequency_max: 0,
            amplitude: 0,
            amplitude_max: 0,
            motor_position: 0,
            duration: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
struct FilterState {
    last_update_ns: u64,
    abs_last_slip: [f64; WHEEL_COUNT],
    abs_pump_ema: f64,
    abs_primed: bool,
    susp_last_velocity: [f64; WHEEL_COUNT],
    susp_frozen_seconds: f64,
    susp_baseline: [f64; WHEEL_COUNT],
    susp_baseline_ready: [bool; WHEEL_COUNT],
    susp_motion_floor: f64,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            last_update_ns: 0,
            abs_last_slip: [0.0; WHEEL_COUNT],
            abs_pump_ema: 0.0,
            abs_primed: false,
            susp_last_velocity: [0.0; WHEEL_COUNT],
            susp_frozen_seconds: 0.0,
            susp_baseline: [0.0; WHEEL_COUNT],
            susp_baseline_ready: [false; WHEEL_COUNT],
            susp_motion_floor: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HapticEffect {
    effect: VibrationEffect,
    tyre: TyreId,
    modulation: Modulation,
    threshold: f64,
    frequency: u32,
    frequency_max: u32,
    amplitude: u32,
    amplitude_max: u32,
    motor_position: u32,
    duration: f64,
    filter: FilterState,
}

impl HapticEffect {
    pub fn new(settings: &HapticSettings) -> Self {
        Self {
            effect: settings.effect,
            tyre: settings.tyre,
            modulation: settings.modulation,
            threshold: settings.threshold,
            frequency: settings.frequency,
            frequency_max: settings.frequency_max,
            amplitude: settings.amplitude,
            amplitude_max: settings.amplitude_max,
            motor_position: settings.motor_position,
            duration: settings.duration,
            filter: FilterState::default(),
        }
    }

    pub fn effect(&self) -> VibrationEffect {
        self.effect
    }

    pub fn tyre(&self) -> TyreId {
        self.tyre
    }

    pub fn modulation(&self) -> Modulation {
        self.modulation
    }

    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    pub fn frequency(&self) -> u32 {
        self.frequency
    }

    pub fn frequency_max(&self) -> u32 {
        self.frequency_max
    }

    pub fn amplitude(&self) -> u32 {
        self.amplitude
    }

    pub fn amplitude_max(&self) -> u32 {
        self.amplitude_max
    }

    pub fn motor_position(&self) -> u32 {
        self.motor_position
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn play(&mut self, sim: &Telemetry, dt_seconds: f64) -> f64 {
        let dt = clamp_dt(dt_seconds);
        let slip = wheel_slip(sim, self.effect);
        if self.effect != VibrationEffect::Suspension && !car_is_moving(sim) {
            return 0.0;
        }
        match self.effect {
            VibrationEffect::TyreSlip => slip_play(sim, &slip, self.tyre, self.threshold),
            VibrationEffect::TyreLock => lock_play(sim, &slip, self.tyre, self.threshold),
            VibrationEffect::AbsBrakes => {
                self.filter
                    .abs_play(sim, &slip, self.tyre, self.threshold, dt)
            }
            VibrationEffect::Suspension => {
                self.filter
                    .suspension_play(sim, self.tyre, self.threshold, dt)
            }
            VibrationEffect::EngineRpm | VibrationEffect::GearShift => 0.0,
        }
    }

    pub fn play_with_clock(&mut self, sim: &Telemetry, clock: &impl Clock) -> f64 {
        let dt = self.filter.seconds_since_last(clock);
        self.play(sim, dt)
    }
}

pub fn has_tyre_diameter(sim: &Telemetry) -> bool {
    (0..WHEEL_COUNT).all(|index| sim.tyre_diameter(index) > 0.0)
}

pub fn measure_tyre_diameter(sim: &mut Telemetry) -> bool {
    if sim.velocity() <= MIN_VELOCITY || sim.brake() > MAX_BRAKE || sim.gas() > MAX_THROTTLE {
        return false;
    }
    let speed_ms = KM_H_TO_M_S * f64::from(sim.velocity());
    if speed_ms == 0.0 || sim.x_velocity() / speed_ms >= MAX_X_VELOCITY {
        return false;
    }
    for index in 0..WHEEL_COUNT {
        let diameter = speed_ms / sim.tyre_rps(index) * 2.0;
        sim.set_tyre_diameter(index, diameter);
    }
    true
}

pub fn chassis_is_rolling(sim: &Telemetry) -> bool {
    sim.velocity() >= SUSP_MIN_SPEED_KMH
}

fn slip_play(sim: &Telemetry, slip: &[f64; WHEEL_COUNT], tyre: TyreId, threshold: f64) -> f64 {
    if sim.gas() <= THROTTLE_APPLIED_FRAC {
        return 0.0;
    }
    sum_slip_beyond(slip, tyre, threshold, SLIP_WHEELSPIN)
}

fn lock_play(sim: &Telemetry, slip: &[f64; WHEEL_COUNT], tyre: TyreId, threshold: f64) -> f64 {
    if sim.brake() <= BRAKE_APPLIED_FRAC {
        return 0.0;
    }
    sum_slip_beyond(slip, tyre, threshold, SLIP_LOCKUP)
}

fn tyre_selected(selected: TyreId, wheel: usize) -> bool {
    if selected == TyreId::AllFour {
        return true;
    }
    if selected as usize == wheel {
        return true;
    }
    if selected == TyreId::Fronts
        && (wheel == TyreId::FrontLeft as usize || wheel == TyreId::FrontRight as usize)
    {
        return true;
    }
    selected == TyreId::Rears
        && (wheel == TyreId::RearLeft as usize || wheel == TyreId::RearRight as usize)
}

fn brake_applied(sim: &Telemetry) -> bool {
    sim.brake() > BRAKE_APPLIED_FRAC
}

fn car_is_moving(sim: &Telemetry) -> bool {
    if sim.y_velocity() <= MIN_Y_VELOCITY {
        return false;
    }
    sim.z_velocity().abs() <= MAX_Z_VELOCITY
}

fn sim_provides_slip(sim: &Telemetry) -> bool {
    if sim.simapi() == SIMULATOR_DIRT_RALLY_2 {
        return true;
    }
    (0..WHEEL_COUNT).any(|index| sim.tyre_slip(index).abs() != 0.0)
}

fn sum_slip_beyond(slip: &[f64; WHEEL_COUNT], tyre: TyreId, threshold: f64, lockup: i32) -> f64 {
    let mut play = 0.0;
    for (index, value) in slip.iter().enumerate() {
        if !tyre_selected(tyre, index) {
            continue;
        }
        play += slip_term(*value, threshold, lockup);
    }
    play
}

fn slip_term(value: f64, threshold: f64, lockup: i32) -> f64 {
    if lockup != 0 {
        if value > threshold {
            return value - threshold;
        }
        return 0.0;
    }
    if value < -threshold {
        return value.abs() - threshold.abs();
    }
    0.0
}

fn reference_ticks(dt_seconds: f64) -> f64 {
    dt_seconds * FILTER_REFERENCE_HZ
}

fn scaled_alpha(alpha: f64, dt_seconds: f64) -> f64 {
    1.0 - (1.0 - alpha).powf(reference_ticks(dt_seconds))
}

fn clamp_dt(dt_seconds: f64) -> f64 {
    if dt_seconds < FILTER_MIN_DT_S {
        return FILTER_MIN_DT_S;
    }
    if dt_seconds > FILTER_MAX_DT_S {
        return FILTER_MAX_DT_S;
    }
    dt_seconds
}

fn effect_uses_slip(effect: VibrationEffect) -> bool {
    matches!(
        effect,
        VibrationEffect::TyreSlip | VibrationEffect::TyreLock | VibrationEffect::AbsBrakes
    )
}

fn wheel_slip(sim: &Telemetry, effect: VibrationEffect) -> [f64; WHEEL_COUNT] {
    let mut slip = [0.0; WHEEL_COUNT];
    if sim_provides_slip(sim) {
        for (index, slot) in slip.iter_mut().enumerate() {
            *slot = sim.tyre_slip(index);
        }
        return slip;
    }
    if effect_uses_slip(effect) {
        calculate_wheel_slip(sim, &mut slip);
    }
    slip
}

fn calculate_wheel_slip(sim: &Telemetry, slip: &mut [f64; WHEEL_COUNT]) {
    let speed_ms = KM_H_TO_M_S * f64::from(sim.velocity());
    if !has_tyre_diameter(sim) || speed_ms <= MIN_SPEED_M_S {
        return;
    }
    for (index, slot) in slip.iter_mut().enumerate() {
        *slot = (speed_ms - sim.tyre_diameter(index) * sim.tyre_rps(index) / 2.0) / speed_ms;
    }
}

impl FilterState {
    fn abs_reset(&mut self) {
        self.abs_last_slip = [0.0; WHEEL_COUNT];
        self.abs_pump_ema = 0.0;
        self.abs_primed = false;
    }

    fn abs_play(
        &mut self,
        sim: &Telemetry,
        slip: &[f64; WHEEL_COUNT],
        tyre: TyreId,
        threshold: f64,
        dt_seconds: f64,
    ) -> f64 {
        if !brake_applied(sim) {
            self.abs_reset();
            return 0.0;
        }
        let mut max_lock: f64 = 0.0;
        let mut max_ds: f64 = 0.0;
        for (index, value) in slip.iter().enumerate() {
            if !tyre_selected(tyre, index) {
                continue;
            }
            let lock = lock_slip_only(*value);
            max_lock = max_lock.max(lock);
            if self.abs_primed {
                max_ds = max_ds.max((lock - self.abs_last_slip[index]).abs());
            }
            self.abs_last_slip[index] = lock;
        }
        self.abs_primed = true;
        max_ds /= reference_ticks(dt_seconds);
        let alpha = scaled_alpha(ABS_PUMP_ALPHA, dt_seconds);
        self.abs_pump_ema += alpha * (max_ds - self.abs_pump_ema);
        if max_lock <= threshold || self.abs_pump_ema <= ABS_MIN_PUMP {
            return 0.0;
        }
        max_lock - threshold
    }

    fn suspension_frozen(&mut self, sim: &Telemetry, dt_seconds: f64) -> bool {
        let mut same = true;
        for index in 0..WHEEL_COUNT {
            if sim.susp_velocity(index) != self.susp_last_velocity[index] {
                same = false;
            }
            self.susp_last_velocity[index] = sim.susp_velocity(index);
        }
        if !same {
            self.susp_frozen_seconds = 0.0;
            return false;
        }
        if self.susp_frozen_seconds < SUSP_FROZEN_SECONDS {
            self.susp_frozen_seconds += dt_seconds;
        }
        self.susp_frozen_seconds >= SUSP_FROZEN_SECONDS
    }

    fn suspension_play(
        &mut self,
        sim: &Telemetry,
        tyre: TyreId,
        threshold: f64,
        dt_seconds: f64,
    ) -> f64 {
        let rolling = chassis_is_rolling(sim);
        let frozen = self.suspension_frozen(sim, dt_seconds);
        let baseline_alpha = scaled_alpha(SUSP_VEL_EMA_ALPHA, dt_seconds);
        let mut max_motion: f64 = 0.0;
        for index in 0..WHEEL_COUNT {
            let velocity = sim.susp_velocity(index);
            if !self.susp_baseline_ready[index] {
                self.susp_baseline[index] = velocity;
                self.susp_baseline_ready[index] = true;
                continue;
            }
            self.susp_baseline[index] += baseline_alpha * (velocity - self.susp_baseline[index]);
            if !tyre_selected(tyre, index) {
                continue;
            }
            max_motion = max_motion.max((velocity - self.susp_baseline[index]).abs());
        }
        if !rolling || frozen {
            self.susp_motion_floor = max_motion;
            return 0.0;
        }
        let env_alpha = scaled_alpha(SUSP_ENV_ALPHA, dt_seconds);
        self.susp_motion_floor += env_alpha * (max_motion - self.susp_motion_floor);
        let mut gate = threshold;
        let raised = self.susp_motion_floor * SUSP_IMPACT_RATIO;
        if raised > gate {
            gate = raised;
        }
        if max_motion <= gate {
            return 0.0;
        }
        max_motion - gate
    }

    fn seconds_since_last(&mut self, clock: &impl Clock) -> f64 {
        let now_ns = clock.monotonic_ns();
        let last_ns = self.last_update_ns;
        self.last_update_ns = now_ns;
        if last_ns == 0 {
            return 1.0 / FILTER_REFERENCE_HZ;
        }
        (now_ns.saturating_sub(last_ns)) as f64 / NS_PER_S
    }
}

pub fn proximity_car_count() -> usize {
    PROXIMITY_CARS
}

fn lock_slip_only(slip: f64) -> f64 {
    if slip <= 0.0 {
        return 0.0;
    }
    slip
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::VirtualClock;

    const DT: f64 = 1.0 / FILTER_REFERENCE_HZ;
    const THRESHOLD: f64 = 0.1;

    fn moving() -> Telemetry {
        let mut sim = Telemetry::new();
        sim.set_y_velocity(1.0);
        sim.set_velocity(80);
        sim
    }

    fn slip_effect(effect: VibrationEffect, tyre: TyreId) -> HapticEffect {
        HapticEffect::new(&HapticSettings {
            effect,
            tyre,
            threshold: THRESHOLD,
            ..HapticSettings::default()
        })
    }

    #[test]
    fn slip_sums_selected_wheels_when_throttle_is_applied() {
        let mut sim = moving();
        sim.set_gas(0.2);
        sim.set_tyre_slip(0, -0.4);
        sim.set_tyre_slip(1, -0.2);
        sim.set_tyre_slip(2, -0.9);
        let mut fronts = slip_effect(VibrationEffect::TyreSlip, TyreId::Fronts);
        let play = fronts.play(&sim, DT);
        assert!((play - 0.4).abs() < 1e-9);
        sim.set_gas(0.01);
        assert_eq!(fronts.play(&sim, DT), 0.0);
    }

    #[test]
    fn lock_sums_wheels_above_threshold_when_braking() {
        let mut sim = moving();
        sim.set_brake(0.2);
        sim.set_tyre_slip(3, 0.4);
        let mut effect = slip_effect(VibrationEffect::TyreLock, TyreId::AllFour);
        assert!((effect.play(&sim, DT) - 0.3).abs() < 1e-9);
        sim.set_y_velocity(0.0);
        assert_eq!(effect.play(&sim, DT), 0.0);
    }

    #[test]
    fn abs_needs_a_lock_slip_pump() {
        let mut sim = moving();
        sim.set_brake(0.2);
        sim.set_tyre_slip(0, 0.5);
        let mut effect = slip_effect(VibrationEffect::AbsBrakes, TyreId::AllFour);
        assert_eq!(effect.play(&sim, DT), 0.0);
        sim.set_tyre_slip(0, 0.9);
        let play = effect.play(&sim, DT);
        assert!((play - 0.8).abs() < 1e-9);
    }

    #[test]
    fn suspension_spike_then_frozen_repeat_is_silent() {
        let mut sim = moving();
        sim.set_susp_velocity(0, 0.2);
        let mut effect = slip_effect(VibrationEffect::Suspension, TyreId::AllFour);
        assert_eq!(effect.play(&sim, DT), 0.0);
        sim.set_susp_velocity(0, 5.0);
        assert!(effect.play(&sim, DT) > 0.0);
        for _ in 0..20 {
            let _ = effect.play(&sim, DT);
        }
        assert!(effect.filter.susp_frozen_seconds >= SUSP_FROZEN_SECONDS);
        assert_eq!(effect.play(&sim, DT), 0.0);
    }

    #[test]
    fn tyre_diameter_uses_straight_line_coasting() {
        let mut sim = Telemetry::new();
        sim.set_velocity(100);
        for index in 0..WHEEL_COUNT {
            sim.set_tyre_rps(index, 10.0);
        }
        assert!(measure_tyre_diameter(&mut sim));
        let expected = KM_H_TO_M_S * 100.0 / 10.0 * 2.0;
        assert!((sim.tyre_diameter(0) - expected).abs() < 1e-9);
        sim.set_gas(0.2);
        assert!(!measure_tyre_diameter(&mut sim));
    }

    #[test]
    fn clock_play_uses_reference_dt_on_the_first_sample() {
        let sim = moving();
        let mut clock = VirtualClock::new();
        clock.advance_us(16_000);
        let mut effect = slip_effect(VibrationEffect::TyreSlip, TyreId::AllFour);
        assert_eq!(effect.play_with_clock(&sim, &clock), 0.0);
        clock.advance_us(16_000);
        let dt = effect.filter.seconds_since_last(&clock);
        let tick_s = 16.0 / (crate::device::MS_PER_SECOND as f64);
        assert!((dt - tick_s).abs() < 1e-9);
    }
}
