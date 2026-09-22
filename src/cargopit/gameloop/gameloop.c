#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <termios.h>
#include <signal.h>
#include <uv.h>

#include "gameloop.h"
#include "loopdata.h"
#include "acr_udp.h"
#include "../helper/simd_telemetry.h"
#include "../helper/confighelper.h"
#include "../helper/ensure_simd.h"
#include "../devices/simdevice.h"
#include "../devices/hapticeffect.h"
#include "../simulatorapi/simapi/simapi/simdata.h"
#include "../simulatorapi/simapi/simapi/simmapper.h"
#include "../simulatorapi/simapi/simapi/simmap.h"
#include "../simulatorapi/simapi/simapi/dirt2.h"
#include "../slog/slog.h"

#define SIMAPI_DAEMON_PROBE_US           50000
#define SIM_UDP_PORT_DIRT_RALLY_2_NATIVE 20777
#define SIM_CHECK_INTERVAL_MS            1000
#define SIM_MAPPING_START_DELAY_MS       2000
#define TYRE_DIAMETER_CHECK_INTERVAL_MS  1000
#define TYRE_DIAMETER_UNSET              (-1)
#define DEVICE_SPINDOWN_SECONDS          1
#define MS_PER_SECOND                    1000.0
#define UDP_BIND_ADDRESS                 "0.0.0.0"
#define QUIT_KEY                         'q'
#define SIM_NOT_DETECTED                 "None Detected"
#define SIMD_NOT_DETECTED                "Not Detected"
#define SIMD_RUNNING                     "Running"
#define CONTROL_REPLY_OK                 "{\"ok\":true}"
#define CONTROL_REPLY_UNKNOWN            "{\"ok\":false,\"error\":\"unknown command\"}"
#define CONTROL_STATUS_MAX               512
#define CONTROL_NAME_MAX                 128
#define JSON_TRUE                        "true"
#define JSON_FALSE                       "false"

static const char* const APP_STATE_NAMES[] =
{
    [APPSTATE_EXITING]   = "exiting",
    [APPSTATE_SEARCHING] = "searching",
    [APPSTATE_MAPPING]   = "mapping",
};

/* One play session per process; libuv callbacks reach it through their handle data. */
static loop_data session;
static pthread_t loop_thread;

void datacheckcallback(uv_timer_t* handle);
void shmdatamapcallback(uv_timer_t* handle);
void tyrediametercheckcallback(uv_timer_t* handle);
static void releaseloop(loop_data* f);
int startudp(int port);

static int require_simd(void)
{
    SimdEnsureStatus simd_status = ensure_simd();
    if (simd_status == SIMD_OK)
    {
        return 0;
    }
    return (simd_status == SIMD_NOT_INSTALLED)
           ? CARGOPIT_ERROR_SIMD_REQUIRED
           : CARGOPIT_ERROR_UNKNOWN;
}

static void restore_stdin_terminal(const struct termios* canonicalmode, int stdin_was_raw)
{
    if (stdin_was_raw == 0)
    {
        return;
    }
    tcsetattr(STDIN_FILENO, TCSANOW, canonicalmode);
}

static void map_live_simdata(SimData* simdata, SimMap* simmap,
                             SimulatorAPI api, bool udp, char* packet)
{
    simapi_datamap(simdata, simmap, api, udp, packet);
}

static int interval_ms_for_fps(int fps)
{
    int interval = (int) (MS_PER_SECOND / cargopit_clamp_fps(fps) + 0.5);
    return interval > 0 ? interval : 1;
}

void simapilib_loginfo(char* message)
{
    slogi(message);
}

void simapilib_logdebug(char* message)
{
    slogd(message);
}

void simapilib_logtrace(char* message)
{
    slog_display(SLOG_TRACE, 1, message);
}

static void close_walk_cb(uv_handle_t* handle, void* arg)
{
    (void) arg;
    if (!uv_is_closing(handle))
    {
        uv_close(handle, NULL);
    }
}

static void stop_udp(loop_data* f)
{
    if (uv_is_active((uv_handle_t*) &f->recv_socket))
    {
        uv_udp_recv_stop(&f->recv_socket);
    }
}

