#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#include <stdio.h>
#include <unistd.h>
#include <stdint.h>
#include <math.h>

#include "sound.h"
#include "simdevice.h"
#include "sounddevice.h"
#include "hapticeffect.h"
#include "sound/usb_generic_shaker.h"
#include "sound/custom_frequency_response.h"
#include "../simulatorapi/simapi/simapi/simdata.h"
#include "../helper/parameters.h"
#include "../slog/slog.h"

#define ENGINE_IDLE_AMP_FRAC 0.20
#define ENGINE_LOAD_AMP_WEIGHT 0.75
#define ENGINE_RPM_AMP_WEIGHT 0.12
#define ENGINE_HARMONIC2_GAIN 0.0
#define ENGINE_HARMONIC3_GAIN 0.0
#define ENGINE_PULSE_DEPTH 0.0
#define GEAR_DURATION_DEFAULT_S 0.10
#define HAPTIC_SUSPENSION_PLAY_REF 8.0
#define HAPTIC_SUSP_GAMMA 0.50
#define HAPTIC_ABS_PLAY_REF 0.25
#define HAPTIC_ABS_PULSE_ON_S 0.050
#define HAPTIC_ABS_PULSE_PERIOD_S 0.125
#define HAPTIC_ABS_PULSE_HZ (1.0 / HAPTIC_ABS_PULSE_PERIOD_S)
#define HAPTIC_ABS_PULSE_DUTY (HAPTIC_ABS_PULSE_ON_S / HAPTIC_ABS_PULSE_PERIOD_S)
#define HAPTIC_ABS_PULSE_DEPTH 1.0
#define HAPTIC_SLIP_PLAY_REF 0.35

static double clamp_unit(double v)
{
    if (v < 0.0)
    {
        return 0.0;
    }
    if (v > 1.0)
    {
        return 1.0;
    }
    return v;
}

static double engine_amp_frac(double throttle, uint32_t rpms, uint32_t maxrpm)
{
    double load;
    double rpm_term;
    double amp;

    load = clamp_unit(throttle);
    if (maxrpm == 0)
    {
        rpm_term = 0.0;
    }
    else
    {
        rpm_term = clamp_unit((double)rpms / (double)maxrpm);
    }
    amp = ENGINE_IDLE_AMP_FRAC
        + ENGINE_LOAD_AMP_WEIGHT * load
        + ENGINE_RPM_AMP_WEIGHT * rpm_term;
    return clamp_unit(amp);
}

static double engine_firing_hz(uint32_t rpms)
{
    if (rpms == 0)
    {
        return 0.0;
    }
    return ((double)rpms * (double)SHAKER_ENGINE_CYLINDERS)
        / (SHAKER_ENGINE_SECONDS_PER_MINUTE * (double)SHAKER_ENGINE_STROKE_CYCLES);
}

static double engine_rpm_frac(uint32_t rpms, uint32_t idlerpm, uint32_t maxrpm)
{
    if (rpms == 0 || maxrpm == 0)
    {
        return 0.0;
    }
    double idle = (double)idlerpm;
    double maxr = (double)maxrpm;
    double rpm = (double)rpms;
    if (idle <= 0.0 || idle >= maxr)
    {
        return clamp_unit(rpm / maxr);
    }
    return clamp_unit((rpm - idle) / (maxr - idle));
}

static double engine_band_min_hz(double configured_min_hz)
{
    if (configured_min_hz < SHAKER_TONE_MIN_HZ || configured_min_hz >= SHAKER_TONE_MAX_HZ)
    {
        return SHAKER_TONE_MIN_HZ;
    }
    return configured_min_hz;
}

static double engine_band_max_hz(double configured_max_hz, double band_min_hz)
{
    if (configured_max_hz <= band_min_hz || configured_max_hz > SHAKER_TONE_MAX_HZ)
    {
        return SHAKER_TONE_MAX_HZ;
    }
    return configured_max_hz;
}

static double engine_idle_tone_hz(uint32_t idlerpm, double band_min_hz, double band_max_hz)
{
    double hz = engine_firing_hz(idlerpm);
    if (hz < band_min_hz || hz >= band_max_hz)
    {
        return band_min_hz;
    }
    return hz;
}

