#ifndef _USB_GENERIC_SHAKER_H
#define _USB_GENERIC_SHAKER_H

#include <stdint.h>
#include "../simdevice.h"

#define SHAKER_ENGINE_CYLINDERS 4
#define SHAKER_ENGINE_STROKE_CYCLES 2
#define SHAKER_ENGINE_SECONDS_PER_MINUTE 60.0

/* Floor: seat is dead below ~32 Hz. Map idle firing → 120 Hz across the
 * car's full RPM range so pitch never plateaus; LPF matches the ceiling. */
#define SHAKER_TONE_MIN_HZ 32.0
#define SHAKER_TONE_MAX_HZ 120.0
#define SHAKER_OUTPUT_LP_HZ SHAKER_TONE_MAX_HZ

int usb_generic_shaker_init(SoundDevice* sounddevice, pa_threaded_mainloop* mainloop, pa_context* context, const char* devname, int volume, uint32_t channelmask, int channels, const char* streamname);
int usb_generic_shaker_free(SoundDevice* sounddevice, pa_threaded_mainloop* mainloop);
//#else
//int usb_generic_shaker_init(SoundDevice* sounddevice);
//int usb_generic_shaker_free(SoundDevice* sounddevice);
//#endif

#endif