/* Runs on the loop thread; the loop returns once devices are released and every handle is idle. */
static void session_request_exit(loop_data* f)
{
    if (f->state == APPSTATE_EXITING)
    {
        return;
    }
    slogi("Cargopit is exiting...");
    f->state = APPSTATE_EXITING;
    uv_timer_stop(&f->datachecktimer);
    uv_timer_stop(&f->datamaptimer);
    uv_timer_stop(&f->tyrediametertimer);
    stop_udp(f);
    if (f->stdin_poll != NULL)
    {
        uv_poll_stop(f->stdin_poll);
    }
    if (f->signals_started)
    {
        uv_signal_stop(&f->sigterm);
        uv_signal_stop(&f->sigint);
    }
    if (!uv_is_closing((uv_handle_t*) &f->stop_async))
    {
        uv_close((uv_handle_t*) &f->stop_async, NULL);
    }
    control_server_stop(&f->control);
    if (f->numdevices > 0 && !f->releasing)
    {
        releaseloop(f);
    }
}

static void on_stop_requested(uv_async_t* handle)
{
    session_request_exit((loop_data*) handle->data);
}

static void stop_mainloop_from_signal(uv_signal_t* handle, int signum)
{
    slogi("signal %i received, stopping", signum);
    session_request_exit((loop_data*) handle->data);
}

static void on_stdin_key(uv_poll_t* handle, int status, int events)
{
    loop_data* f = (loop_data*) handle->data;
    char ch;

    (void) status;
    (void) events;
    if (read(STDIN_FILENO, &ch, sizeof(ch)) != sizeof(ch) || ch != QUIT_KEY)
    {
        return;
    }
    if (f->releasing)
    {
        return;
    }
    if (f->state == APPSTATE_MAPPING)
    {
        fprintf(stdout, "\nUser requested stop, releasing devices\n");
        fflush(stdout);
        slogi("User requested stop, releasing devices");
        f->user_stopped = true;
        releaseloop(f);
        return;
    }
    session_request_exit(f);
}

static uv_poll_t* init_stdin_quit_poll(uv_loop_t* loop, struct termios* canonicalmode, int* stdin_was_raw)
{
    *stdin_was_raw = 0;
    if (!isatty(STDIN_FILENO))
    {
        slogd("stdin is not a tty; skip quit-key poll");
        return NULL;
    }

    struct termios newsettings;
    if (tcgetattr(STDIN_FILENO, canonicalmode) != 0)
    {
        slogw("could not read stdin terminal settings");
        return NULL;
    }
    newsettings = *canonicalmode;
    newsettings.c_lflag &= (tcflag_t)(~ICANON);
    newsettings.c_lflag &= (tcflag_t)(~ECHO);
    newsettings.c_cc[VMIN] = 1;
    newsettings.c_cc[VTIME] = 0;
    if (tcsetattr(STDIN_FILENO, TCSANOW, &newsettings) != 0)
    {
        slogw("could not set stdin to raw mode");
        return NULL;
    }
    *stdin_was_raw = 1;

    uv_poll_t* poll = malloc(uv_handle_size(UV_POLL));
    if (poll == NULL)
    {
        restore_stdin_terminal(canonicalmode, *stdin_was_raw);
        *stdin_was_raw = 0;
        return NULL;
    }
    if (uv_poll_init(loop, poll, STDIN_FILENO) != 0)
    {
        slogw("could not poll stdin; continuing without quit key");
        free(poll);
        restore_stdin_terminal(canonicalmode, *stdin_was_raw);
        *stdin_was_raw = 0;
        return NULL;
    }
    return poll;
}

void tyrediametercheckcallback(uv_timer_t* handle)
{
    loop_data* f = (loop_data*) handle->data;
    SimData* simdata = f->simdata;

    if (simdata->car[0] != '\0')
    {
        slogi("car is %s", simdata->car);
        // check for saved tyre diameter in config file
        // if not saved version exists get tyre diameter and save it
        // use config check variable to track if the config check has been performed
        // avoid many opens of the same file
        if(hasTyreDiameter(simdata)==false && f->ms->configcheck == 0)
        {
            slogi("attempting load of tyre diameter config");
            if (loadtyreconfig(simdata, f->ms->tyre_diameter_config, true) != 0)
            {
                slogw("could not load tyre diameter config");
            }
            f->ms->configcheck = 1;
        }

        if(hasTyreDiameter(simdata)==false)
        {
            slogt("could not find tyre diameter in config file, attempting to calculate new");
            getTyreDiameter(simdata);
            // if this successfully calculates data the diameters will be saved to the file after the
            // sim stops actively mapping data
        }
    }
    if(hasTyreDiameter(simdata)==true)
    {
        int a = loadtyreconfig(simdata, f->ms->tyre_diameter_config, false);
        if(a < 0)
        {
            slogi("saving new tyre diameter config for car %s", simdata->car);
            savetyreconfig(simdata, f->ms->tyre_diameter_config);
        }
        uv_timer_stop(handle);
    }
}