static double engine_tone_hz(
    uint32_t rpm,
    uint32_t idlerpm,
    uint32_t maxrpm,
    double configured_min_hz,
    double configured_max_hz)
{
    double band_min_hz = engine_band_min_hz(configured_min_hz);
    double band_max_hz = engine_band_max_hz(configured_max_hz, band_min_hz);
    double idle_hz = engine_idle_tone_hz(idlerpm, band_min_hz, band_max_hz);
    double rpm_frac = engine_rpm_frac(rpm, idlerpm, maxrpm);
    return idle_hz + (band_max_hz - idle_hz) * rpm_frac;
}

static uint32_t engine_play_rpm(const SimData* simdata)
{
    if (simdata == NULL || simdata->maxrpm == 0)
    {
        return 0;
    }
    if (simdata->rpms > 0)
    {
        if (simdata->idlerpm > 0 && simdata->rpms < simdata->idlerpm)
        {
            return simdata->idlerpm;
        }
        return simdata->rpms;
    }
    if (simdata->gear == SIMAPI_GEAR_NEUTRAL && simdata->idlerpm > 0)
    {
        return simdata->idlerpm;
    }
    return 0;
}

static uint32_t lerp_u32(uint32_t a, uint32_t b, double t)
{
    t = clamp_unit(t);
    double v = (double)a + ((double)b - (double)a) * t;
    if (v < 0.0)
    {
        return 0;
    }
    return (uint32_t)lrint(v);
}

static uint32_t haptic_amplitude_from_level(double level)
{
    return lerp_u32(0, HAPTIC_AMPLITUDE_UNITY, level);
}

int gear_sound_set(SimDevice* this, SimData* simdata)
{
    SoundDevice* sounddevice = (void *) this->derived;
    SoundData* data = &sounddevice->sounddata;

    if (data->last_gear == simdata->gear)
    {
        return 0;
    }

    data->last_gear = simdata->gear;
    data->curr_frequency = this->hapticeffect.basefrequency;
    data->curr_amplitude = HAPTIC_AMPLITUDE_UNITY;
    data->curr_duration = 0.0;
    data->phase = 0.0;
    data->play_frequency = data->curr_frequency;
    data->play_amplitude = (double)data->curr_amplitude;
    data->play_gain = 1.0;

    slogt("set gear frequency to %f", data->curr_frequency);
    return 0;
}


double modulate(SimDevice* this, double raw_effect, HapticEffectModulationType modulation)
{
    SoundDevice* sounddevice = (void *) this->derived;

    if (raw_effect < 0.0)
    {
        raw_effect = 0.0;
    }
    if (raw_effect > 1.0)
    {
        raw_effect = 1.0;
    }

    double modulated_effect = raw_effect;
    switch (modulation)
    {
        case HAPTIC_EFFECT_MODULATION_FREQUENCY:
            modulated_effect = ((this->hapticeffect.frequencyMax - this->hapticeffect.basefrequency) * raw_effect) + this->hapticeffect.basefrequency;
            sounddevice->sounddata.curr_frequency = modulated_effect;
            sounddevice->sounddata.curr_amplitude = HAPTIC_AMPLITUDE_UNITY;
            slogt("set curr frequency to %f from raw effect %f and base frequency %i", sounddevice->sounddata.curr_frequency, raw_effect, this->hapticeffect.basefrequency);
            break;
        case HAPTIC_EFFECT_MODULATION_AMPLIFY:
            sounddevice->sounddata.curr_amplitude = haptic_amplitude_from_level(raw_effect);
            sounddevice->sounddata.curr_frequency = this->hapticeffect.basefrequency;
            slogt("set curr amplitude to %i from raw effect %f", sounddevice->sounddata.curr_amplitude, raw_effect);
            break;
        case HAPTIC_EFFECT_MODULATION_NONE:
        default:
            sounddevice->sounddata.curr_frequency = this->hapticeffect.basefrequency;
            sounddevice->sounddata.curr_amplitude = HAPTIC_AMPLITUDE_UNITY;
            break;
    }

    return modulated_effect;
}

