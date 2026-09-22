#include <stdio.h>
#include <string.h>

#include "../src/cargopit/gameloop/gameloop.h"
#include "../src/cargopit/simulatorapi/simapi/simapi/simdata.h"

#define TEST_PEDAL_MIN 0.2
#define TEST_SLIP_SPIN_MAX (-0.2)
#define TEST_SLIP_LOCK_MIN 0.2
#define TEST_BRAKE_TEMP_MIN 0.5
#define TEST_WHEEL_COUNT 4

static int failures;

static void fail(const char* msg)
{
    fprintf(stderr, "FAIL: %s\n", msg);
    failures++;
}

static void check_spin(const SimData* simdata)
{
    int i;
    if (simdata->gas <= TEST_PEDAL_MIN)
    {
        fail("wheel spin must apply throttle");
    }
    for (i = 0; i < TEST_WHEEL_COUNT; i++)
    {
        if (simdata->tyreslipratio[i] >= TEST_SLIP_SPIN_MAX)
        {
            fail("wheel spin must set negative slip");
            return;
        }
    }
}

static void check_lock(const SimData* simdata)
{
    int i;
    if (simdata->brake <= TEST_PEDAL_MIN)
    {
        fail("wheel lock must apply brake");
    }
    for (i = 0; i < TEST_WHEEL_COUNT; i++)
    {
        if (simdata->tyreslipratio[i] <= TEST_SLIP_LOCK_MIN)
        {
            fail("wheel lock must set positive slip");
            return;
        }
        if (simdata->braketemp[i] <= TEST_BRAKE_TEMP_MIN)
        {
            fail("wheel lock must heat brake-button temps");
            return;
        }
    }
}

int main(void)
{
    SimData simdata;

    memset(&simdata, 0, sizeof(simdata));
    set_basic_simdata(&simdata);
    if (simdata.idlerpm == 0 || simdata.maxrpm <= simdata.idlerpm)
    {
        fail("basic test data must set idle and redline");
    }
    if (simdata.Yvelocity <= 0)
    {
        fail("basic test data must set longitudinal velocity");
    }

    set_wheel_spin_simdata(&simdata);
    check_spin(&simdata);

    set_wheel_lock_simdata(&simdata);
    check_lock(&simdata);

    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    return 0;
}
