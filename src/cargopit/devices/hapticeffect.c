#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <math.h>
#include <time.h>

#include "usbhapticdevice.h"
#include "simdevice.h"
#include "../../helper/confighelper.h"
#include "../../simulatorapi/simapi/simapi/simdata.h"
#include "../../simulatorapi/simapi/simapi/simmapper.h"
#include "../../slog/slog.h"

#define kmhtoms      0.277778
#define minspeedinms 0.5
#define minvelocity  50
#define maxbrake     0
#define maxthrottle  0
#define maxXvelocity 0.001
#define minYvelocity 0
#define maxZvelocity 1
#define HAPTIC_BRAKE_APPLIED_FRAC 0.05
#define HAPTIC_THROTTLE_APPLIED_FRAC 0.08
#define HAPTIC_ABS_PUMP_ALPHA 0.35
#define HAPTIC_ABS_MIN_PUMP 0.04
#define HAPTIC_SLIP_WHEELSPIN 0
#define HAPTIC_SLIP_LOCKUP 1
#define HAPTIC_SUSP_MIN_SPEED_KMH 1
#define HAPTIC_SUSP_FROZEN_TICKS 12
#define HAPTIC_SUSP_VEL_EMA_ALPHA 0.25
#define HAPTIC_SUSP_ENV_ALPHA 0.06
#define HAPTIC_SUSP_IMPACT_RATIO 2.8
/* The smoothing constants above were tuned per tick at the default 60 fps device rate. */
#define HAPTIC_FILTER_REFERENCE_HZ 60.0
#define HAPTIC_FILTER_MIN_DT_S 0.0001
#define HAPTIC_FILTER_MAX_DT_S 0.25
#define HAPTIC_SUSP_FROZEN_SECONDS (HAPTIC_SUSP_FROZEN_TICKS / HAPTIC_FILTER_REFERENCE_HZ)
#define HAPTIC_NS_PER_S 1000000000.0


bool hasTyreDiameter(const SimData* simdata)
{
    if (simdata->tyrediameter[0] <= 0 || simdata->tyrediameter[1] <= 0 || simdata->tyrediameter[2] <= 0 || simdata->tyrediameter[3] <= 0)
    {
        slogt("failed to find tyre diameter data");
        return false;
    }
    slogt("tyre diameter data found");
    return true;
}

int loadtyreconfig(SimData* simdata, char* configfile, bool setDiameters)
{
    config_t cfg;
    config_init(&cfg);
    const config_setting_t* config_cars = NULL;

    if (!config_read_file(&cfg, configfile))
    {
        slogw("Could not open diameters save file.");
        config_destroy(&cfg);
        return 1;
    }

    config_cars = config_lookup(&cfg, "cars");

    if(config_cars == NULL)
    {
        slogd("diameters config file corrupted");
        config_destroy(&cfg);
        return 1;
    }
    slogt("parsing diameters config file");

    const int cars = config_setting_length(config_cars);

    bool foundCar = false;
    int i = 0;
    while (i<cars)
    {
        DeviceSettings settings;

        config_setting_t* config_car = config_setting_get_elem(config_cars, i);
        if(config_car != NULL)
        {
            const char* car = NULL;
            const char* simstr;
            int sim = 0;
            double tyre0;
            double tyre1;
            double tyre2;
            double tyre3;
            config_setting_lookup_string(config_car, "car", &car);
            config_setting_lookup_float(config_car, "tyre0", &tyre0);
            config_setting_lookup_float(config_car, "tyre1", &tyre1);
            config_setting_lookup_float(config_car, "tyre2", &tyre2);
            config_setting_lookup_float(config_car, "tyre3", &tyre3);
            int found = config_setting_lookup_int(config_car, "sim", &sim);
            //if(found == CONFIG_FALSE)
            //{
            //    int found = config_setting_lookup_int(config_car, "sim", &sim);
            //}
            //else
            //{
            //    sim = simapi_strtogame(simstr);
            //}


            if(simdata->car != NULL && car != NULL)
            {
                if(simdata->car[0] != '\0' && car[0] != '\0')
                {
                    slogt("%s %s %i %i", simdata->car, car, simdata->simexe, sim);
                    if (strcicmp(car, simdata->car) == 0 && sim == simdata->simexe)
                    {
                        slogi("found saved car %s with tyre diameters %f %f %f %f", car, tyre0, tyre1, tyre2, tyre3);
                        foundCar = true;
                        if(setDiameters == true)
                        {
                            simdata->tyrediameter[0] = tyre0;
                            simdata->tyrediameter[1] = tyre1;
                            simdata->tyrediameter[2] = tyre2;
                            simdata->tyrediameter[3] = tyre3;

                        }
                        break;
                    }
                }
            }
        }
        else
        {
            slogw("Possible corruption in config file on entry %i attempting to continue", i+1);
        }
        i++;
    }


    config_destroy(&cfg);

    if(foundCar == true)
    {
        return i;
    }
    return -1;
}