static bool device_needs_tyre_diameter(const SimDevice* device)
{
    VibrationEffectType effect = device->hapticeffect.effecttype;
    return effect == EFFECT_TYRELOCK || effect == EFFECT_TYRESLIP || effect == EFFECT_ABSBRAKES;
}

static bool sim_needs_tyre_diameter(const loop_data* f)
{
    const SimInfo* siminfo = &f->siminfo;
    return siminfo->SimCalculatesTyreDiameter == false
        && siminfo->SimSupportsHapticEffects == true
        && siminfo->SimCalculatesSlipRatio == false
        && f->ms->useconfig == 1
        && f->ms->tyre_diameter_config != NULL;
}

static void maybe_start_tyre_diameter_calc(loop_data* f, const SimDevice* device)
{
    if (f->started_tyre_calc || !device_needs_tyre_diameter(device) || !sim_needs_tyre_diameter(f))
    {
        return;
    }
    slogi("Starting timer to calculate tyre diameters and save to config file");
    f->started_tyre_calc = true;
    uv_timer_start(&f->tyrediametertimer, tyrediametercheckcallback, 0, TYRE_DIAMETER_CHECK_INTERVAL_MS);
}

static void start_device_runners(loop_data* f)
{
    f->runners = calloc(f->numdevices, sizeof(DeviceRunner));
    if (f->runners == NULL)
    {
        sloge("could not allocate device runners");
        return;
    }
    for (int x = 0; x < f->numdevices; x++)
    {
        SimDevice* device = &f->simdevices[x];
        if (device->initialized == false)
        {
            continue;
        }
        if (device_runner_start(&f->runners[x], device, &f->snapshot, device->fps) != 0)
        {
            sloge("could not start thread for device %i", x);
            continue;
        }
        slogi("starting device type %i at id %i on its own thread at %i fps", device->type, x, cargopit_clamp_fps(device->fps));
        maybe_start_tyre_diameter_calc(f, device);
    }
}

static void publish_frame(loop_data* f)
{
    telemetry_snapshot_publish(&f->snapshot, f->simdata);
}

static void load_devices_if_pending(loop_data* f)
{
    CargopitSettings* ms = f->ms;
    int configureddevices;
    int confignum;

    if (f->devices_pending == false)
    {
        return;
    }
    f->devices_pending = false;
    slogi("loading device profile from %s (config-index %i)", ms->config_str, ms->config_index);
    confignum = resolve_config_index(ms->config_str, ms->config_index);
    if (confignum < 0)
    {
        sloge("no device profile to load (config-index %i)", ms->config_index);
        return;
    }

    configcheck(ms->config_str, confignum, &configureddevices);
    DeviceSettings* ds = malloc(configureddevices * sizeof(DeviceSettings));
    slogd("loading confignum %i, with %i devices.", confignum, configureddevices);
    f->numdevices = load_device_configs(ms->config_str, confignum, configureddevices, ms, ds);

    if(ms->useconfig == 1)
    {
        ms->configcheck = 0;
    }

    f->simdevices = malloc(f->numdevices * sizeof(SimDevice));
    int initdevices = devinit(f->simdevices, &f->siminfo, configureddevices, ds, ms);
    slogi("initialized %i devices", initdevices);

    for( int i = 0; i < configureddevices; i++)
    {
        settingsfree(ds[i]);
    }
    free(ds);

    f->started_tyre_calc = false;
    start_device_runners(f);
}

/* Owns a session's devices while they are stopped and freed on the libuv threadpool. */
typedef struct
{
    uv_work_t req;
    loop_data* session;
    SimDevice* devices;
    DeviceRunner* runners;
    int numdevices;
    SimData spindown;
}
ReleaseJob;

static void stop_device_runners(ReleaseJob* job)
{
    if (job->runners == NULL)
    {
        return;
    }
    for (int x = 0; x < job->numdevices; x++)
    {
        device_runner_stop(&job->runners[x]);
        slogi("device %i: %lu updates, %lu overruns", x,
              (unsigned long) atomic_load(&job->runners[x].updates),
              (unsigned long) atomic_load(&job->runners[x].overruns));
    }
}

