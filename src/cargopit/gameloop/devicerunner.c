#include <string.h>
#include <time.h>

#include "devicerunner.h"
#include "../helper/confighelper.h"

#define NS_PER_SECOND 1000000000ULL

int telemetry_snapshot_init(TelemetrySnapshot* snapshot)
{
    memset(&snapshot->frame, 0, sizeof(snapshot->frame));
    snapshot->sequence = 0;
    return pthread_mutex_init(&snapshot->lock, NULL);
}

void telemetry_snapshot_destroy(TelemetrySnapshot* snapshot)
{
    pthread_mutex_destroy(&snapshot->lock);
}

void telemetry_snapshot_publish(TelemetrySnapshot* snapshot, const SimData* simdata)
{
    pthread_mutex_lock(&snapshot->lock);
    memcpy(&snapshot->frame, simdata, sizeof(snapshot->frame));
    snapshot->sequence++;
    pthread_mutex_unlock(&snapshot->lock);
}

void telemetry_snapshot_read(TelemetrySnapshot* snapshot, SimData* out)
{
    pthread_mutex_lock(&snapshot->lock);
    memcpy(out, &snapshot->frame, sizeof(*out));
    pthread_mutex_unlock(&snapshot->lock);
}

static uint64_t timespec_to_ns(const struct timespec* ts)
{
    return (uint64_t) ts->tv_sec * NS_PER_SECOND + (uint64_t) ts->tv_nsec;
}

static struct timespec ns_to_timespec(uint64_t ns)
{
    struct timespec ts;
    ts.tv_sec = (time_t) (ns / NS_PER_SECOND);
    ts.tv_nsec = (long) (ns % NS_PER_SECOND);
    return ts;
}

static uint64_t monotonic_now_ns(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return timespec_to_ns(&now);
}

/* Absolute deadlines keep the average rate exact; after an overrun, resync instead of bursting to catch up. */
static uint64_t next_deadline(DeviceRunner* runner, uint64_t deadline_ns)
{
    uint64_t next = deadline_ns + runner->period_ns;
    uint64_t now = monotonic_now_ns();

    if (next >= now)
    {
        return next;
    }
    atomic_fetch_add(&runner->overruns, 1);
    return now;
}

static void* device_runner_main(void* arg)
{
    DeviceRunner* runner = arg;
    uint64_t deadline_ns = monotonic_now_ns();

    while (atomic_load(&runner->running))
    {
        telemetry_snapshot_read(runner->snapshot, &runner->frame);
        runner->device->update(runner->device, &runner->frame);
        atomic_fetch_add(&runner->updates, 1);

        deadline_ns = next_deadline(runner, deadline_ns);
        struct timespec deadline = ns_to_timespec(deadline_ns);
        clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &deadline, NULL);
    }
    return NULL;
}

int device_runner_start(DeviceRunner* runner, SimDevice* device, TelemetrySnapshot* snapshot, int fps)
{
    if (runner == NULL || device == NULL || snapshot == NULL)
    {
        return -1;
    }
    runner->device = device;
    runner->snapshot = snapshot;
    runner->period_ns = NS_PER_SECOND / (uint64_t) cargopit_clamp_fps(fps);
    atomic_init(&runner->updates, 0);
    atomic_init(&runner->overruns, 0);
    atomic_init(&runner->running, true);
    runner->started = pthread_create(&runner->thread, NULL, device_runner_main, runner) == 0;
    return runner->started ? 0 : -1;
}

void device_runner_stop(DeviceRunner* runner)
{
    if (runner == NULL || !runner->started)
    {
        return;
    }
    atomic_store(&runner->running, false);
    pthread_join(runner->thread, NULL);
    runner->started = false;
}
