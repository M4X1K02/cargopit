#ifndef _DEVICERUNNER_H
#define _DEVICERUNNER_H

#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>

#include "../devices/simdevice.h"
#include "../simulatorapi/simapi/simapi/simdata.h"

/* Latest telemetry frame: the loop thread publishes, device runners copy it out. */
typedef struct
{
    pthread_mutex_t lock;
    SimData frame;
    uint64_t sequence;
}
TelemetrySnapshot;

int telemetry_snapshot_init(TelemetrySnapshot* snapshot);
void telemetry_snapshot_destroy(TelemetrySnapshot* snapshot);
void telemetry_snapshot_publish(TelemetrySnapshot* snapshot, const SimData* simdata);
void telemetry_snapshot_read(TelemetrySnapshot* snapshot, SimData* out);

/*
 * Drives one device on its own thread at a fixed rate. Blocking device I/O
 * (serial timeouts, hid_write) only delays that device, never the loop or
 * other devices.
 */
typedef struct
{
    SimDevice* device;
    TelemetrySnapshot* snapshot;
    uint64_t period_ns;
    atomic_bool running;
    atomic_uint_fast64_t updates;
    atomic_uint_fast64_t overruns;
    bool started;
    pthread_t thread;
    SimData frame;
}
DeviceRunner;

int device_runner_start(DeviceRunner* runner, SimDevice* device, TelemetrySnapshot* snapshot, int fps);
/* Stops and joins the thread; waits for an in-flight update to return. */
void device_runner_stop(DeviceRunner* runner);

#endif
