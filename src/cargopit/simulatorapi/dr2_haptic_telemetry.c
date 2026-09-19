#include <math.h>
#include <stddef.h>

#include "dr2_haptic_telemetry.h"
#include "simapi/simapi/simapi.h"

#define DR2_AXIS_X 0
#define DR2_AXIS_Y 1
#define DR2_AXIS_Z 2
#define DR2_WHEEL_COUNT 4
#define DR2_SLIP_MIN_SPEED_MS 0.5

/*
 * Published dirt2mapper fills tyreRPS/suspension as RL, RR, FL, FR.
 * SimData and haptic tyre selection use FL, FR, RL, RR.
 */
#define DR2_PACKED_RL 0
#define DR2_PACKED_RR 1
#define DR2_PACKED_FL 2
#define DR2_PACKED_FR 3
#define DR2_WHEEL_FL 0
#define DR2_WHEEL_FR 1
#define DR2_WHEEL_RL 2
#define DR2_WHEEL_RR 3

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

static void dr2_unshuffle_wheels(double* wheels)
{
    double packed[DR2_WHEEL_COUNT];
    int i;

    if (wheels == NULL)
    {
        return;
    }

    for (i = 0; i < DR2_WHEEL_COUNT; i++)
    {
        packed[i] = wheels[i];
    }
    wheels[DR2_WHEEL_FL] = packed[DR2_PACKED_FL];
    wheels[DR2_WHEEL_FR] = packed[DR2_PACKED_FR];
    wheels[DR2_WHEEL_RL] = packed[DR2_PACKED_RL];
    wheels[DR2_WHEEL_RR] = packed[DR2_PACKED_RR];
}

static void dr2_unshuffle_wheel_channels(SimData* simdata)
{
    dr2_unshuffle_wheels(simdata->tyreRPS);
    dr2_unshuffle_wheels(simdata->suspvelocity);
    dr2_unshuffle_wheels(simdata->suspension);
    dr2_unshuffle_wheels(simdata->braketemp);
    dr2_unshuffle_wheels(simdata->tyrepressure);
}

static void dr2_project_local_velocity(SimData* simdata)
{
    /* dirt2mapper stores right/up/forward in tyrecontact0/1/2. */
    simdata->Xvelocity = dr2_dot_world_velocity(simdata, simdata->tyrecontact0);
    simdata->Yvelocity = dr2_dot_world_velocity(simdata, simdata->tyrecontact2);
    /*
     * Leave Zvelocity as simapi's 0. Projecting world-up into Z trips the
     * 1 m/s airborne gate on rally bumps and mutes lockup and slip.
     */
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

static int dr2_siminfo_is_dirt_rally_2(const SimInfo* siminfo)
{
    if (siminfo->simulatorapi == SIMULATORAPI_DIRT_RALLY_2)
    {
        return 1;
    }
    if (siminfo->mapapi == SIMULATORAPI_DIRT_RALLY_2)
    {
        return 1;
    }
    return 0;
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

    dr2_unshuffle_wheel_channels(simdata);
    dr2_project_local_velocity(simdata);
    dr2_map_longitudinal_slip(simdata);
}

void dr2_apply_haptic_siminfo(SimInfo* siminfo)
{
    if (siminfo == NULL)
    {
        return;
    }
    if (!dr2_siminfo_is_dirt_rally_2(siminfo))
    {
        return;
    }

    siminfo->SimCalculatesSlipRatio = true;
}