// we could make a vtable for these different effects too
int sounddev_engine_update(SimDevice* this, SimData* simdata)
{
    SoundDevice* sounddevice = (void *) this->derived;
    SoundData* data = &sounddevice->sounddata;
    uint32_t rpm = engine_play_rpm(simdata);

    if (rpm == 0 || simdata->maxrpm == 0)
    {
        data->curr_frequency = 0.0;
        data->curr_amplitude = 0;
        data->pulse_hz = 0.0;
        return 0;
    }

    double firing_hz = engine_firing_hz(rpm);
    double amp_frac = engine_amp_frac(simdata->gas, rpm, simdata->maxrpm);

    data->duration = 0.0;
    data->curr_frequency = engine_tone_hz(
        rpm,
        simdata->idlerpm,
        simdata->maxrpm,
        (double)this->hapticeffect.basefrequency,
        (double)this->hapticeffect.frequencyMax);
    amp_frac *= custom_frequency_response_analysis_resonance_band_amplitude_scale(
        data->curr_frequency);
    data->curr_amplitude = haptic_amplitude_from_level(amp_frac);
    data->harmonic2_gain = ENGINE_HARMONIC2_GAIN;
    data->harmonic3_gain = ENGINE_HARMONIC3_GAIN;
    data->pulse_depth = ENGINE_PULSE_DEPTH;
    data->pulse_hz = 0.0;

    slogt("engine rumble rpm %u firing %f tone %f throttle %f amp %u",
          rpm, firing_hz, data->curr_frequency, simdata->gas, data->curr_amplitude);
    return 0;
}

static int sounddev_continuous_tone_update(SimDevice* this, SimData* simdata, double play_ref)
{
    SoundDevice* sounddevice = (void *) this->derived;
    SoundData* data = &sounddevice->sounddata;
    double play;
    double level;

    play = slipeffect(simdata, &this->hapticeffect, this->hapticeffect.useconfig, this->hapticeffect.configcheck, this->hapticeffect.tyrediameterconfig);
    if (play <= 0.0 || play_ref <= 0.0)
    {
        data->curr_frequency = 0.0;
        data->curr_amplitude = 0;
        data->curr_duration = 0.0;
        return 0;
    }

    level = clamp_unit(play / play_ref);

    data->duration = 0.0;
    data->curr_frequency = (double)this->hapticeffect.basefrequency;
    data->curr_amplitude = haptic_amplitude_from_level(level);
    slogt("continuous tone level %f freq %f amp %u",
          level, data->curr_frequency, data->curr_amplitude);
    return 0;
}

int sounddev_tyreslip_update(SimDevice* this, SimData* simdata)
{
    return sounddev_continuous_tone_update(this, simdata, HAPTIC_SLIP_PLAY_REF);
}

int sounddev_tyrelock_update(SimDevice* this, SimData* simdata)
{
    return sounddev_continuous_tone_update(this, simdata, HAPTIC_SLIP_PLAY_REF);
}

int sounddev_absbrakes_update(SimDevice* this, SimData* simdata)
{
    SoundDevice* sounddevice = (void *) this->derived;
    SoundData* data = &sounddevice->sounddata;
    double play;
    double level;

    play = slipeffect(simdata, &this->hapticeffect, this->hapticeffect.useconfig, this->hapticeffect.configcheck, this->hapticeffect.tyrediameterconfig);
    if (play <= 0.0 || HAPTIC_ABS_PLAY_REF <= 0.0)
    {
        data->curr_frequency = 0.0;
        data->curr_amplitude = 0;
        data->curr_duration = 0.0;
        data->pulse_hz = 0.0;
        data->pulse_depth = 0.0;
        data->pulse_duty = 0.0;
        return 0;
    }

    level = clamp_unit(play / HAPTIC_ABS_PLAY_REF);

    data->duration = 0.0;
    data->curr_frequency = (double)this->hapticeffect.basefrequency;
    data->curr_amplitude = haptic_amplitude_from_level(level);
    data->pulse_hz = HAPTIC_ABS_PULSE_HZ;
    data->pulse_depth = HAPTIC_ABS_PULSE_DEPTH;
    data->pulse_duty = HAPTIC_ABS_PULSE_DUTY;
    slogt("abs vibration level %f freq %f amp %u pulse %f",
          level, data->curr_frequency, data->curr_amplitude, data->pulse_hz);
    return 0;
}

