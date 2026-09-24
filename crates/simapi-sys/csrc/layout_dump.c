#include <stddef.h>
#include <stdio.h>

#include "simapi.h"
#include "simdata.h"

#define EMIT_OFF(name, field) \
    printf("pub const " name ": usize = %zu;\n", offsetof(SimData, field))

int main(void)
{
    if (sizeof(SimData) != 46044 || SIMAPI_VERSION != 1)
    {
        return 1;
    }

    printf("pub const SIMDATA_SIZE: usize = %zu;\n", sizeof(SimData));
    printf("pub const SIMAPI_VERSION_VALUE: u32 = %d;\n", SIMAPI_VERSION);
    printf("pub const BOOL_SIZE: usize = %zu;\n", sizeof(bool));
    printf("pub const F64_SIZE: usize = %zu;\n", sizeof(double));
    EMIT_OFF("OFF_MTICK", mtick);
    EMIT_OFF("OFF_SIMSTATUS", simstatus);
    EMIT_OFF("OFF_VELOCITY", velocity);
    EMIT_OFF("OFF_RPMS", rpms);
    EMIT_OFF("OFF_GEAR", gear);
    EMIT_OFF("OFF_PULSES", pulses);
    EMIT_OFF("OFF_MAXRPM", maxrpm);
    EMIT_OFF("OFF_IDLERPM", idlerpm);
    EMIT_OFF("OFF_LAP", lap);
    EMIT_OFF("OFF_POSITION", position);
    EMIT_OFF("OFF_NUMLAPS", numlaps);
    EMIT_OFF("OFF_GEARC", gearc);
    printf("pub const GEARC_BYTES: usize = %zu;\n", sizeof(((SimData*)0)->gearc));
    printf("pub const CAR_BYTES: usize = %zu;\n", sizeof(((SimData*)0)->car));
    printf("pub const TRACK_BYTES: usize = %zu;\n", sizeof(((SimData*)0)->track));
    EMIT_OFF("OFF_XVELOCITY", Xvelocity);
    EMIT_OFF("OFF_YVELOCITY", Yvelocity);
    EMIT_OFF("OFF_ZVELOCITY", Zvelocity);
    EMIT_OFF("OFF_GAS", gas);
    EMIT_OFF("OFF_BRAKE", brake);
    EMIT_OFF("OFF_ABS", abs);
    EMIT_OFF("OFF_TYRE_RPS", tyreRPS);
    EMIT_OFF("OFF_TYRE_DIAMETER", tyrediameter);
    EMIT_OFF("OFF_TYRE_SLIP_RATIO", tyreslipratio);
    EMIT_OFF("OFF_BRAKE_TEMP", braketemp);
    EMIT_OFF("OFF_SUSPENSION", suspension);
    EMIT_OFF("OFF_SUSP_VELOCITY", suspvelocity);
    EMIT_OFF("OFF_COURSE_FLAG", courseflag);
    EMIT_OFF("OFF_PLAYER_FLAG", playerflag);
    EMIT_OFF("OFF_CAR", car);
    EMIT_OFF("OFF_TRACK", track);
    EMIT_OFF("OFF_CLUTCH", clutch);
    EMIT_OFF("OFF_STEER", steer);
    EMIT_OFF("OFF_FUEL", fuel);
    EMIT_OFF("OFF_FUELCAPACITY", fuelcapacity);
    EMIT_OFF("OFF_TURBOBOOST", turboboost);
    EMIT_OFF("OFF_TYRE_TEMP", tyretemp);
    EMIT_OFF("OFF_PROXIMITY", pd);
    EMIT_OFF("OFF_SIMAPI", simapi);
    EMIT_OFF("OFF_SIMEXE", simexe);
    EMIT_OFF("OFF_SIMON", simon);
    EMIT_OFF("OFF_SIMAPIVERSION", simapiversion);
    printf("pub const PROXIMITY_STRIDE: usize = %zu;\n", sizeof(ProximityData));
    printf("pub const OFF_PROX_RADIUS: usize = %zu;\n", offsetof(ProximityData, radius));
    printf("pub const OFF_PROX_THETA: usize = %zu;\n", offsetof(ProximityData, theta));
    return 0;
}
