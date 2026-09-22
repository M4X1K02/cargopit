#include <math.h>
#include <stdatomic.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "../src/cargopit/gameloop/devicerunner.h"

#define RUN_SECONDS            1.0
#define US_PER_SECOND          1000000
#define NS_PER_SECOND          1000000000.0
#define HEALTHY_FPS            60
#define ODD_FPS                144
#define BLOCKED_FPS            60
#define BLOCKED_UPDATE_US      400000
#define RATE_TOLERANCE         0.10
#define SNAPSHOT_RPMS          4321
#define MAX_STOP_SECONDS       0.1

typedef struct
{
    atomic_int updates;
    atomic_uint last_rpms;
    useconds_t block_us;
}
FakeDevice;

static int failures;

static void fail(const char* msg)
{
    fprintf(stderr, "FAIL: %s\n", msg);
    failures++;
}

static int fake_update(SimDevice* device, SimData* simdata)
{
    FakeDevice* fake = device->derived;
    atomic_store(&fake->last_rpms, simdata->rpms);
    atomic_fetch_add(&fake->updates, 1);
    if (fake->block_us > 0)
    {
        usleep(fake->block_us);
    }
    return 0;
}

static void init_fake(SimDevice* device, FakeDevice* fake, useconds_t block_us)
{
    memset(device, 0, sizeof(*device));
    memset(fake, 0, sizeof(*fake));
    fake->block_us = block_us;
    device->derived = fake;
    device->update = fake_update;
    device->initialized = true;
}

static double now_seconds(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec / NS_PER_SECOND;
}

static void check_rate(const char* name, int updates, int fps)
{
    double expected = fps * RUN_SECONDS;
    if (fabs(updates - expected) / expected > RATE_TOLERANCE)
    {
        fprintf(stderr, "%s: %d updates, expected about %.0f\n", name, updates, expected);
        fail("device update rate must match its configured fps");
    }
}

/* 1000/144 truncates to a 6 ms timer (166 Hz); absolute deadlines must hold 144 Hz. */
static void test_rate_is_exact_for_non_divisor_fps(TelemetrySnapshot* snapshot)
{
    SimDevice device;
    FakeDevice fake;
    DeviceRunner runner;

    init_fake(&device, &fake, 0);
    memset(&runner, 0, sizeof(runner));
    device_runner_start(&runner, &device, snapshot, ODD_FPS);
    usleep((useconds_t) (RUN_SECONDS * US_PER_SECOND));
    device_runner_stop(&runner);
    check_rate("144 fps device", atomic_load(&fake.updates), ODD_FPS);
}

/* A device stuck in slow I/O must not slow down any other device. */
static void test_blocked_device_does_not_stall_others(TelemetrySnapshot* snapshot)
{
    SimDevice healthy_device;
    SimDevice blocked_device;
    FakeDevice healthy;
    FakeDevice blocked;
    DeviceRunner healthy_runner;
    DeviceRunner blocked_runner;

    init_fake(&healthy_device, &healthy, 0);
    init_fake(&blocked_device, &blocked, BLOCKED_UPDATE_US);
    memset(&healthy_runner, 0, sizeof(healthy_runner));
    memset(&blocked_runner, 0, sizeof(blocked_runner));
    device_runner_start(&blocked_runner, &blocked_device, snapshot, BLOCKED_FPS);
    device_runner_start(&healthy_runner, &healthy_device, snapshot, HEALTHY_FPS);
    usleep((useconds_t) (RUN_SECONDS * US_PER_SECOND));

    double stop_started = now_seconds();
    device_runner_stop(&healthy_runner);
    if (now_seconds() - stop_started > MAX_STOP_SECONDS)
    {
        fail("stopping a healthy runner must not wait on other devices");
    }
    device_runner_stop(&blocked_runner);

    check_rate("healthy device next to blocked one", atomic_load(&healthy.updates), HEALTHY_FPS);
    if (atomic_load(&blocked_runner.overruns) == 0)
    {
        fail("blocked device must report overruns");
    }
}

static void test_runner_reads_published_snapshot(TelemetrySnapshot* snapshot)
{
    SimDevice device;
    FakeDevice fake;
    DeviceRunner runner;
    SimData simdata;

    memset(&simdata, 0, sizeof(simdata));
    simdata.rpms = SNAPSHOT_RPMS;
    telemetry_snapshot_publish(snapshot, &simdata);

    init_fake(&device, &fake, 0);
    memset(&runner, 0, sizeof(runner));
    device_runner_start(&runner, &device, snapshot, HEALTHY_FPS);
    usleep(US_PER_SECOND / HEALTHY_FPS * 3);
    device_runner_stop(&runner);
    if (atomic_load(&fake.last_rpms) != SNAPSHOT_RPMS)
    {
        fail("device must see the latest published telemetry");
    }
}

int main(void)
{
    TelemetrySnapshot snapshot;

    if (telemetry_snapshot_init(&snapshot) != 0)
    {
        fail("snapshot init");
        return 1;
    }
    test_rate_is_exact_for_non_divisor_fps(&snapshot);
    test_blocked_device_does_not_stall_others(&snapshot);
    test_runner_reads_published_snapshot(&snapshot);
    telemetry_snapshot_destroy(&snapshot);

    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    return 0;
}
