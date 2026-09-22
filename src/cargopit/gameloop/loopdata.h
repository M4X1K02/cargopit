#ifndef _LOOPDATA_H
#define _LOOPDATA_H

#include <uv.h>
#include "../helper/parameters.h"
#include "../helper/confighelper.h"
#include "../devices/simdevice.h"
#include "../simulatorapi/simapi/simapi/simdata.h"
#include "../simulatorapi/simapi/simapi/simmapper.h"
#include "devicerunner.h"
#include "control.h"

/* Ordered: each user quit request steps one state down. */
typedef enum
{
    APPSTATE_EXITING   = 0,
    APPSTATE_SEARCHING = 1,
    APPSTATE_MAPPING   = 2
}
AppState;

typedef struct device_loop_data
{
    SimData* simdata;
    SimDevice* simdevice;
} device_loop_data;

/* Everything one play session owns; every libuv handle below runs on `loop`. */
typedef struct loop_data
{
    uv_loop_t* loop;
    AppState state;
    bool devices_pending;
    bool use_udp;
    bool releasing;
    /* Set by the quit key: stay idle after release so the next quit exits instead of re-mapping. */
    bool user_stopped;
    bool started_tyre_calc;
    bool signals_started;
    int numdevices;
    int config_index;
    SimInfo siminfo;
    // cargopit settings is a pointer from cargopit.c and freed there
    CargopitSettings* ms;
    SimData* simdata;
    SimMap* simmap;
    // allocated when devices load; ownership moves to the release job in releaseloop
    SimDevice* simdevices;
    DeviceRunner* runners;
    TelemetrySnapshot snapshot;

    uv_timer_t datachecktimer;
    uv_timer_t datamaptimer;
    uv_timer_t tyrediametertimer;
    uv_udp_t recv_socket;
    uv_async_t stop_async;
    uv_signal_t sigterm;
    uv_signal_t sigint;
    uv_poll_t* stdin_poll;
    ControlServer control;
} loop_data;


typedef struct test_loop_args
{
    CargopitSettings* ms;
    DeviceSettings* ds;
    SimData* simdata;
    int devicenum;
    int confignum;
} test_loop_args;

#endif