int savetyreconfig(SimData* simdata, char* configfile)
{
    config_t cfg;
    config_setting_t* root;
    config_setting_t* array;
    config_setting_t* carobject;
    config_setting_t* setting;


    config_init(&cfg);
    if (!config_read_file(&cfg, configfile))
    {
        slogw("Could not open diameters save file, creating new.");
        root = config_root_setting(&cfg);
        array = config_setting_add(root, "cars", CONFIG_TYPE_LIST);
    }
    else
    {
        array = config_lookup(&cfg, "cars");
    }

    // TODO add check to not add same car-sim combination twice
    carobject = config_setting_add(array, "cars", CONFIG_TYPE_GROUP);

    setting = config_setting_add(carobject, "car", CONFIG_TYPE_STRING);
    config_setting_set_string(setting, simdata->car);
    setting = config_setting_add(carobject, "sim", CONFIG_TYPE_INT64);
    config_setting_set_int64(setting, simdata->simexe);
    setting = config_setting_add(carobject, "tyre0", CONFIG_TYPE_FLOAT);
    config_setting_set_float(setting, simdata->tyrediameter[0]);
    setting = config_setting_add(carobject, "tyre1", CONFIG_TYPE_FLOAT);
    config_setting_set_float(setting, simdata->tyrediameter[1]);
    setting = config_setting_add(carobject, "tyre2", CONFIG_TYPE_FLOAT);
    config_setting_set_float(setting, simdata->tyrediameter[2]);
    setting = config_setting_add(carobject, "tyre3", CONFIG_TYPE_FLOAT);
    config_setting_set_float(setting, simdata->tyrediameter[3]);

    /* Write out the new configuration. */
    if(! config_write_file(&cfg, configfile))
    {
      slogi("Error while writing file.");
      config_destroy(&cfg);
    }

    slogi("New configuration successfully written to: %s for sim %i, car %s\n", configfile, simdata->simexe, simdata->car);

    config_destroy(&cfg);

    return 0;
}

void getTyreDiameter(SimData* simdata)
{
    if(simdata->velocity > minvelocity && simdata->brake <= maxbrake && simdata->gas <= maxthrottle)
    {
        double Speedms = kmhtoms * simdata->velocity;
        if (simdata->Xvelocity/Speedms < maxXvelocity)
        {
            for(int i = 0; i < 4; i++)
            {
                simdata->tyrediameter[i] = Speedms / simdata->tyreRPS[i] * 2;
            }
            slogi("Successfully set tyre diameters for wheel slip effects.");
        }

    }
}

static int tyre_is_selected(CargopitTyreIdentifier selected, int wheel)
{
    if (selected == ALLFOUR)
    {
        return 1;
    }
    if (selected == (CargopitTyreIdentifier)wheel)
    {
        return 1;
    }
    if (selected == FRONTS && (wheel == FRONTLEFT || wheel == FRONTRIGHT))
    {
        return 1;
    }
    if (selected == REARS && (wheel == REARLEFT || wheel == REARRIGHT))
    {
        return 1;
    }
    return 0;
}

static int brake_is_applied(const SimData* simdata)
{
    return simdata->brake > HAPTIC_BRAKE_APPLIED_FRAC;
}

static int throttle_is_applied(const SimData* simdata)
{
    return simdata->gas > HAPTIC_THROTTLE_APPLIED_FRAC;
}

static int car_is_moving_for_tyres(const SimData* simdata)
{
    if (simdata->Yvelocity <= minYvelocity)
    {
        return 0;
    }
    if (fabs(simdata->Zvelocity) > maxZvelocity)
    {
        return 0;
    }
    return 1;
}