int sounddev_suspension_update(SimDevice* this, SimData* simdata)
{
    SoundDevice* sounddevice = (void *) this->derived;
    SoundData* data = &sounddevice->sounddata;
    double effect;
    double level;
    uint32_t fmin;
    uint32_t fmax;

    if (!haptic_chassis_is_rolling(simdata))
    {
        data->curr_frequency = 0.0;
        data->curr_amplitude = 0;
        data->curr_duration = 0.0;
        data->duration = 0.0;
        data->play_gain = 0.0;
        data->play_frequency = 0.0;
        data->play_amplitude = 0.0;
        return 0;
    }

    effect = slipeffect(simdata, &this->hapticeffect, this->hapticeffect.useconfig, this->hapticeffect.configcheck, this->hapticeffect.tyrediameterconfig);
    if (effect <= 0.0 || HAPTIC_SUSPENSION_PLAY_REF <= 0.0)
    {
        data->curr_frequency = 0.0;
        data->curr_amplitude = 0;
        return 0;
    }

    level = clamp_unit(effect / HAPTIC_SUSPENSION_PLAY_REF);
    level = pow(level, HAPTIC_SUSP_GAMMA);

    fmin = this->hapticeffect.basefrequency;
    fmax = this->hapticeffect.frequencyMax;
    if (fmax < fmin)
    {
        fmax = fmin;
    }

    data->duration = 0.0;
    data->curr_frequency = (double)fmin + ((double)fmax - (double)fmin) * level;
    data->curr_amplitude = haptic_amplitude_from_level(level);
    slogt("suspension vibration level %f freq %f amp %u",
          level, data->curr_frequency, data->curr_amplitude);
    return 0;
}

int sounddev_gearshift_update(SimDevice* this, SimData* simdata)
{
    gear_sound_set(this, simdata);
}


int sounddev_free(SimDevice* this)
{
    SoundDevice* sounddevice = (void *) this->derived;

    usb_generic_shaker_free(sounddevice, mainloop);
    free(sounddevice);

    return 0;
}

int sounddev_init(SoundDevice* sounddevice, const char* devname, SoundDeviceSettings sds)
{
    slogi("initializing standalone sound device...");


    slogi("pipewire stream volume is: %i", sds.volume);
    slogi("pan is: %i", sds.pan);
    slogi("channels is: %i", sds.channels);
    slogi("noise is: %i", sds.noise);


    sounddevice->sounddata.noise = (double)sds.noise;
    sounddevice->sounddata.curr_duration = 0;
    sounddevice->sounddata.duration = 0.0;

    sounddevice->sounddata.phase = 0;

    sounddevice->sounddata.curr_amplitude = 0;
    sounddevice->sounddata.curr_frequency = 0.0;
    sounddevice->sounddata.play_frequency = 0.0;
    sounddevice->sounddata.play_gain = 0.0;
    sounddevice->sounddata.play_amplitude = 0.0;
    sounddevice->sounddata.harmonic2_gain = 0.0;
    sounddevice->sounddata.harmonic3_gain = 0.0;
    sounddevice->sounddata.pulse_hz = 0.0;
    sounddevice->sounddata.pulse_phase = 0.0;
    sounddevice->sounddata.pulse_depth = 0.0;
    sounddevice->sounddata.pulse_duty = 0.0;
    sounddevice->sounddata.lp1 = 0.0;
    sounddevice->sounddata.lp2 = 0.0;


    const char* streamname= "Engine";
    switch (sounddevice->m.hapticeffect.effecttype) {
        case (EFFECT_GEARSHIFT):
            sounddevice->sounddata.last_gear = 0;
            sounddevice->sounddata.duration = sounddevice->m.hapticeffect.duration;
            if (sounddevice->sounddata.duration <= 0.0)
            {
                sounddevice->sounddata.duration = GEAR_DURATION_DEFAULT_S;
            }
            streamname = "Gear";
            break;
        case (EFFECT_TYRESLIP):
            streamname = "TyreSlip";
            break;
        case (EFFECT_TYRELOCK):
            streamname = "TyreLock";
            break;
        case (EFFECT_ABSBRAKES):
            streamname = "ABS";
            break;
        case (EFFECT_SUSPENSION):
            streamname = "Suspension";
            break;
        case (EFFECT_ENGINERPM):
        default:
            streamname = "Engine";
            break;
    }


    // Returned, not discarded: this function is declared int and used to fall
    // off its end, so new_sound_device() read whatever happened to be in the
    // return register and treated most devices as failures -- "Could not
    // initialize Sound Device" for 22 of 24 configured shakers, while their
    // PulseAudio streams had in fact connected. The devices were freed and
    // never fed telemetry, so the graph looked correct in qpwgraph and nothing
    // shook. Being undefined behaviour it varied by build, which is why the
    // same config worked against a locally compiled cargopit and not the
    // packaged one.
    return usb_generic_shaker_init(sounddevice, mainloop, context, devname, sds.volume, sds.pan, sds.channels, streamname);
}