/* Threadpool: joining runners and spin-down may block on device I/O, so it stays off the loop thread. */
static void release_devices_work(uv_work_t* req)
{
    ReleaseJob* job = req->data;
    SimDevice* devices = job->devices;

    stop_device_runners(job);
    for (int x = 0; x < job->numdevices; x++)
    {
        if (devices[x].initialized == true)
        {
            devices[x].update(&devices[x], &job->spindown);
        }
    }
    sleep(DEVICE_SPINDOWN_SECONDS);
    for (int x = 0; x < job->numdevices; x++)
    {
        if (devices[x].initialized == true)
        {
            devices[x].free(&devices[x]);
        }
    }
}

static void finish_release(loop_data* f);

static void release_devices_done(uv_work_t* req, int status)
{
    ReleaseJob* job = req->data;

    (void) status;
    free(job->runners);
    free(job->devices);
    finish_release(job->session);
    free(job);
}

static ReleaseJob* take_devices_for_release(loop_data* f)
{
    ReleaseJob* job = calloc(1, sizeof(ReleaseJob));
    if (job == NULL)
    {
        return NULL;
    }
    job->req.data = job;
    job->session = f;
    job->devices = f->simdevices;
    job->runners = f->runners;
    job->numdevices = f->numdevices;
    // help things spin down
    job->spindown = *f->simdata;
    job->spindown.simstatus = SIMAPI_STATUS_OFF;
    job->spindown.rpms = 0;
    job->spindown.velocity = 0;
    f->simdevices = NULL;
    f->runners = NULL;
    f->numdevices = 0;
    return job;
}

static void finish_release(loop_data* f)
{
    int r = simapi_sim_clear(f->simdata, f->simmap, false);
    slogd("simfree returned %i", r);
    f->releasing = false;
    if (f->state == APPSTATE_EXITING)
    {
        return;
    }
    f->state = APPSTATE_SEARCHING;
    if (f->user_stopped)
    {
        slogi("stopped mapping data, press q again to quit");
        return;
    }
    slogi("restarting checking for data...");
    uv_timer_start(&f->datachecktimer, datacheckcallback, 0, SIM_CHECK_INTERVAL_MS);
}

static void releaseloop(loop_data* f)
{
    if (f->releasing)
    {
        return;
    }
    slogi("release loop");
    f->releasing = true;
    f->devices_pending = false;
    uv_timer_stop(&f->datamaptimer);
    uv_timer_stop(&f->datachecktimer);
    uv_timer_stop(&f->tyrediametertimer);
    stop_udp(f);
    slogi("releasing devices, please wait");
    if (f->simdevices == NULL)
    {
        finish_release(f);
        return;
    }
    ReleaseJob* job = take_devices_for_release(f);
    if (job == NULL)
    {
        sloge("could not allocate device release job");
        finish_release(f);
        return;
    }
    if (uv_queue_work(f->loop, &job->req, release_devices_work, release_devices_done) != 0)
    {
        slogw("could not schedule device release; releasing on the loop thread");
        release_devices_work(&job->req);
        release_devices_done(&job->req, 0);
    }
}

static bool mapping_should_stop(const loop_data* f)
{
    return f->siminfo.isSimOn == false
        || f->simdata->simstatus <= SIMAPI_STATUS_MENU
        || f->state != APPSTATE_MAPPING;
}

void shmdatamapcallback(uv_timer_t* handle)
{
    loop_data* f = (loop_data*) handle->data;

    if (f->state == APPSTATE_MAPPING)
    {
        map_live_simdata(f->simdata, f->simmap, f->siminfo.mapapi, false, NULL);
        publish_frame(f);
        load_devices_if_pending(f);
    }
    if (mapping_should_stop(f))
    {
        releaseloop(f);
    }
}

static void on_alloc(uv_handle_t* client, size_t suggested_size, uv_buf_t* buf)
{
    (void) client;
    buf->base = calloc(1, suggested_size);
    buf->len = buf->base != NULL ? suggested_size : 0;
}

static void map_udp_packet(loop_data* f, char* packet, ssize_t nread)
{
    if (acr_udp_packet_ok(packet, (size_t) nread))
    {
        acr_udp_apply(f->simdata, packet);
        acr_udp_publish(f->simmap, f->simdata);
        return;
    }
    map_live_simdata(f->simdata, f->simmap, f->siminfo.mapapi, true, packet);
}

