#ifndef DR2_HAPTIC_TELEMETRY_H
#define DR2_HAPTIC_TELEMETRY_H

#include "simapi/simapi/simapi.h"
#include "simapi/simapi/simdata.h"

/*
 * simapi currently copies DiRT Rally 2 g-force into X/Y velocity, stores
 * wheels as RL/RR/FL/FR, and leaves tyreslipratio unset. Haptic bump and
 * lockup need Assetto-style wheel order, local chassis velocity, and
 * longitudinal slip. Apply this after simapi_datamap() until that mapping
 * lives on a published simapi pin.
 */
void dr2_apply_haptic_telemetry(SimData* simdata);
void dr2_apply_haptic_siminfo(SimInfo* siminfo);

#endif
