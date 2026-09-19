#ifndef DR2_HAPTIC_TELEMETRY_H
#define DR2_HAPTIC_TELEMETRY_H

#include "simapi/simapi/simdata.h"

/*
 * simapi currently copies DiRT Rally 2 g-force into X/Y velocity and leaves
 * tyreslipratio unset. Haptic bump and lockup need local chassis velocity
 * and longitudinal slip. Apply this after simapi_datamap() until that mapping
 * lives on a published simapi pin.
 */
void dr2_apply_haptic_telemetry(SimData* simdata);

#endif
