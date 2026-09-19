#ifndef _SOUNDDEVICE_H
#define _SOUNDDEVICE_H

//#ifdef USE_PULSEAUDIO
#include <pulse/pulseaudio.h>
//#else
//#include "portaudio.h"
//#endif


#define MAX_TABLE_SIZE   (6000)
typedef struct
{
    uint8_t last_gear;
    int volume;
    double duration;
    double curr_frequency;
    uint32_t curr_amplitude;
    double curr_duration;
    double phase;
    double noise;
    double play_frequency;
    double play_gain;
    double play_amplitude;
    double harmonic2_gain;
    double harmonic3_gain;
    double pulse_hz;
    double pulse_phase;
    double pulse_depth;
    double pulse_duty;
    double lp1;
    double lp2;
}
SoundData;

#endif