static const vtable engine_sound_simdevice_vtable = { &sounddev_engine_update, &sounddev_free };
static const vtable gear_sound_simdevice_vtable = { &sounddev_gearshift_update, &sounddev_free };
static const vtable tyreslip_sound_simdevice_vtable = { &sounddev_tyreslip_update, &sounddev_free };
static const vtable tyrelock_sound_simdevice_vtable = { &sounddev_tyrelock_update, &sounddev_free };
static const vtable absbrakes_sound_simdevice_vtable = { &sounddev_absbrakes_update, &sounddev_free };
static const vtable suspension_sound_simdevice_vtable = { &sounddev_suspension_update, &sounddev_free };

SoundDevice* new_sound_device(DeviceSettings* ds, CargopitSettings* ms, SimInfo* siminfo) {

    SoundDevice* this = (SoundDevice*) calloc(1, sizeof(SoundDevice));

    this->m.update = &update;
    this->m.free = &simdevfree;
    this->m.derived = this;
    int error = 0;

    switch (ds->hapticsettings.effect_type)
    {
        case (EFFECT_TYRESLIP):
        case (EFFECT_TYRELOCK):
        case (EFFECT_ABSBRAKES):
        case (EFFECT_SUSPENSION):
            if(siminfo->SimSupportsHapticEffects == false)
            {
                slogw("Skipping sound effect setup because sim does not support haptic effects");
                error = CARGOPIT_ERROR_UNSUPPORTED_SIM_FEATURE;
            }
        defaut:
            error = 0;
    }


    if(error == 0)
    {
        initializeHapticEffect(&this->m.hapticeffect, &ds->hapticsettings, ms);
        slogt("Attempting to configure sound device with subtype: %i", ds->hapticsettings.effect_type);
        switch (ds->hapticsettings.effect_type)
        {
            case (EFFECT_ENGINERPM):
                this->m.vtable = &engine_sound_simdevice_vtable;
                slogi("Initializing sound device for engine vibrations.");
                break;
            case (EFFECT_GEARSHIFT):
                this->m.vtable = &gear_sound_simdevice_vtable;
                slogi("Initializing sound device for gear shift vibrations.");
                break;
            case (EFFECT_TYRESLIP):
                this->m.vtable = &tyreslip_sound_simdevice_vtable;
                slogi("Initializing sound device for tyre slip vibrations.");
                break;
            case (EFFECT_TYRELOCK):
                this->m.vtable = &tyrelock_sound_simdevice_vtable;
                slogi("Initializing sound device for tyre lock vibrations.");
                break;
            case (EFFECT_ABSBRAKES):
                this->m.vtable = &absbrakes_sound_simdevice_vtable;
                slogi("Initializing sound device for abs vibrations.");
                break;

            case (EFFECT_SUSPENSION):
                this->m.vtable = &suspension_sound_simdevice_vtable;
                slogi("Initializing sound device for suspension vibrations.");
                break;
        }
    }

    if(error == 0)
    {
        slogt("Attempting to use sound device %s", ds->dev);
        error = sounddev_init(this, ds->dev, ds->sounddevsettings);
    }

    if (error != 0)
    {
        free(this);
        return NULL;
    }

    return this;
}
