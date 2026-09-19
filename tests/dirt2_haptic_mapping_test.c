#include <math.h>
#include <stdio.h>
#include <string.h>

#include "../src/monocoque/simulatorapi/dr2_haptic_telemetry.h"
#include "../src/monocoque/simulatorapi/simapi/simapi/simapi.h"
#include "../src/monocoque/simulatorapi/simapi/simapi/simdata.h"

#define DR2_TEST_EPS 0.0001
#define DR2_WHEEL_COUNT 4
#define DR2_AXIS_X 0
#define DR2_AXIS_Y 1
#define DR2_AXIS_Z 2
#define DR2_IDENTITY_FORWARD_MS 10.0
#define DR2_LOCKED_WHEEL_MS 0.0
#define DR2_SPIN_WHEEL_MS 20.0
#define DR2_GFORCE_SENTINEL 5.0

static int failures = 0;

static void check_close(const char* name, double got, double expected)
{
    if (fabs(got - expected) > DR2_TEST_EPS)
    {
        fprintf(stderr, "FAIL: %s = %f, expected %f\n", name, got, expected);
        failures++;
    }
}

static void fill_identity_basis(SimData* simdata)
{
    simdata->tyrecontact0[DR2_AXIS_X] = 1.0;
    simdata->tyrecontact0[DR2_AXIS_Y] = 0.0;
    simdata->tyrecontact0[DR2_AXIS_Z] = 0.0;
    simdata->tyrecontact1[DR2_AXIS_X] = 0.0;
    simdata->tyrecontact1[DR2_AXIS_Y] = 1.0;
    simdata->tyrecontact1[DR2_AXIS_Z] = 0.0;
    simdata->tyrecontact2[DR2_AXIS_X] = 0.0;
    simdata->tyrecontact2[DR2_AXIS_Y] = 0.0;
    simdata->tyrecontact2[DR2_AXIS_Z] = 1.0;
}

static void fill_forward_drive(SimData* simdata, double wheel_speed_ms)
{
    int wheel;

    memset(simdata, 0, sizeof(*simdata));
    simdata->simapi = SIMULATORAPI_DIRT_RALLY_2;
    fill_identity_basis(simdata);
    simdata->worldXvelocity = 0.0;
    simdata->worldYvelocity = 0.0;
    simdata->worldZvelocity = DR2_IDENTITY_FORWARD_MS;
    simdata->Xvelocity = DR2_GFORCE_SENTINEL;
    simdata->Yvelocity = DR2_GFORCE_SENTINEL;
    for (wheel = 0; wheel < DR2_WHEEL_COUNT; wheel++)
    {
        simdata->tyreRPS[wheel] = wheel_speed_ms;
    }
}

int main(void)
{
    SimData simdata;
    int wheel;

    fill_forward_drive(&simdata, DR2_IDENTITY_FORWARD_MS);
    dr2_apply_haptic_telemetry(&simdata);
    check_close("Xvelocity", simdata.Xvelocity, 0.0);
    check_close("Yvelocity", simdata.Yvelocity, DR2_IDENTITY_FORWARD_MS);
    check_close("Zvelocity", simdata.Zvelocity, 0.0);
    for (wheel = 0; wheel < DR2_WHEEL_COUNT; wheel++)
    {
        check_close("rolling slip", simdata.tyreslipratio[wheel], 0.0);
    }

    fill_forward_drive(&simdata, DR2_LOCKED_WHEEL_MS);
    dr2_apply_haptic_telemetry(&simdata);
    for (wheel = 0; wheel < DR2_WHEEL_COUNT; wheel++)
    {
        check_close("lockup slip", simdata.tyreslipratio[wheel], 1.0);
    }

    fill_forward_drive(&simdata, DR2_SPIN_WHEEL_MS);
    dr2_apply_haptic_telemetry(&simdata);
    for (wheel = 0; wheel < DR2_WHEEL_COUNT; wheel++)
    {
        check_close("spin slip", simdata.tyreslipratio[wheel], -1.0);
    }

    memset(&simdata, 0, sizeof(simdata));
    simdata.simapi = SIMULATORAPI_DIRT_RALLY_2;
    dr2_apply_haptic_telemetry(&simdata);
    for (wheel = 0; wheel < DR2_WHEEL_COUNT; wheel++)
    {
        check_close("stopped slip", simdata.tyreslipratio[wheel], 0.0);
    }

    memset(&simdata, 0, sizeof(simdata));
    simdata.simapi = SIMULATORAPI_ASSETTO_CORSA;
    simdata.Xvelocity = DR2_GFORCE_SENTINEL;
    dr2_apply_haptic_telemetry(&simdata);
    check_close("non-DR2 Xvelocity", simdata.Xvelocity, DR2_GFORCE_SENTINEL);

    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    return 0;
}
