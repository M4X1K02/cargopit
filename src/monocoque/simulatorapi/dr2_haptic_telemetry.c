#include <math.h>

#include "dr2_haptic_telemetry.h"
#include "simapi/simapi/simapi.h"

#define DR2_AXIS_X 0
#define DR2_AXIS_Y 1
#define DR2_AXIS_Z 2
#define DR2_WHEEL_COUNT 4
#define DR2_SLIP_MIN_SPEED_MS 0.5

static double dr2_dot_world_velocity(const SimData* simdata, const double* axis)
{
    return simdata->worldXvelocity * axis[DR2_AXIS_X]
           + simdata->worldYvelocity * axis[DR2_AXIS_Y]
           + simdata->worldZvelocity * axis[DR2_AXIS_Z];
}

static double dr2_world_speed_ms(const SimData* simdata)
{
    double vx = simdata->worldXvelocity;
    double vy = simdata->worldYvelocity;
    double vz = simdata->worldZvelocity;

    return sqrt(vx * vx + vy * vy + vz * vz);
}

static void dr2_project_local_velocity(SimData* simdata)
{
    /* dirt2mapper stores right/up/forward in tyrecontact0/1/2. */
    simdata->Xvelocity = dr2_dot_world_velocity(simdata, simdata->tyrecontact0);
    simdata->Yvelocity = dr2_dot_world_velocity(simdata, simdata->tyrecontact2);
    simdata->Zvelocity = dr2_dot_world_velocity(simdata, simdata->tyrecontact1);
}

static void dr2_map_one_wheel_slip(SimData* simdata, int wheel, double chassis_speed_ms)
{
    if (chassis_speed_ms < DR2_SLIP_MIN_SPEED_MS)
    {
        simdata->tyreslipratio[wheel] = 0.0;
        return;
    }

    /* DR2 UDP wheelSpeed is linear m/s; simapi stores that in tyreRPS. */
    simdata->tyreslipratio[wheel] =
        (chassis_speed_ms - simdata->tyreRPS[wheel]) / chassis_speed_ms;
}

static void dr2_map_longitudinal_slip(SimData* simdata)
{
    double chassis_speed_ms = dr2_world_speed_ms(simdata);
    int wheel;

    for (wheel = 0; wheel < DR2_WHEEL_COUNT; wheel++)
    {
        dr2_map_one_wheel_slip(simdata, wheel, chassis_speed_ms);
    }
}

void dr2_apply_haptic_telemetry(SimData* simdata)
{
    if (simdata == NULL)
    {
        return;
    }
    if (simdata->simapi != SIMULATORAPI_DIRT_RALLY_2)
    {
        return;
    }

    dr2_project_local_velocity(simdata);
    dr2_map_longitudinal_slip(simdata);
}