static int sim_provides_slip_ratio(const SimData* simdata)
{
    int i;

    if (simdata->simapi == SIMULATORAPI_DIRT_RALLY_2)
    {
        return 1;
    }
    for (i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        if (fabs(simdata->tyreslipratio[i]) != 0.0)
        {
            return 1;
        }
    }
    return 0;
}

static double sum_slip_beyond(
    const double* wheelslip,
    CargopitTyreIdentifier tyre,
    double threshold,
    int lockup)
{
    double play = 0.0;
    int i;
    for (i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        if (!tyre_is_selected(tyre, i))
        {
            continue;
        }
        if (lockup != 0)
        {
            if (wheelslip[i] > threshold)
            {
                play += wheelslip[i] - threshold;
            }
            continue;
        }
        if (wheelslip[i] < -threshold)
        {
            play += fabs(wheelslip[i]) - fabs(threshold);
        }
    }
    return play;
}

static double lock_slip_only(double slip)
{
    if (slip <= 0.0)
    {
        return 0.0;
    }
    return slip;
}

static double reference_ticks(double dt_seconds)
{
    return dt_seconds * HAPTIC_FILTER_REFERENCE_HZ;
}

/* Per-tick EMA alpha rescaled so the time constant stays fixed at any tick rate. */
static double scaled_alpha(double alpha, double dt_seconds)
{
    return 1.0 - pow(1.0 - alpha, reference_ticks(dt_seconds));
}

static double clamp_dt(double dt_seconds)
{
    if (dt_seconds < HAPTIC_FILTER_MIN_DT_S)
    {
        return HAPTIC_FILTER_MIN_DT_S;
    }
    if (dt_seconds > HAPTIC_FILTER_MAX_DT_S)
    {
        return HAPTIC_FILTER_MAX_DT_S;
    }
    return dt_seconds;
}

static void abs_pump_reset(HapticFilterState* f)
{
    memset(f->abs_last_slip, 0, sizeof(f->abs_last_slip));
    f->abs_pump_ema = 0.0;
    f->abs_primed = 0;
}

static double abs_from_lock_pump(
    HapticFilterState* f,
    const SimData* simdata,
    const double* wheelslip,
    CargopitTyreIdentifier tyre,
    double threshold,
    double dt_seconds)
{
    double max_lock = 0.0;
    double max_ds = 0.0;
    int i;

    if (simdata == NULL || wheelslip == NULL || !brake_is_applied(simdata))
    {
        abs_pump_reset(f);
        return 0.0;
    }

    for (i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        double slip;
        if (!tyre_is_selected(tyre, i))
        {
            continue;
        }
        slip = lock_slip_only(wheelslip[i]);
        max_lock = fmax(max_lock, slip);
        if (f->abs_primed != 0)
        {
            max_ds = fmax(max_ds, fabs(slip - f->abs_last_slip[i]));
        }
        f->abs_last_slip[i] = slip;
    }
    f->abs_primed = 1;
    /* Slip change per reference tick, so the pump level does not depend on the device rate. */
    max_ds /= reference_ticks(dt_seconds);
    f->abs_pump_ema += scaled_alpha(HAPTIC_ABS_PUMP_ALPHA, dt_seconds) * (max_ds - f->abs_pump_ema);

    if (max_lock <= threshold)
    {
        return 0.0;
    }
    if (f->abs_pump_ema <= HAPTIC_ABS_MIN_PUMP)
    {
        return 0.0;
    }
    return max_lock - threshold;
}

int haptic_chassis_is_rolling(const SimData* simdata)
{
    if (simdata == NULL)
    {
        return 0;
    }
    if (simdata->velocity < HAPTIC_SUSP_MIN_SPEED_KMH)
    {
        return 0;
    }
    return 1;
}

static int suspension_telemetry_is_frozen(HapticFilterState* f, const SimData* simdata, double dt_seconds)
{
    int same = memcmp(f->susp_last_velocity, simdata->suspvelocity, sizeof(f->susp_last_velocity)) == 0;

    memcpy(f->susp_last_velocity, simdata->suspvelocity, sizeof(f->susp_last_velocity));
    if (!same)
    {
        f->susp_frozen_seconds = 0.0;
        return 0;
    }
    if (f->susp_frozen_seconds < HAPTIC_SUSP_FROZEN_SECONDS)
    {
        f->susp_frozen_seconds += dt_seconds;
    }
    return f->susp_frozen_seconds >= HAPTIC_SUSP_FROZEN_SECONDS;
}