static void on_udp_recv(uv_udp_t* handle, ssize_t nread, const uv_buf_t* rcvbuf, const struct sockaddr* addr, unsigned flags)
{
    loop_data* f = (loop_data*) handle->data;

    (void) addr;
    (void) flags;
    if (nread <= 0)
    {
        free(rcvbuf->base);
        return;
    }
    slogt("udp data received");
    if (f->state == APPSTATE_MAPPING)
    {
        map_udp_packet(f, rcvbuf->base, nread);
        publish_frame(f);
        load_devices_if_pending(f);
    }
    if (mapping_should_stop(f))
    {
        releaseloop(f);
    }
    free(rcvbuf->base);
}

static void close_simapi_map(SimMap* simmap)
{
    if (simmap == NULL)
    {
        return;
    }
    if (simmap->addr != NULL)
    {
        munmap(simmap->addr, sizeof(SimData));
        simmap->addr = NULL;
    }
    if (simmap->fd != -1)
    {
        close(simmap->fd);
        simmap->fd = -1;
    }
    simmap->hasSimApiDat = false;
}

static bool simapi_daemon_advancing(SimData* simdata, SimMap* simmap)
{
    uint64_t first_tick;

    if (simdata == NULL || simmap == NULL || simmap->addr == NULL)
    {
        return false;
    }
    if (simdata->simon == false)
    {
        return false;
    }
    first_tick = simdata->mtick;
    usleep(SIMAPI_DAEMON_PROBE_US);
    simapi_datamap(simdata, simmap, SIMULATORAPI_SIMAPI_TEST, false, NULL);
    return simdata->mtick != first_tick;
}

static SimulatorEXE discovered_sim_exe(loop_data* f)
{
    SimInfo exe_info;

    if (f->siminfo.simulatorexe != SIMULATOREXE_SIMAPI_TEST_NONE)
    {
        return f->siminfo.simulatorexe;
    }
    memset(&exe_info, 0, sizeof(exe_info));
    return simapi_get_sim_exe(&exe_info);
}

static bool try_acr_udp_bridge(loop_data* f, SimData* simdata, SimMap* simmap)
{
    SimulatorEXE exe = discovered_sim_exe(f);
    TelemetrySource source;

    if (exe != SIMULATOREXE_ASSETTO_CORSA_RALLY)
    {
        return false;
    }
    source = simd_telemetry_source(exe);
    if (source == TELEMETRY_SOURCE_SHM)
    {
        return false;
    }
    if (source != TELEMETRY_SOURCE_UDP && !ac_physics_shm_is_blank())
    {
        return false;
    }
    if (startudp(SIM_UDP_PORT_ASSETTO_CORSA_RALLY) != 0)
    {
        sloge("could not bind Assetto Corsa Rally UDP port %i",
              SIM_UDP_PORT_ASSETTO_CORSA_RALLY);
        return false;
    }
    if (source == TELEMETRY_SOURCE_UDP)
    {
        slogi("Assetto Corsa Rally telemetry=udp; binding Proton UDP on %i",
              SIM_UDP_PORT_ASSETTO_CORSA_RALLY);
    }
    else
    {
        slogi("Assetto Corsa Rally Linux SHM is blank; using Proton UDP on %i",
              SIM_UDP_PORT_ASSETTO_CORSA_RALLY);
    }
    close_simapi_map(simmap);
    f->use_udp = true;
    f->siminfo.SimUsesUDP = true;
    f->siminfo.isSimOn = true;
    f->siminfo.simulatorapi = SIMULATORAPI_ASSETTO_CORSA;
    f->siminfo.mapapi = SIMULATORAPI_ASSETTO_CORSA;
    f->siminfo.simulatorexe = exe;
    simdata->simon = true;
    simdata->simapi = SIMULATORAPI_ASSETTO_CORSA;
    simdata->simexe = exe;
    simdata->simstatus = SIMAPI_STATUS_ACTIVEPLAY;
    return true;
}

static void apply_simd_telemetry_preference(loop_data* f)
{
    SimInfo probe;
    SimulatorEXE exe;
    TelemetrySource source;

    memset(&probe, 0, sizeof(probe));
    exe = simapi_get_sim_exe(&probe);
    source = simd_telemetry_source(exe);
    if (source == TELEMETRY_SOURCE_UDP)
    {
        f->ms->force_udp_mode = true;
        return;
    }
    if (source == TELEMETRY_SOURCE_SHM)
    {
        f->ms->force_udp_mode = false;
    }
}

