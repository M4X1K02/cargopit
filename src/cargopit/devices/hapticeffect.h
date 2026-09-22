#ifndef _HAPTICEFFECT_H
#define _HAPTICEFFECT_H

#include <stdio.h>
#include <stdint.h>
#include "../simulatorapi/simapi/simapi/simdata.h"

#define HAPTIC_WHEEL_COUNT 4


typedef enum
{
    HAPTIC_EFFECT_MODULATION_NONE            = 0,
    HAPTIC_EFFECT_MODULATION_FREQUENCY       = 1,
    HAPTIC_EFFECT_MODULATION_AMPLIFY         = 2,
}
HapticEffectModulationType;


typedef struct
{
    uint32_t curr_frequency;
    uint32_t curr_amplitude;
    double curr_duration;
    uint32_t last_gear;
}
CurrentEffectData;

/* Smoothing state for one device's effect; never shared between devices. */
typedef struct
{
    uint64_t last_update_ns;
    double abs_last_slip[HAPTIC_WHEEL_COUNT];
    double abs_pump_ema;
    int abs_primed;
    double susp_last_velocity[HAPTIC_WHEEL_COUNT];
    double susp_frozen_seconds;
    double susp_baseline[HAPTIC_WHEEL_COUNT];
    int susp_baseline_ready[HAPTIC_WHEEL_COUNT];
    double susp_motion_floor;
}
HapticFilterState;

typedef struct
{
    VibrationEffectType effecttype;
    CargopitTyreIdentifier tyre;
    HapticEffectModulationType modulationType;

    uint32_t volume;
    double duration;
    double threshold;
    uint32_t motorposition;
    uint32_t basefrequency;
    uint32_t frequencyMax;
    uint32_t baseamplitude;
    uint32_t amplitudeMax;
   
    CurrentEffectData live_effect;
    HapticFilterState filter;

    int useconfig;
    int* configcheck;
    char* tyrediameterconfig;
}
HapticEffect;

int initializeHapticEffect(HapticEffect* h, HapticEffectSettings* hs, CargopitSettings* ms);
int haptic_chassis_is_rolling(const SimData* simdata);
/* Effect intensity for one device tick; dt_seconds is the time since that device's previous tick. */
double haptic_effect_play(SimData* simdata, HapticEffect* h, double dt_seconds);
/* haptic_effect_play() with dt measured from the monotonic clock. */
double slipeffect(SimData* simdata, HapticEffect* h);
bool hasTyreDiameter(const SimData* simdata);
int loadtyreconfig(SimData* simdata, char* configfile, bool setDiameters);
int savetyreconfig(SimData* simdata, char* configfile);
void getTyreDiameter(SimData* simdata);

#endif