static double suspension_from_velocity(
    HapticFilterState* f,
    const SimData* simdata,
    CargopitTyreIdentifier tyre,
    double threshold,
    double dt_seconds)
{
    double max_motion = 0.0;
    double gate;
    double baseline_alpha = scaled_alpha(HAPTIC_SUSP_VEL_EMA_ALPHA, dt_seconds);
    int rolling = haptic_chassis_is_rolling(simdata);
    int frozen = suspension_telemetry_is_frozen(f, simdata, dt_seconds);
    int i;

    for (i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        double velocity = simdata->suspvelocity[i];

        if (f->susp_baseline_ready[i] == 0)
        {
            f->susp_baseline[i] = velocity;
            f->susp_baseline_ready[i] = 1;
            continue;
        }
        f->susp_baseline[i] += baseline_alpha * (velocity - f->susp_baseline[i]);
        if (!tyre_is_selected(tyre, i))
        {
            continue;
        }
        max_motion = fmax(max_motion, fabs(velocity - f->susp_baseline[i]));
    }

    if (!rolling || frozen)
    {
        f->susp_motion_floor = max_motion;
        return 0.0;
    }

    f->susp_motion_floor += scaled_alpha(HAPTIC_SUSP_ENV_ALPHA, dt_seconds) * (max_motion - f->susp_motion_floor);
    gate = threshold;
    if (f->susp_motion_floor * HAPTIC_SUSP_IMPACT_RATIO > gate)
    {
        gate = f->susp_motion_floor * HAPTIC_SUSP_IMPACT_RATIO;
    }
    if (max_motion <= gate)
    {
        return 0.0;
    }
    return max_motion - gate;
}

int initializeHapticEffect(HapticEffect* h, HapticEffectSettings* hs, CargopitSettings* ms)
{

    switch (hs->effect_type)
    {
        case (EFFECT_ENGINERPM):
            h->effecttype = EFFECT_ENGINERPM;
            slogi("Initializing haptic effect for engine vibrations.");
            break;
        case (EFFECT_GEARSHIFT):
            h->effecttype = EFFECT_GEARSHIFT;
            slogi("Initializing haptic effect for gear shift vibrations.");
            break;
        case (EFFECT_TYRESLIP):
            h->effecttype = EFFECT_TYRESLIP;
            slogi("Initializing haptic effect for tyre slip vibrations.");
            break;
        case (EFFECT_TYRELOCK):
            h->effecttype = EFFECT_TYRELOCK;
            slogi("Initializing haptic effect for tyre lock vibrations.");
            break;
        case (EFFECT_ABSBRAKES):
            h->effecttype = EFFECT_ABSBRAKES;
            slogi("Initializing haptic effect for abs vibrations.");
            break;
    
        case (EFFECT_SUSPENSION):
            h->effecttype = EFFECT_SUSPENSION;
            slogi("Initializing haptic effect for suspension vibrations.");
            break;
        default:
            h->effecttype = hs->effect_type;
            slogw("Initializing unknown haptic effect type %i.", hs->effect_type);
            break;
    }

    h->tyre = hs->tyre;
    slogi("Haptic effect: %i %i, tyre %i %i", h->effecttype, hs->effect_type, h->tyre, hs->tyre);

    h->threshold = hs->threshold;
    h->modulationType = hs->modulation;
    h->basefrequency = hs->frequency;
    h->frequencyMax = hs->frequencyMax;
    h->baseamplitude = hs->amplitude;
    h->amplitudeMax = hs->amplitudeMax;
    h->motorposition = hs->motorposition;
    h->duration = hs->duration;

    slogt("haptic duration: %f", h->duration);
    slogt("haptic base frequency: %i", h->basefrequency);
    slogt("haptic base amplitude: %i", h->baseamplitude);
    slogt("haptic motorposition: %i", h->motorposition);

    h->useconfig = ms->useconfig;
    h->configcheck = &ms->configcheck;
    h->tyrediameterconfig = ms->tyre_diameter_config;
    memset(&h->filter, 0, sizeof(h->filter));
    return 0;
}