static void discover_sim(loop_data* f, SimData* simdata, SimMap* simmap)
{
    apply_simd_telemetry_preference(f);
    f->siminfo = simapi_get_sim(simdata, simmap, f->ms->force_udp_mode, startudp, false);
    if (f->siminfo.mapapi == SIMULATORAPI_SIMAPI_TEST
        && !simapi_daemon_advancing(simdata, simmap))
    {
        slogd("SIMAPI.DAT is not advancing; mapping the simulator directly");
        close_simapi_map(simmap);
        f->siminfo = simapi_get_sim(simdata, simmap, f->ms->force_udp_mode, startudp, true);
    }
    if (try_acr_udp_bridge(f, simdata, simmap))
    {
        return;
    }
    if (discovered_sim_exe(f) == SIMULATOREXE_ASSETTO_CORSA_RALLY
        && simd_telemetry_source(SIMULATOREXE_ASSETTO_CORSA_RALLY) == TELEMETRY_SOURCE_SHM
        && ac_physics_shm_is_blank())
    {
        slogi("Assetto Corsa Rally telemetry=shm but /dev/shm/acpmf_physics is empty");
        slogi("Proton keeps Local\\acpmf_physics inside Wine; acr_shm_udp must mirror it");
    }
}

static void ensure_play_publishes_simapi(SimMap* simmap, SimData* simdata, SimulatorAPI mapapi)
{
    if (mapapi == SIMULATORAPI_SIMAPI_TEST)
    {
        return;
    }
    if (simmap == NULL || simmap->addr != NULL)
    {
        return;
    }
    simapi_universalmap_open(simmap, simdata);
}

static int bind_udp_port(int port)
{
    struct sockaddr_in recv_addr;

    uv_ip4_addr(UDP_BIND_ADDRESS, port, &recv_addr);
    return uv_udp_bind(&session.recv_socket, (const struct sockaddr*) &recv_addr, UV_UDP_REUSEADDR);
}

/* Passed to simapi as its bind callback, which carries no context pointer. */
int startudp(int port)
{
    int bind_port = port;
    int err;

    if (port == DIRT_RALLY_2_UDP_PORT)
    {
        bind_port = SIM_UDP_PORT_DIRT_RALLY_2_NATIVE;
    }
    err = bind_udp_port(bind_port);
    slogi("udp bind port %i result %i", bind_port, err);
    return err;
}

static void begin_live_mapping(uv_timer_t* handle, loop_data* f)
{
    int interval;

    f->state = APPSTATE_MAPPING;
    f->devices_pending = true;
    for (int i = 0; i < HAPTIC_WHEEL_COUNT; i++)
    {
        f->simdata->tyrediameter[i] = TYRE_DIAMETER_UNSET;
    }
    ensure_play_publishes_simapi(f->simmap, f->simdata, f->siminfo.mapapi);
    if (f->use_udp == true || f->siminfo.SimUsesUDP == true)
    {
        slogt("starting udp receive loop");
        map_live_simdata(f->simdata, f->simmap, f->siminfo.simulatorapi, true, NULL);
        publish_frame(f);
        load_devices_if_pending(f);
        uv_udp_recv_start(&f->recv_socket, on_alloc, on_udp_recv);
        slogt("udp receive loop started");
        uv_timer_stop(handle);
        return;
    }
    interval = interval_ms_for_fps(f->ms->fps);
    slogd("starting telemetry mapping at %i fps (%i ms ticks)", f->ms->fps, interval);
    uv_timer_start(&f->datamaptimer, shmdatamapcallback, SIM_MAPPING_START_DELAY_MS, interval);
}

void datacheckcallback(uv_timer_t* handle)
{
    loop_data* f = (loop_data*) handle->data;
    SimData* simdata = f->simdata;
    SimMap* simmap = f->simmap;

    if (f->state == APPSTATE_EXITING)
    {
        slogi("stopped checking for data");
        uv_timer_stop(handle);
        return;
    }
    if (f->releasing)
    {
        return;
    }

    if (f->state == APPSTATE_SEARCHING)
    {
        discover_sim(f, simdata, simmap);
        if (f->ms->force_udp_mode == true)
        {
            f->use_udp = true;
        }
    }

    if (f->siminfo.isSimOn == false || simdata->simstatus < SIMAPI_STATUS_ACTIVEPLAY)
    {
        return;
    }

    if (f->state == APPSTATE_SEARCHING)
    {
        begin_live_mapping(handle, f);
        return;
    }

    if (f->siminfo.mapapi != SIMULATORAPI_SIMAPI_TEST)
    {
        return;
    }

    f->siminfo = simapi_get_sim(simdata, simmap, f->ms->force_udp_mode, NULL, false);
    if (f->siminfo.isSimOn == false)
    {
        releaseloop(f);
    }
}

