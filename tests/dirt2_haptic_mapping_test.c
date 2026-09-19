#include <math.h>
#include <stdio.h>
#include <string.h>

#include "../src/cargopit/simulatorapi/dr2_haptic_telemetry.h"
#include "../src/cargopit/simulatorapi/simapi/simapi/simapi.h"
#include "../src/cargopit/simulatorapi/simapi/simapi/simdata.h"

#define DR2_TEST_EPS 0.0001
#define DR2_WHEEL_COUNT 4
#define DR2_AXIS_X 0
#define DR2_AXIS_Y 1
#define DR2_AXIS_Z 2
#define DR2_IDENTITY_FORWARD_MS 10.0
#define DR2_LOCKED_WHEEL_MS 0.0
#define DR2_SPIN_WHEEL_MS 20.0
#define DR2_GFORCE_SENTINEL 5.0
#define DR2_VERTICAL_MS 3.0
#define DR2_ZVELOCITY_SENTINEL 99.0
#define DR2_PACKED_RL 0
#define DR2_PACKED_RR 1
#define DR2_PACKED_FL 2
#define DR2_PACKED_FR 3
#define DR2_WHEEL_FL 0
#define DR2_WHEEL_FR 1
#define DR2_WHEEL_RL 2
#define DR2_WHEEL_RR 3
#define DR2_SUSP_PACKED_RL 1.0
#define DR2_SUSP_PACKED_RR 2.0
#define DR2_SUSP_PACKED_FL 3.0
#define DR2_SUSP_PACKED_FR 4.0

static int failures = 0;

static void check_close(const char* name, double got, double expected)
{
    if (fabs(got - expected) > DR2_TEST_EPS)
    {
        fprintf(stderr, "FAIL: %s = %f, expected %f\n", name, got, expected);
        failures++;
    }
}

static void check_true(const char* name, int got)
{
    if (got == 0)
    {
        fprintf(stderr, "FAIL: %s is false\n", name);
        failures++;
    }
}

static void check_false(const char* name, int got)
{
    if (got != 0)
    {
        fprintf(stderr, "FAIL: %s is true\n", name);
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

static void fill_packed_wheel_speeds(SimData* simdata, double wheel_speed_ms)
{
    simdata->tyreRPS[DR2_PACKED_RL] = wheel_speed_ms;
    simdata->tyreRPS[DR2_PACKED_RR] = wheel_speed_ms;
    simdata->tyreRPS[DR2_PACKED_FL] = wheel_speed_ms;
    simdata->tyreRPS[DR2_PACKED_FR] = wheel_speed_ms;
}

static void fill_forward_drive(SimData* simdata, double wheel_speed_ms)
{
    memset(simdata, 0, sizeof(*simdata));
    simdata->simapi = SIMULATORAPI_DIRT_RALLY_2;
    fill_identity_basis(simdata);
    simdata->worldXvelocity = 0.0;
    simdata->worldYvelocity = 0.0;
    simdata->worldZvelocity = DR2_IDENTITY_FORWARD_MS;
    simdata->Xvelocity = DR2_GFORCE_SENTINEL;
    simdata->Yvelocity = DR2_GFORCE_SENTINEL;
    fill_packed_wheel_speeds(simdata, wheel_speed_ms);
}

static void test_local_velocity_and_uniform_slip(void)
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
}

static void test_does_not_write_vertical_into_zvelocity(void)
{
    SimData simdata;

    fill_forward_drive(&simdata, DR2_IDENTITY_FORWARD_MS);
    simdata.worldYvelocity = DR2_VERTICAL_MS;
    simdata.Zvelocity = DR2_ZVELOCITY_SENTINEL;
    dr2_apply_haptic_telemetry(&simdata);
    check_close("Zvelocity sentinel", simdata.Zvelocity, DR2_ZVELOCITY_SENTINEL);
}

static void test_unshuffles_simapi_wheel_order(void)
{
    SimData simdata;

    fill_forward_drive(&simdata, DR2_IDENTITY_FORWARD_MS);
    simdata.tyreRPS[DR2_PACKED_RL] = DR2_IDENTITY_FORWARD_MS;
    simdata.tyreRPS[DR2_PACKED_RR] = DR2_IDENTITY_FORWARD_MS;
    simdata.tyreRPS[DR2_PACKED_FL] = DR2_LOCKED_WHEEL_MS;
    simdata.tyreRPS[DR2_PACKED_FR] = DR2_IDENTITY_FORWARD_MS;
    simdata.suspvelocity[DR2_PACKED_RL] = DR2_SUSP_PACKED_RL;
    simdata.suspvelocity[DR2_PACKED_RR] = DR2_SUSP_PACKED_RR;
    simdata.suspvelocity[DR2_PACKED_FL] = DR2_SUSP_PACKED_FL;
    simdata.suspvelocity[DR2_PACKED_FR] = DR2_SUSP_PACKED_FR;

    dr2_apply_haptic_telemetry(&simdata);

    check_close("FL lockup slip", simdata.tyreslipratio[DR2_WHEEL_FL], 1.0);
    check_close("FR rolling slip", simdata.tyreslipratio[DR2_WHEEL_FR], 0.0);
    check_close("RL rolling slip", simdata.tyreslipratio[DR2_WHEEL_RL], 0.0);
    check_close("RR rolling slip", simdata.tyreslipratio[DR2_WHEEL_RR], 0.0);
    check_close("FL suspvelocity", simdata.suspvelocity[DR2_WHEEL_FL], DR2_SUSP_PACKED_FL);
    check_close("FR suspvelocity", simdata.suspvelocity[DR2_WHEEL_FR], DR2_SUSP_PACKED_FR);
    check_close("RL suspvelocity", simdata.suspvelocity[DR2_WHEEL_RL], DR2_SUSP_PACKED_RL);
    check_close("RR suspvelocity", simdata.suspvelocity[DR2_WHEEL_RR], DR2_SUSP_PACKED_RR);
}

static void test_stopped_and_non_dr2(void)
{
    SimData simdata;
    int wheel;

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
}

static void test_siminfo_marks_slip_as_provided(void)
{
    SimInfo siminfo;

    memset(&siminfo, 0, sizeof(siminfo));
    siminfo.simulatorapi = SIMULATORAPI_DIRT_RALLY_2;
    dr2_apply_haptic_siminfo(&siminfo);
    check_true("DR2 SimCalculatesSlipRatio", siminfo.SimCalculatesSlipRatio);

    memset(&siminfo, 0, sizeof(siminfo));
    siminfo.mapapi = SIMULATORAPI_DIRT_RALLY_2;
    dr2_apply_haptic_siminfo(&siminfo);
    check_true("DR2 mapapi SimCalculatesSlipRatio", siminfo.SimCalculatesSlipRatio);

    memset(&siminfo, 0, sizeof(siminfo));
    siminfo.simulatorapi = SIMULATORAPI_ASSETTO_CORSA;
    dr2_apply_haptic_siminfo(&siminfo);
    check_false("non-DR2 SimCalculatesSlipRatio", siminfo.SimCalculatesSlipRatio);
}

int main(void)
{
    test_local_velocity_and_uniform_slip();
    test_does_not_write_vertical_into_zvelocity();
    test_unshuffles_simapi_wheel_order();
    test_stopped_and_non_dr2();
    test_siminfo_marks_slip_as_provided();

    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    return 0;
}