static int effect_uses_wheel_slip(VibrationEffectType effecttype)
{
    return effecttype == EFFECT_TYRESLIP || effecttype == EFFECT_TYRELOCK || effecttype == EFFECT_ABSBRAKES;
}

static void calculate_wheel_slip(const SimData* simdata, double* wheelslip)
{
    double speed_ms = kmhtoms * simdata->velocity;
    int i;

    if (!hasTyreDiameter(simdata) || speed_ms <= minspeedinms)
    {
        return;
    }
    for (i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        wheelslip[i] = (speed_ms - simdata->tyrediameter[i] * simdata->tyreRPS[i] / 2) / speed_ms;
    }
    slogt("wheelslip values are %f %f %f %f", wheelslip[0], wheelslip[1], wheelslip[2], wheelslip[3]);
    slogt("velocities (x,y,z) are %f %f %f", simdata->Xvelocity, simdata->Yvelocity, simdata->Zvelocity);
}

static void wheel_slip_for_effect(const SimData* simdata, VibrationEffectType effecttype, double* wheelslip)
{
    memset(wheelslip, 0, sizeof(double) * HAPTIC_WHEEL_COUNT);
    if (sim_provides_slip_ratio(simdata))
    {
        memcpy(wheelslip, simdata->tyreslipratio, sizeof(double) * HAPTIC_WHEEL_COUNT);
        slogt("wheelslip values from sim are %f %f %f %f", wheelslip[0], wheelslip[1], wheelslip[2], wheelslip[3]);
        return;
    }
    if (effect_uses_wheel_slip(effecttype))
    {
        calculate_wheel_slip(simdata, wheelslip);
    }
}

double haptic_effect_play(SimData* simdata, HapticEffect* h, double dt_seconds)
{
    VibrationEffectType effecttype;
    double wheelslip[HAPTIC_WHEEL_COUNT];
    double play = 0.0;

    if (simdata == NULL || h == NULL)
    {
        return 0.0;
    }
    effecttype = h->effecttype;
    dt_seconds = clamp_dt(dt_seconds);
    wheel_slip_for_effect(simdata, effecttype, wheelslip);

    if (effecttype != EFFECT_SUSPENSION && !car_is_moving_for_tyres(simdata))
    {
        return 0.0;
    }

    switch (effecttype)
    {
        case EFFECT_TYRESLIP:
            if (!throttle_is_applied(simdata))
            {
                return 0.0;
            }
            play = sum_slip_beyond(wheelslip, h->tyre, h->threshold, HAPTIC_SLIP_WHEELSPIN);
            slogt("slip is %f", play);
            break;
        case EFFECT_TYRELOCK:
            if (!brake_is_applied(simdata))
            {
                return 0.0;
            }
            play = sum_slip_beyond(wheelslip, h->tyre, h->threshold, HAPTIC_SLIP_LOCKUP);
            slogt("lock is %f", play);
            break;
        case EFFECT_ABSBRAKES:
            play = abs_from_lock_pump(&h->filter, simdata, wheelslip, h->tyre, h->threshold, dt_seconds);
            slogt("abs is %f", play);
            break;
        case EFFECT_SUSPENSION:
            play = suspension_from_velocity(&h->filter, simdata, h->tyre, h->threshold, dt_seconds);
            slogt("suspension is %f", play);
            break;
        default:
            slogw("Unknown effect type %i", effecttype);
            break;
    }

    return play;
}

static double seconds_since_last_update(HapticFilterState* f)
{
    struct timespec now;
    uint64_t now_ns;
    uint64_t last_ns = f->last_update_ns;

    clock_gettime(CLOCK_MONOTONIC, &now);
    now_ns = (uint64_t) now.tv_sec * (uint64_t) HAPTIC_NS_PER_S + (uint64_t) now.tv_nsec;
    f->last_update_ns = now_ns;
    if (last_ns == 0)
    {
        return 1.0 / HAPTIC_FILTER_REFERENCE_HZ;
    }
    return (double) (now_ns - last_ns) / HAPTIC_NS_PER_S;
}

double slipeffect(SimData* simdata, HapticEffect* h)
{
    if (h == NULL)
    {
        return 0.0;
    }
    return haptic_effect_play(simdata, h, seconds_since_last_update(&h->filter));
}
