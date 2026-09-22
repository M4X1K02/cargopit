#include <math.h>
#include <stdio.h>
#include <string.h>

#include "../src/cargopit/helper/confighelper.h"
#include "../src/cargopit/devices/hapticeffect.h"
#include "../src/cargopit/simulatorapi/simapi/simapi/simdata.h"

#define REFERENCE_HZ          60.0
#define FAST_HZ               240.0
#define SIM_SECONDS           1.0
#define ABS_THRESHOLD         0.30
#define ABS_SLIP_LOW          0.40
#define ABS_SLIP_HIGH         0.72
#define TEST_BRAKE            0.85
#define TEST_YVELOCITY        40.0
#define TEST_VELOCITY_KMH     150
#define PUMP_RATE_TOLERANCE   0.25
#define SUSP_FROZEN_SECONDS   0.2
#define SUSP_TIME_TOLERANCE   0.05
#define SUSP_THRESHOLD        0.1
#define SUSP_VELOCITY         3.0

static int failures;

static void fail(const char* msg)
{
    fprintf(stderr, "FAIL: %s\n", msg);
    failures++;
}

static void init_effect(HapticEffect* h, VibrationEffectType type, double threshold)
{
    memset(h, 0, sizeof(*h));
    h->effecttype = type;
    h->tyre = ALLFOUR;
    h->threshold = threshold;
}

static void set_braking(SimData* simdata, double slip)
{
    int i;
    memset(simdata, 0, sizeof(*simdata));
    simdata->brake = TEST_BRAKE;
    simdata->Yvelocity = TEST_YVELOCITY;
    simdata->velocity = TEST_VELOCITY_KMH;
    for (i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        simdata->tyreslipratio[i] = slip;
    }
}

static double abs_slip_at_frame(int frame)
{
    return (frame % 2 == 0) ? ABS_SLIP_LOW : ABS_SLIP_HIGH;
}

/* A device that sees steady lock must stay quiet even while another device sees ABS pumping. */
static void test_devices_do_not_share_abs_state(void)
{
    HapticEffect pumping;
    HapticEffect steady;
    SimData pumping_data;
    SimData steady_data;
    double dt = 1.0 / REFERENCE_HZ;
    double steady_play = 0.0;
    double pumping_play = 0.0;
    int frame;

    init_effect(&pumping, EFFECT_ABSBRAKES, ABS_THRESHOLD);
    init_effect(&steady, EFFECT_ABSBRAKES, ABS_THRESHOLD);
    for (frame = 0; frame < (int) REFERENCE_HZ; frame++)
    {
        set_braking(&pumping_data, abs_slip_at_frame(frame));
        set_braking(&steady_data, ABS_SLIP_HIGH);
        pumping_play = haptic_effect_play(&pumping_data, &pumping, dt);
        steady_play += haptic_effect_play(&steady_data, &steady, dt);
    }
    if (pumping_play <= 0.0)
    {
        fail("ABS pumping device must play");
    }
    if (steady_play != 0.0)
    {
        fail("steady-lock device must not pick up another device's ABS pumping");
    }
}

/* Telemetry changes at 60 Hz; the device ticks at `device_hz` and resamples the latest frame. */
static double abs_pump_after_one_second(double device_hz)
{
    HapticEffect h;
    SimData simdata;
    double dt = 1.0 / device_hz;
    int ticks = (int) (SIM_SECONDS * device_hz);
    int tick;

    init_effect(&h, EFFECT_ABSBRAKES, ABS_THRESHOLD);
    for (tick = 0; tick < ticks; tick++)
    {
        int frame = (int) (tick * REFERENCE_HZ / device_hz);
        set_braking(&simdata, abs_slip_at_frame(frame));
        haptic_effect_play(&simdata, &h, dt);
    }
    return h.filter.abs_pump_ema;
}

static void test_abs_pump_is_rate_independent(void)
{
    double reference = abs_pump_after_one_second(REFERENCE_HZ);
    double fast = abs_pump_after_one_second(FAST_HZ);

    if (reference <= 0.0)
    {
        fail("reference ABS pump level must be positive");
        return;
    }
    if (fabs(fast - reference) / reference > PUMP_RATE_TOLERANCE)
    {
        fprintf(stderr, "reference pump %f, fast pump %f\n", reference, fast);
        fail("ABS pump level must not depend on device tick rate");
    }
}

static double seconds_until_suspension_frozen(double device_hz)
{
    HapticEffect h;
    SimData simdata;
    double dt = 1.0 / device_hz;
    int tick;
    int i;

    init_effect(&h, EFFECT_SUSPENSION, SUSP_THRESHOLD);
    memset(&simdata, 0, sizeof(simdata));
    simdata.velocity = TEST_VELOCITY_KMH;
    for (i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        simdata.suspvelocity[i] = SUSP_VELOCITY;
    }
    for (tick = 1; tick <= (int) (SIM_SECONDS * device_hz); tick++)
    {
        haptic_effect_play(&simdata, &h, dt);
        if (h.filter.susp_frozen_seconds >= SUSP_FROZEN_SECONDS)
        {
            return tick * dt;
        }
    }
    return SIM_SECONDS;
}

static void test_suspension_freeze_detection_is_time_based(void)
{
    double reference = seconds_until_suspension_frozen(REFERENCE_HZ);
    double fast = seconds_until_suspension_frozen(FAST_HZ);

    if (fabs(reference - fast) > SUSP_TIME_TOLERANCE)
    {
        fprintf(stderr, "frozen after %f s at %.0f Hz, %f s at %.0f Hz\n", reference, REFERENCE_HZ, fast, FAST_HZ);
        fail("suspension freeze detection must take the same time at any tick rate");
    }
}

int main(void)
{
    test_devices_do_not_share_abs_state();
    test_abs_pump_is_rate_independent();
    test_suspension_freeze_detection_is_time_based();

    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    return 0;
}