static int session_open(loop_data* f, CargopitSettings* ms, uv_loop_t* loop)
{
    memset(f, 0, sizeof(*f));
    f->simdata = calloc(1, sizeof(SimData));
    f->simmap = simapi_simmap_create();
    if (f->simdata == NULL || f->simmap == NULL || telemetry_snapshot_init(&f->snapshot) != 0)
    {
        free(f->simdata);
        free(f->simmap);
        return CARGOPIT_ERROR_UNKNOWN;
    }
    f->loop = loop;
    f->ms = ms;
    f->siminfo.mapapi = -1;
    f->state = APPSTATE_SEARCHING;
    f->config_index = resolve_config_index(ms->config_str, ms->config_index);

    simapi_set_log_info(simapilib_loginfo);
    simapi_set_log_debug(simapilib_logdebug);
    simapi_set_log_trace(simapilib_logtrace);

    uv_udp_init(loop, &f->recv_socket);
    uv_timer_init(loop, &f->datachecktimer);
    uv_timer_init(loop, &f->datamaptimer);
    uv_timer_init(loop, &f->tyrediametertimer);
    uv_async_init(loop, &f->stop_async, on_stop_requested);
    f->recv_socket.data = f;
    f->datachecktimer.data = f;
    f->datamaptimer.data = f;
    f->tyrediametertimer.data = f;
    f->stop_async.data = f;

    uv_timer_start(&f->datachecktimer, datacheckcallback, SIM_CHECK_INTERVAL_MS, SIM_CHECK_INTERVAL_MS);
    return 0;
}

static void session_close(loop_data* f)
{
    uv_walk(f->loop, close_walk_cb, NULL);
    uv_run(f->loop, UV_RUN_DEFAULT);
    uv_loop_close(f->loop);
    slogi("All threads stopped...");
    telemetry_snapshot_destroy(&f->snapshot);
    free(f->simdata);
    free(f->simmap);
    f->simdata = NULL;
    f->simmap = NULL;
}

static const char* session_sim_name(const loop_data* f)
{
    if (f->state == APPSTATE_EXITING || f->siminfo.simulatorexe <= 0)
    {
        return SIM_NOT_DETECTED;
    }
    return simapi_gametofullstr(f->siminfo.simulatorexe);
}

/* JSON-safe copy of a display name: drops quotes, backslashes and control characters. */
static void json_safe_copy(char* out, size_t len, const char* in)
{
    size_t n = 0;
    for (; in != NULL && *in != '\0' && n + 1 < len; in++)
    {
        if (*in == '"' || *in == '\\' || (unsigned char) *in < ' ')
        {
            continue;
        }
        out[n++] = *in;
    }
    out[n] = '\0';
}

static char* session_status_json(const loop_data* f)
{
    char sim[CONTROL_NAME_MAX];
    unsigned long updates = 0;
    unsigned long overruns = 0;
    int active = 0;
    char* out = malloc(CONTROL_STATUS_MAX);

    if (out == NULL)
    {
        return NULL;
    }
    for (int x = 0; f->runners != NULL && x < f->numdevices; x++)
    {
        if (!f->runners[x].started)
        {
            continue;
        }
        active++;
        updates += (unsigned long) atomic_load(&f->runners[x].updates);
        overruns += (unsigned long) atomic_load(&f->runners[x].overruns);
    }
    json_safe_copy(sim, sizeof(sim), session_sim_name(f));
    snprintf(out, CONTROL_STATUS_MAX,
             "{\"ok\":true,\"state\":\"%s\",\"releasing\":%s,\"paused\":%s,\"config_index\":%d,"
             "\"sim\":\"%s\",\"devices\":%d,\"updates\":%lu,\"overruns\":%lu}",
             APP_STATE_NAMES[f->state], f->releasing ? JSON_TRUE : JSON_FALSE, f->user_stopped ? JSON_TRUE : JSON_FALSE,
             f->config_index, sim, active, updates, overruns);
    return out;
}

/* Re-read the device profile: release now, and the next detected frame loads devices from disk again. */
static void session_reload(loop_data* f)
{
    f->user_stopped = false;
    if (f->state == APPSTATE_MAPPING)
    {
        slogi("reload requested, releasing devices to load the saved profile");
        releaseloop(f);
        return;
    }
    if (f->state == APPSTATE_SEARCHING && !f->releasing && !uv_is_active((uv_handle_t*) &f->datachecktimer))
    {
        uv_timer_start(&f->datachecktimer, datacheckcallback, 0, SIM_CHECK_INTERVAL_MS);
    }
}

static char* handle_control_command(void* ctx, const char* command)
{
    loop_data* f = ctx;

    if (strcmp(command, CONTROL_CMD_STATUS) == 0)
    {
        return session_status_json(f);
    }
    if (strcmp(command, CONTROL_CMD_RELOAD) == 0)
    {
        session_reload(f);
        return strdup(CONTROL_REPLY_OK);
    }
    if (strcmp(command, CONTROL_CMD_STOP) == 0)
    {
        session_request_exit(f);
        return strdup(CONTROL_REPLY_OK);
    }
    return strdup(CONTROL_REPLY_UNKNOWN);
}

static void start_cli_controls(loop_data* f)
{
    control_server_start(&f->control, f->loop, handle_control_command, f);
    uv_signal_init(f->loop, &f->sigterm);
    uv_signal_init(f->loop, &f->sigint);
    f->sigterm.data = f;
    f->sigint.data = f;
    uv_signal_start(&f->sigterm, stop_mainloop_from_signal, SIGTERM);
    uv_signal_start(&f->sigint, stop_mainloop_from_signal, SIGINT);
    f->signals_started = true;
    if (f->stdin_poll == NULL)
    {
        return;
    }
    f->stdin_poll->data = f;
    if (uv_poll_start(f->stdin_poll, UV_READABLE, on_stdin_key) != 0)
    {
        slogw("could not start stdin poll; continuing without quit key");
    }
}

AppState gameloop_app_state(void)
{
    return session.state;
}

static void* run_background_loop(void* arg)
{
    loop_data* f = arg;
    uv_run(f->loop, UV_RUN_DEFAULT);
    return NULL;
}

int start_loop(CargopitSettings* ms)
{
    int simd_error = require_simd();
    if (simd_error != 0)
    {
        return simd_error;
    }

    uv_loop_t* loop = malloc(sizeof(uv_loop_t));
    if (loop == NULL)
    {
        return CARGOPIT_ERROR_UNKNOWN;
    }
    if (uv_loop_init(loop) != 0 || session_open(&session, ms, loop) != 0)
    {
        free(loop);
        return CARGOPIT_ERROR_UNKNOWN;
    }
    return pthread_create(&loop_thread, NULL, run_background_loop, &session);
}

int cargopit_mainloop_stop(CargopitSettings* ms)
{
    (void) ms;
    uv_async_send(&session.stop_async);
    pthread_join(loop_thread, NULL);
    session_close(&session);
    free(session.loop);
    session.loop = NULL;
    return 0;
}

const char* get_simexe_name(void)
{
    return session_sim_name(&session);
}

const char* get_simd_onoff(void)
{
    if (session.state == APPSTATE_EXITING || session.siminfo.mapapi != 0)
    {
        return SIMD_NOT_DETECTED;
    }
    return SIMD_RUNNING;
}

int cargopit_mainloop(CargopitSettings* ms)
{
    struct termios canonicalmode;
    int stdin_was_raw = 0;
    uv_loop_t* loop = uv_default_loop();
    int simd_error = require_simd();

    if (simd_error != 0)
    {
        return simd_error;
    }
    if (session_open(&session, ms, loop) != 0)
    {
        return CARGOPIT_ERROR_UNKNOWN;
    }
    memset(&canonicalmode, 0, sizeof(canonicalmode));
    session.stdin_poll = init_stdin_quit_poll(loop, &canonicalmode, &stdin_was_raw);
    start_cli_controls(&session);

    fprintf(stdout, "Searching for sim data... Press q to quit...\n");
    uv_run(loop, UV_RUN_DEFAULT);
    session_close(&session);
    uv_library_shutdown();

    fprintf(stdout, "\n");
    fflush(stdout);
    restore_stdin_terminal(&canonicalmode, stdin_was_raw);
    free(session.stdin_poll);
    session.stdin_poll = NULL;
    return 0;
}
