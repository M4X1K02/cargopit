#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <poll.h>
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

#define DEFAULT_UPDATE_RATE      240.0
#define SIM_CHECK_RATE           1.0
#define SIMAPI_DAEMON_PROBE_US   50000
#define SIM_UDP_PORT_DIRT_RALLY_2_NATIVE 20777
#define TEST_TICK_US                 16000
#define TEST_PHASE_HOLD_US           3000000
#define TEST_IDLE_HOLD_US            1000000
#define TEST_WHEEL_COUNT             4
#define TEST_RPM_IDLE                1000
#define TEST_RPM_MID_LOW             2000
#define TEST_RPM_MID                 4000
#define TEST_RPM_HIGH                7000
#define TEST_RPM_MAX                 8000
#define TEST_RPM_SWEEP_STEP          50
#define TEST_VELOCITY_IDLE           16
#define TEST_VELOCITY_SLOW           100
#define TEST_VELOCITY_CRUISE         160
#define TEST_VELOCITY_FAST           200
#define TEST_VELOCITY_TOP            300
#define TEST_VELOCITY_SPIN           15
#define TEST_VELOCITY_LOCK           150
#define TEST_YVELOCITY               100
#define TEST_PEDAL_APPLIED           0.85
#define TEST_GAS_CRUISE              0.40
#define TEST_SLIP_SPIN               (-0.45)
#define TEST_SLIP_LOCK               0.85
#define TEST_GEAR_PULSE_TICKS        8
#define TEST_SLIP_ABS_A              0.40
#define TEST_SLIP_ABS_B              0.72
#define TEST_ABS_OFF                 0.0
#define TEST_ABS_ACTIVE              1.0
#define TEST_ABS_PULSE_TICKS         2
#define TEST_BRAKE_TEMP_HOT          0.90
#define TEST_SUSP_VEL_A              2.0
#define TEST_SUSP_VEL_B              18.0
#define TEST_TYRE_DIAMETER_UNSET     (-1.0)
#define TEST_TYRE_DIAMETER_FL        0.638636385206394
#define TEST_TYRE_DIAMETER_FR        0.633384434597093
#define TEST_TYRE_DIAMETER_RL        0.710475735564615
#define TEST_TYRE_DIAMETER_RR        0.710475735564615
#define TEST_TYRE_RPS_SPIN           50.0
#define TEST_TYRE_RPS_LOCK           25.0
#define TEST_CAR_NAME                "CAR"
#define TEST_GEAR_CHAR_NEUTRAL       'N'
#define TEST_GEAR_CHAR_FIRST         '1'
#define TEST_GEAR_CHAR_SECOND        '2'
#define TEST_GEAR_CHAR_THIRD         '3'
#define TEST_GEAR_CHAR_FOURTH        '4'
#define TEST_MSG_REV                 "Revving rpm from idle to redline and back"
#define TEST_MSG_RPM_IDLE            "Setting rpms to idle"
#define TEST_MSG_GREEN               "Green Flag!"
#define TEST_MSG_FIRST               "Shifting into first gear"
#define TEST_MSG_SPEED_SLOW          "Setting speed to 100"
#define TEST_MSG_SPIN                "Testing wheel spin"
#define TEST_MSG_BUTTONS             "Lighting brake button LEDs"
#define TEST_MSG_LOCK                "Testing wheel lock"
#define TEST_MSG_SECOND              "Shifting into second gear"
#define TEST_MSG_ABS                 "Testing ABS"
#define TEST_MSG_SPEED_FAST          "Setting speed to 200"
#define TEST_MSG_YELLOW              "Yellow Flag!"
#define TEST_MSG_THIRD               "Shifting into third gear"
#define TEST_MSG_RPM_MID_LOW         "Setting rpms to 2000"
#define TEST_MSG_BLUE                "Blue Flag!"
#define TEST_MSG_RPM_MID             "Setting rpms to 4000"
#define TEST_MSG_FOURTH              "Shifting into fourth gear"
#define TEST_MSG_SPEED_TOP           "Setting speed to 300"
#define TEST_MSG_RPM_HIGH            "Setting rpms to 7000"
#define TEST_MSG_RED                 "Red Flag!"
#define TEST_MSG_RPM_MAX             "Setting rpms to redline"
#define TEST_MSG_SUSPENSION          "Testing suspension"
#define TEST_MSG_COAST               "Returning to idle"
#define TEST_MSG_PREPARING           "preparing test with %i devices..."
#define TEST_STEP_PREFIX             "test step: "
#define TEST_MSG_STARTING            "Starting"
#define TEST_MSG_FINISHED            "Finished"
#define TEST_MSG_STOPPED             "Stopped"
#define TEST_LABEL_SERIAL_LIGHTS     "serial lights"
#define TEST_LABEL_USB_LIGHTS        "USB lights"
#define TEST_LABEL_DEVICE            "device"
#define TEST_KEY_QUIT                'q'
#define TEST_KEY_QUIT_UPPER          'Q'
#define TEST_KEY_ESC                 '\033'
#define TEST_STDIN_POLL_MS           0
#define TEST_STDIN_POLL_FDS          1
#define TEST_STDIN_READ_BYTES        1

bool go = false;
bool go2 = false;
struct sigaction act;
static pthread_t loop_thread;
static uv_loop_t *loop = NULL;

SimData* simdata;
SimMap* simmap;
loop_data* baton;

device_loop_data* test_baton;
SimDevice* test_simdevice;
SimInfo* test_siminfo;
static volatile sig_atomic_t tester_abort;
static int tester_stdin_raw;

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

static uv_poll_t* init_stdin_quit_poll(struct termios* canonicalmode, int* stdin_was_raw)
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
    if (uv_poll_init(uv_default_loop(), poll, STDIN_FILENO) != 0)
    {
        slogw("could not poll stdin; continuing without quit key");
        free(poll);
        restore_stdin_terminal(canonicalmode, *stdin_was_raw);
        *stdin_was_raw = 0;
        return NULL;
    }
    return poll;
}

static void stop_mainloop_from_signal(uv_signal_t* handle, int signum)
{
    (void)signum;
    uv_signal_stop(handle);
    appstate = 0;
    slogi("signal stop, appstate is now %i", appstate);
    uv_stop(uv_default_loop());
}


uv_idle_t idler;
uv_timer_t datachecktimer;
uv_timer_t showstatstimer;
uv_timer_t datamaptimer;
uv_timer_t tyrediametertimer;
uv_timer_t testdevicetimer;
uv_udp_t recv_socket;

bool doui = false;


void shmdatamapcallback(uv_timer_t* handle);
void showstatscallback(uv_timer_t* handle);
void datacheckcallback(uv_timer_t* handle);
void tyrediametercheckcallback(uv_timer_t* handle);
void startdatalogger(CargopitSettings* ms, loop_data* l);

static void close_walk_cb(uv_handle_t* handle, void* arg)
{
    if (!uv_is_closing(handle))
    {
        uv_close(handle, NULL);
    }
}
#define ASSERT(expr) expr

static void showstats(SimData* simdata)
{
    printf("\r");
    for (int i=0; i<4; i++)
    {
        if (i==0)
        {
            fputc('s', stdout);
            fputc('p', stdout);
            fputc('e', stdout);
            fputc('e', stdout);
            fputc('d', stdout);
            fputc(':', stdout);
            fputc(' ', stdout);

            int speed = simdata->velocity;
            int digits = 0;
            if (speed > 0)
            {
                while (speed > 0)
                {
                    speed = speed / 10;
                    digits++;
                }
                speed = simdata->velocity;
                int s[digits];
                int digit = 0;
                while (speed > 0)
                {
                    int mod = speed % 10;
                    s[digit] = mod;
                    speed = speed / 10;
                    digit++;
                }
                speed = simdata->velocity;
                digit = digits;
                while (digit > 0)
                {
                    fputc(s[digit-1]+'0', stdout);
                    digit--;
                }
            }
            else
            {
                fputc('0', stdout);
            }
            fputc(' ', stdout);
        }
        if (i==1)
        {
            fputc('r', stdout);
            fputc('p', stdout);
            fputc('m', stdout);
            fputc('s', stdout);
            fputc(':', stdout);
            fputc(' ', stdout);

            int rpms = simdata->rpms;
            int digits = 0;
            if (rpms > 0)
            {
                while (rpms > 0)
                {
                    rpms = rpms / 10;
                    digits++;
                }
                rpms = simdata->rpms;
                int s[digits];
                int digit = 0;
                while (rpms > 0)
                {
                    int mod = rpms % 10;
                    s[digit] = mod;
                    rpms = rpms / 10;
                    digit++;
                }
                rpms = simdata->rpms;
                digit = digits;
                while (digit > 0)
                {
                    fputc(s[digit-1]+'0', stdout);
                    digit--;
                }
            }
            else
            {
                fputc('0', stdout);
            }
            fputc(' ', stdout);
        }
        if (i==2)
        {
            fputc('g', stdout);
            fputc('e', stdout);
            fputc('a', stdout);
            fputc('r', stdout);
            fputc(':', stdout);
            fputc(' ', stdout);
            fputc(simdata->gear+'0', stdout);
            fputc(' ', stdout);
        }
        if (i==3)
        {
            fputc('a', stdout);
            fputc('l', stdout);
            fputc('t', stdout);
            fputc(':', stdout);
            fputc(' ', stdout);

            int alt = simdata->altitude;
            int digits = 0;
            if (alt > 0)
            {
                while (alt > 0)
                {
                    alt = alt / 10;
                    digits++;
                }
                alt = simdata->altitude;
                int s[digits];
                int digit = 0;
                while (alt > 0)
                {
                    int mod = alt % 10;
                    s[digit] = mod;
                    alt = alt / 10;
                    digit++;
                }
                alt = simdata->altitude;
                digit = digits;
                while (digit > 0)
                {
                    fputc(s[digit-1]+'0', stdout);
                    digit--;
                }
            }
            else
            {
                fputc('0', stdout);
            }
            fputc(' ', stdout);
        }
    }
    fflush(stdout);
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

void on_timer_close_complete(uv_handle_t* handle)
{
    free(handle);
}


void devicetimercallback(uv_timer_t* handle)
{
    void* b = uv_handle_get_data((uv_handle_t*) handle);
    device_loop_data* f = (device_loop_data*) b;
    SimData* simdata = f->simdata;
    SimDevice* device = f->simdevice;
    device->update(device, simdata);
}


void tyrediametercheckcallback(uv_timer_t* handle)
{
    void* b = uv_handle_get_data((uv_handle_t*) handle);
    device_loop_data* f = (device_loop_data*) b;
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
            slogi("saving new tyre diameter config for car %s");
            savetyreconfig(simdata, f->ms->tyre_diameter_config);
        }
        uv_timer_stop(handle);
    }

}

void looprun(CargopitSettings* ms, loop_data* f, SimData* simdata)
{
    if (doui == true)
    {
        slogi("loading device profile from %s (config-index %i)", ms->config_str, ms->config_index);
        int confignum = resolve_config_index(ms->config_str, ms->config_index);
        if (confignum < 0)
        {
            sloge("no device profile to load (config-index %i)", ms->config_index);
            doui = false;
            return;
        }

        int configureddevices;
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

        int numdevices = f->numdevices;
        SimDevice* devices = f->simdevices;
        f->device_timers = (uv_timer_t*) (malloc(uv_handle_size(UV_TIMER) * numdevices));
        f->device_batons = (device_loop_data*) (malloc(sizeof(device_loop_data) * numdevices));
        f->started_tyre_calc_thread = false;

        for (int x = 0; x < numdevices; x++)
        {
            if (devices[x].initialized == true)
            {
                device_loop_data* dld = &f->device_batons[x];
                dld->simdevice = &devices[x];
                dld->simdata = simdata;
                uv_timer_t* dt = &f->device_timers[x];
                uv_timer_init(uv_default_loop(), dt);
                uv_handle_set_data((uv_handle_t*) dt, (void*) dld);
                int interval = 1000/devices[x].fps;
                uv_timer_start(dt, devicetimercallback, 0, interval);
                slogi("starting device type %i at id at %i fps: %i (%i ms ticks)", devices[x].type, x, devices[x].fps, interval);
               
                SimInfo siminfo = f->siminfo;
                if(f->started_tyre_calc_thread == false)
                {
                        if(devices[x].hapticeffect.effecttype == EFFECT_TYRELOCK || devices[x].hapticeffect.effecttype == EFFECT_TYRESLIP || devices[x].hapticeffect.effecttype == EFFECT_ABSBRAKES)
                        {
                            if(siminfo.SimCalculatesTyreDiameter == false && siminfo.SimSupportsHapticEffects == true && siminfo.SimCalculatesSlipRatio == false && f->ms->useconfig == 1 && f->ms->tyre_diameter_config != NULL)
                            {
                                slogi("Starting thread to calculate tyre diameters and save to config file");
                                f->started_tyre_calc_thread = true;

                                f->tyrebaton = (device_loop_data*) malloc(sizeof(device_loop_data));
                                f->tyrebaton->simdata = simdata;
                                f->tyrebaton->ms = f->ms;

                                uv_handle_set_data((uv_handle_t*) &tyrediametertimer, (void*) f->tyrebaton);
                                uv_timer_start(&tyrediametertimer, tyrediametercheckcallback, 0, 1000);
                            }
                        }
                }
            }
        }


        //uv_timer_start(&showstatstimer, showstatscallback, 0, 100);
        doui = false;
    }
}

void showstatscallback(uv_timer_t* handle)
{
    void* b = uv_handle_get_data((uv_handle_t*) handle);
    loop_data* f = (loop_data*) b;
    SimData* simdata = f->simdata;
    if(appstate == 2)
    {
        showstats(simdata);
    }
}

void releaseloop(loop_data* f, SimData* simdata, SimMap* simmap)
{
        slogi("release loop");
        if(f->releasing == false)
        {
            f->releasing = true;
            uv_timer_stop(&datamaptimer);
            uv_timer_stop(&showstatstimer);
            if (uv_is_active((uv_handle_t*)&recv_socket))
            {
                uv_udp_recv_stop(&recv_socket);
            }
            // Close the socket handle so it can be reinitialized with a different port
            //if (!uv_is_closing((uv_handle_t*)&recv_socket))
            //{
            //    uv_close((uv_handle_t*)&recv_socket, NULL);
            //}
            slogi("releasing devices, please wait");

            //attempt tyre diameter saving
            uv_timer_stop(&tyrediametertimer);
            if(f->started_tyre_calc_thread == true)
            {
                free(f->tyrebaton);
            }

            f->uion = false;
            SimDevice* devices = f->simdevices;
            int numdevices = f->numdevices;

            // help things spin down
            simdata->simstatus = 0;
            simdata->rpms = 0;
            simdata->velocity = 0;

            for (int x = 0; x < numdevices; x++)
            {
                if (devices[x].initialized == true)
                {
                    uv_timer_t* dt = &f->device_timers[x];
                    slogt("attempting device timer stop and release");
                    slogt("timer active status %i", uv_is_active((uv_handle_t*) dt));
                    uv_timer_stop(dt);
                    //uv_close((uv_handle_t*) dt, on_timer_close_complete);
                }
            }
            free(f->device_batons);
            free(f->device_timers);
            slogt("stopped device timers");
            for (int x = 0; x < numdevices; x++)
            {
                if (devices[x].initialized == true)
                {
                    devices[x].update(&devices[x], simdata);
                }
            }
            sleep(1);
            for (int x = 0; x < numdevices; x++)
            {
                if (devices[x].initialized == true)
                {
                    devices[x].free(&devices[x]);
                }
            }
            free(devices);

            int r = simapi_sim_clear(simdata, simmap, false);
            slogd("simfree returned %i", r);
            f->numdevices = 0;
            slogi("stopped mapping data, press q again to quit");
            //stopui(ms->ui_type, f);
            // free loop data

            if(appstate > 0)
            {
                slogi("restarting checking for data...");
                uv_timer_start(&datachecktimer, datacheckcallback, 0, 1000);
            }
            f->releasing = false;
            if(appstate > 1)
            {
                appstate = 1;
            }
        }
}

void shmdatamapcallback(uv_timer_t* handle)
{
    void* b = uv_handle_get_data((uv_handle_t*) handle);
    loop_data* f = (loop_data*) b;
    SimData* simdata = f->simdata;
    SimMap* simmap = f->simmap;
    CargopitSettings* ms = f->ms;
    //appstate = 2;
    if (appstate == 2)
    {
        map_live_simdata(simdata, simmap, f->siminfo.mapapi, false, NULL);
        looprun(ms, f, simdata);
    }

    if (f->siminfo.isSimOn == false || simdata->simstatus <= 1 || appstate <= 1)
    {
        releaseloop(f, simdata, simmap);
    }
}

void on_alloc(uv_handle_t* client, size_t suggested_size, uv_buf_t* buf) {
    buf->base = malloc(suggested_size);
    buf->len = suggested_size;
    bzero(buf->base, suggested_size);
    slogt("udp malloc:%lu %p\n",buf->len,buf->base);
}

static void on_udp_recv(uv_udp_t* handle, ssize_t nread, const uv_buf_t* rcvbuf, const struct sockaddr* addr, unsigned flags) {
    if (nread > 0) {
        slogt("udp data received");
    }
    if (nread <= 0) {
        free(rcvbuf->base);
        return;
    }

    char* a;
    a = rcvbuf->base;

    void* b = uv_handle_get_data((uv_handle_t*) handle);
    loop_data* f = (loop_data*) b;
    SimData* simdata = f->simdata;
    SimMap* simmap = f->simmap;
    CargopitSettings* ms = f->ms;

    if (appstate == 2 && acr_udp_packet_ok(a, (size_t)nread))
    {
        acr_udp_apply(simdata, a);
        acr_udp_publish(simmap, simdata);
        looprun(ms, f, simdata);
        slogt("udp free  :%lu %p\n",rcvbuf->len,rcvbuf->base);
        free(rcvbuf->base);
        return;
    }

    if (appstate == 2)
    {
        map_live_simdata(simdata, simmap, f->siminfo.mapapi, true, a);
        looprun(ms, f, simdata);
    }

    if (f->siminfo.isSimOn == false || simdata->simstatus <= 1 || appstate <= 1)
    {
        releaseloop(f, simdata, simmap);
    }

    slogt("udp free  :%lu %p\n",rcvbuf->len,rcvbuf->base);
    free(rcvbuf->base);
}

int startudp(int port);

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

    uv_ip4_addr("0.0.0.0", port, &recv_addr);
    return uv_udp_bind(&recv_socket, (const struct sockaddr*) &recv_addr, UV_UDP_REUSEADDR);
}

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

void udpstart(CargopitSettings* sms, loop_data* f, SimData* simdata, SimMap* simmap)
{
    if (appstate == 2)
    {
        map_live_simdata(simdata, simmap, f->siminfo.simulatorapi, true, NULL);
        if (doui == true)
        {
            looprun(sms, f, simdata);
        }
    }
}

static void begin_live_mapping(uv_timer_t* handle, loop_data* f, SimData* simdata, SimMap* simmap)
{
    int interval;

    appstate++;
    doui = true;
    simdata->tyrediameter[0] = -1;
    simdata->tyrediameter[1] = -1;
    simdata->tyrediameter[2] = -1;
    simdata->tyrediameter[3] = -1;
    ensure_play_publishes_simapi(simmap, simdata, f->siminfo.mapapi);
    if (f->use_udp == true || f->siminfo.SimUsesUDP == true)
    {
        slogt("starting udp receive loop");
        udpstart(f->ms, f, simdata, simmap);
        uv_udp_recv_start(&recv_socket, on_alloc, on_udp_recv);
        slogt("udp receive loop started");
        uv_timer_stop(handle);
        return;
    }
    interval = 1000 / f->ms->fps;
    slogd("starting telemetry mapping at %i fps (%i ms ticks)", f->ms->fps, interval);
    uv_timer_start(&datamaptimer, shmdatamapcallback, 2000, interval);
}

void datacheckcallback(uv_timer_t* handle)
{
    void* b = uv_handle_get_data((uv_handle_t*) handle);
    loop_data* f = (loop_data*) b;
    SimData* simdata = f->simdata;
    SimMap* simmap = f->simmap;

    if (appstate == 0)
    {
        slogi("stopped checking for data");
        uv_timer_stop(handle);
        return;
    }

    if (appstate == 1)
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

    if (appstate == 1)
    {
        begin_live_mapping(handle, f, simdata, simmap);
        return;
    }

    if (f->siminfo.mapapi != SIMULATORAPI_SIMAPI_TEST)
    {
        return;
    }

    f->siminfo = simapi_get_sim(simdata, simmap, f->ms->force_udp_mode, NULL, false);
    if (f->siminfo.isSimOn == false)
    {
        appstate = 1;
        releaseloop(f, simdata, simmap);
    }
}

void cb(uv_poll_t* handle, int status, int events)
{
    void* b = uv_handle_get_data((uv_handle_t*) handle);
    loop_data* f = (loop_data*) b;
    char ch;
    ssize_t bytes_read = read(STDIN_FILENO, &ch, sizeof(ch));
    if (bytes_read != sizeof(ch))
    {
        return;
    }

    if (ch == 'q')
    {
        if(f->releasing == false && doui == false)
        {
            appstate--;
            fprintf(stdout, "\nUser requested stop appstate is now %i\n", appstate);
            fflush(stdout);
            slogi("User requested stop appstate is now %i", appstate);
        }
    }

    if (appstate == 0)
    {
        slogi("Cargopit is exiting...");
        uv_udp_recv_stop(&recv_socket);
        uv_timer_stop(&datamaptimer);
        uv_timer_stop(&showstatstimer);
        // at this point these below should be the only active threads
        uv_timer_stop(&datachecktimer);
        uv_poll_stop(handle);
    }
}



void* cargopit_mainloop_start(void* arg)
{
    CargopitSettings* ms = arg;
    simdata = malloc(sizeof(SimData));
    simmap = simapi_simmap_create();
    slogd("setting initial app state");
    appstate = 1;

    //struct pollfd mypoll = { STDIN_FILENO, POLLIN|POLLPRI };
    //uv_poll_t* poll = (uv_poll_t*) malloc(uv_handle_size(UV_POLL));

    baton = (loop_data*) malloc(sizeof(loop_data));
    baton->siminfo.mapapi = -1;
    baton->simmap = simmap;
    baton->simdata = simdata;
    baton->ms = ms;
    baton->uion = false;
    baton->releasing = false;
    baton->use_udp = false;
    baton->req.data = (void*) baton;


    simapi_set_log_info(simapilib_loginfo);
    simapi_set_log_debug(simapilib_logdebug);
    simapi_set_log_trace(simapilib_logtrace);

    //if (0 != uv_poll_init(uv_default_loop(), poll, 0))
    //{
    //    return NULL;
    //};
   
    uv_udp_init(loop, &recv_socket);
    uv_timer_init(loop, &datachecktimer);
    uv_timer_init(loop, &datamaptimer);
    uv_timer_init(loop, &tyrediametertimer);



    uv_handle_set_data((uv_handle_t*) &datachecktimer, (void*) baton);
    uv_handle_set_data((uv_handle_t*) &datamaptimer, (void*) baton);
    //uv_handle_set_data((uv_handle_t*) &showstatstimer, (void*) baton);
    uv_handle_set_data((uv_handle_t*) &recv_socket, (void*) baton);
    //}
    //uv_handle_set_data((uv_handle_t*) poll, (void*) baton);

    //if (0 != uv_poll_start(poll, UV_READABLE, cb))
    //{
    //    return NULL;
    //};

    uv_timer_start(&datachecktimer, datacheckcallback, 1000, 1000);

    //fprintf(stdout, "Searching for sim data... Press q to quit...\n");
    uv_run(loop, UV_RUN_DEFAULT);

    return NULL;
}

SimData* get_test_simdata(void)
{
    if(test_baton == NULL)
    {
        return NULL;
    }
    return test_baton->simdata;
}

void* cargopit_testloop_start(void* arg)
{
    test_loop_args* args = arg;
    simmap = simapi_simmap_create();


    simapi_set_log_info(simapilib_loginfo);
    simapi_set_log_debug(simapilib_logdebug);
    simapi_set_log_trace(simapilib_logtrace);


    DeviceSettings ds;
    //int numdevices = getsingledevice(args->ms->config_str, args->confignum, args->devicenum, args->ms, &ds);
    test_simdevice = malloc(1 * sizeof(SimDevice));
    test_siminfo = malloc(sizeof(SimInfo));
    simapi_set_faux_siminfo(test_siminfo);
    int initdevices = devinit(test_simdevice, test_siminfo, 1, args->ds, args->ms);
    //settingsfree(ds);



    test_baton->siminfo.mapapi = -1;
    test_baton->simdevice = test_simdevice;

    test_baton->ms = args->ms;
    test_baton->req.data = (void*) test_baton;


    uv_timer_init(loop, &testdevicetimer);
    uv_handle_set_data((uv_handle_t*) &testdevicetimer, (void*) test_baton);
    uv_timer_start(&testdevicetimer, devicetimercallback, 1000, 16);

    uv_run(loop, UV_RUN_DEFAULT);


    return NULL;
}

int cargopit_testloop_stop(void)
{
    uv_timer_stop(&testdevicetimer);

    uv_stop(loop);
    pthread_join(loop_thread, NULL);

    uv_run(loop, UV_RUN_NOWAIT);
    uv_walk(loop, close_walk_cb, NULL);
    uv_loop_close(loop);
    uv_library_shutdown();

    slogi("All threads stopped...");

    test_simdevice->free(test_simdevice);
    free(loop);
    free(test_baton);
    //free(simdata);
    free(simmap);

    return 0;
}

int cargopit_mainloop_stop(CargopitSettings* ms)
{

    uv_udp_recv_stop(&recv_socket);
    uv_timer_stop(&datamaptimer);
    uv_timer_stop(&datachecktimer);

    uv_stop(loop);
    pthread_join(loop_thread, NULL);

    uv_run(loop, UV_RUN_NOWAIT);
    uv_walk(loop, close_walk_cb, NULL);
    uv_loop_close(loop);
    uv_library_shutdown();

    slogi("All threads stopped...");

    free(loop);
    free(baton);
    free(simdata);
    free(simmap);

    appstate = 0;
    return 0;
}


int start_loop(CargopitSettings* ms)
{
    int simd_error = require_simd();
    if (simd_error != 0)
    {
        return simd_error;
    }

    loop = malloc(sizeof(uv_loop_t));

    if (loop == NULL)
    {
        return -1;
    }

    if (uv_loop_init(loop) != 0)
    {
        free(loop);
        loop = NULL;
        return -1;
    }
    return pthread_create(&loop_thread, NULL, cargopit_mainloop_start, ms);
}

int start_test(test_loop_args* test_data)
{

    test_baton = (device_loop_data*) malloc(sizeof(device_loop_data));
    test_baton->simdata = test_data->simdata;

    loop = malloc(sizeof(uv_loop_t));

    if (loop == NULL)
    {
        return -1;
    }

    if (uv_loop_init(loop) != 0)
    {
        free(loop);
        loop = NULL;
        return -1;
    }
    return pthread_create(&loop_thread, NULL, cargopit_testloop_start, test_data);
}

const char* get_simexe_name(void)
{
    if(appstate <= 0)
    {
        return "None Detected";
    }
    if(baton == NULL)
    {
        return "None Detected";
    }
    if(baton->siminfo.simulatorexe <= 0)
    {
        return "None Detected";
    }
    return simapi_gametofullstr(baton->siminfo.simulatorexe);
}

const char* get_simd_onoff(void)
{
    if(appstate <= 0)
    {
        return "Not Detected";
    }
    if(baton == NULL)
    {
        return "Not Detected";
    }
    if(baton->siminfo.mapapi == 0)
    {
        return "Running";
    }
    return "Not Detected";
}

int cargopit_mainloop(CargopitSettings* ms)
{
    int simd_error = require_simd();
    if (simd_error != 0)
    {
        return simd_error;
    }

    simdata = malloc(sizeof(SimData));
    simmap = simapi_simmap_create();

    struct termios canonicalmode;
    memset(&canonicalmode, 0, sizeof(canonicalmode));
    int stdin_was_raw = 0;
    uv_poll_t* poll = init_stdin_quit_poll(&canonicalmode, &stdin_was_raw);

    baton = (loop_data*) malloc(sizeof(loop_data));
    baton->simmap = simmap;
    baton->simdata = simdata;
    baton->ms = ms;
    baton->uion = false;
    baton->releasing = false;
    baton->use_udp = false;
    baton->req.data = (void*) baton;

    simapi_set_log_info(simapilib_loginfo);
    simapi_set_log_debug(simapilib_logdebug);
    simapi_set_log_trace(simapilib_logtrace);

    uv_udp_init(uv_default_loop(), &recv_socket);
    uv_timer_init(uv_default_loop(), &datachecktimer);
    uv_timer_init(uv_default_loop(), &showstatstimer);
    uv_timer_init(uv_default_loop(), &datamaptimer);
    uv_timer_init(uv_default_loop(), &tyrediametertimer);
    slogd("setting initial app state");
    appstate = 1;

    uv_handle_set_data((uv_handle_t*) &datachecktimer, (void*) baton);
    uv_handle_set_data((uv_handle_t*) &datamaptimer, (void*) baton);
    uv_handle_set_data((uv_handle_t*) &showstatstimer, (void*) baton);
    uv_handle_set_data((uv_handle_t*) &recv_socket, (void*) baton);
    if (poll != NULL)
    {
        uv_handle_set_data((uv_handle_t*) poll, (void*) baton);
        if (uv_poll_start(poll, UV_READABLE, cb) != 0)
        {
            slogw("could not start stdin poll; continuing without quit key");
        }
    }

    uv_signal_t sigterm;
    uv_signal_t sigint;
    uv_signal_init(uv_default_loop(), &sigterm);
    uv_signal_init(uv_default_loop(), &sigint);
    uv_signal_start(&sigterm, stop_mainloop_from_signal, SIGTERM);
    uv_signal_start(&sigint, stop_mainloop_from_signal, SIGINT);

    uv_timer_start(&datachecktimer, datacheckcallback, 1000, 1000);

    fprintf(stdout, "Searching for sim data... Press q to quit...\n");
    uv_run(uv_default_loop(), UV_RUN_DEFAULT);

    uv_signal_stop(&sigterm);
    uv_signal_stop(&sigint);
    uv_stop(uv_default_loop());
    uv_run(uv_default_loop(), UV_RUN_DEFAULT);
    uv_loop_close(uv_default_loop());
    uv_library_shutdown();
    slogi("All threads stopped...");

    fprintf(stdout, "\n");
    fflush(stdout);
    restore_stdin_terminal(&canonicalmode, stdin_was_raw);
    free(poll);

    free(baton);
    free(simdata);
    free(simmap);

    return 0;
}

static void tester_fill_corners(double* dest, double value)
{
    int i;
    for (i = 0; i < TEST_WHEEL_COUNT; i++)
    {
        dest[i] = value;
    }
}

static void tester_set_gear(SimData* simdata, uint32_t gear, char gear_char)
{
    simdata->gear = gear;
    simdata->gearc[0] = gear_char;
    simdata->gearc[1] = '\0';
}

static void tester_set_tyre_fallback(SimData* simdata, double rps)
{
    tester_fill_corners(simdata->tyreRPS, rps);
    simdata->tyrediameter[0] = TEST_TYRE_DIAMETER_FL;
    simdata->tyrediameter[1] = TEST_TYRE_DIAMETER_FR;
    simdata->tyrediameter[2] = TEST_TYRE_DIAMETER_RL;
    simdata->tyrediameter[3] = TEST_TYRE_DIAMETER_RR;
}

static void tester_clear_effects(SimData* simdata)
{
    simdata->gas = 0.0;
    simdata->brake = 0.0;
    simdata->abs = TEST_ABS_OFF;
    tester_fill_corners(simdata->tyreslipratio, 0.0);
    tester_fill_corners(simdata->braketemp, 0.0);
    tester_fill_corners(simdata->suspvelocity, 0.0);
    tester_fill_corners(simdata->tyreRPS, 0.0);
    tester_fill_corners(simdata->tyrediameter, TEST_TYRE_DIAMETER_UNSET);
}

void set_basic_simdata(SimData* simdata)
{
    snprintf(simdata->car, sizeof(simdata->car), "%s", TEST_CAR_NAME);
    tester_set_gear(simdata, SIMAPI_GEAR_NEUTRAL, TEST_GEAR_CHAR_NEUTRAL);
    simdata->velocity = TEST_VELOCITY_CRUISE;
    simdata->rpms = TEST_RPM_IDLE;
    simdata->maxrpm = TEST_RPM_MAX;
    simdata->idlerpm = TEST_RPM_IDLE;
    tester_clear_effects(simdata);
    simdata->Xvelocity = 0.0;
    simdata->Yvelocity = TEST_YVELOCITY;
    simdata->Zvelocity = 0.0;
}

static void tester_set_rolling(SimData* simdata)
{
    simdata->Yvelocity = TEST_YVELOCITY;
    simdata->Zvelocity = 0.0;
}

void set_wheel_spin_simdata(SimData* simdata)
{
    tester_set_rolling(simdata);
    simdata->velocity = TEST_VELOCITY_SPIN;
    simdata->gas = TEST_PEDAL_APPLIED;
    simdata->brake = 0.0;
    simdata->abs = TEST_ABS_OFF;
    tester_fill_corners(simdata->tyreslipratio, TEST_SLIP_SPIN);
    tester_fill_corners(simdata->braketemp, 0.0);
    tester_set_tyre_fallback(simdata, TEST_TYRE_RPS_SPIN);
}

void set_wheel_lock_simdata(SimData* simdata)
{
    tester_set_rolling(simdata);
    simdata->velocity = TEST_VELOCITY_LOCK;
    simdata->gas = 0.0;
    simdata->brake = TEST_PEDAL_APPLIED;
    simdata->abs = TEST_ABS_OFF;
    tester_fill_corners(simdata->tyreslipratio, TEST_SLIP_LOCK);
    tester_fill_corners(simdata->braketemp, TEST_BRAKE_TEMP_HOT);
    tester_set_tyre_fallback(simdata, TEST_TYRE_RPS_LOCK);
}

static void tester_set_brake_heat(SimData* simdata)
{
    tester_set_rolling(simdata);
    simdata->velocity = TEST_VELOCITY_LOCK;
    simdata->gas = 0.0;
    simdata->brake = TEST_PEDAL_APPLIED;
    simdata->abs = TEST_ABS_OFF;
    tester_fill_corners(simdata->tyreslipratio, 0.0);
    tester_fill_corners(simdata->braketemp, TEST_BRAKE_TEMP_HOT);
    tester_fill_corners(simdata->tyreRPS, 0.0);
    tester_fill_corners(simdata->tyrediameter, TEST_TYRE_DIAMETER_UNSET);
}

static void tester_set_abs(SimData* simdata)
{
    tester_set_rolling(simdata);
    simdata->velocity = TEST_VELOCITY_LOCK;
    simdata->gas = 0.0;
    simdata->brake = TEST_PEDAL_APPLIED;
    simdata->abs = TEST_ABS_ACTIVE;
    tester_fill_corners(simdata->tyreslipratio, TEST_SLIP_ABS_A);
    tester_fill_corners(simdata->braketemp, TEST_BRAKE_TEMP_HOT);
    tester_set_tyre_fallback(simdata, TEST_TYRE_RPS_LOCK);
}

// Drives every initialized device with the current test simdata, then
// mirrors that same simdata into the SIMAPI.DAT shared memory segment (if
// available) so external tools - dashboards, telemetry viewers, anything
// that reads SIMAPI.DAT the same way a real running sim would populate it -
// can follow cargopit's own test sequence instead of seeing nothing.
static void update_devices(SimDevice* devices, int numdevices, SimData* simdata, SimMap* testsimmap)
{
    for (int x = 0; x < numdevices; x++)
    {
        if (devices[x].initialized == true)
        {
            devices[x].update(&devices[x], simdata);
        }
    }
    if (testsimmap != NULL && testsimmap->addr != NULL)
    {
        simdata->mtick++;
        memcpy(testsimmap->addr, simdata, sizeof(SimData));
    }
}

static int tester_enter_raw_stdin(struct termios* saved)
{
    struct termios raw;
    if (!isatty(STDIN_FILENO))
    {
        return 0;
    }
    tcgetattr(STDIN_FILENO, saved);
    raw = *saved;
    raw.c_lflag &= (~ICANON & ~ECHO);
    raw.c_cc[VMIN] = 0;
    raw.c_cc[VTIME] = 0;
    tcsetattr(STDIN_FILENO, TCSANOW, &raw);
    return 1;
}

static void tester_restore_stdin(int raw_applied, const struct termios* saved)
{
    if (!raw_applied)
    {
        return;
    }
    tcsetattr(STDIN_FILENO, TCSANOW, saved);
}

static void tester_set_identity(SimData* simdata)
{
    simdata->simon = true;
    simdata->simstatus = SIMAPI_STATUS_ACTIVEPLAY;
    simdata->simapi = SIMULATORAPI_SIMAPI_TEST;
    simdata->simexe = SIMULATOREXE_SIMAPI_TEST_NONE;
    simdata->simapiversion = SIMAPI_VERSION;
}

static void tester_enable_log_flush(void)
{
    slog_config_t cfg;
    slog_config_get(&cfg);
    cfg.nFlush = 1;
    slog_config_set(&cfg);
    setvbuf(stdout, NULL, _IONBF, 0);
    setvbuf(stderr, NULL, _IONBF, 0);
    fflush(stdout);
    fflush(stderr);
}

static void tester_announce(const char* msg)
{
    slogi("%s%s", TEST_STEP_PREFIX, msg);
    fflush(stdout);
}

static void tester_announce_named(const char* action, const char* name)
{
    slogi("%s%s %s", TEST_STEP_PREFIX, action, name);
    fflush(stdout);
}

static int tester_hold_ticks(unsigned int hold_us)
{
    return (int)(hold_us / TEST_TICK_US);
}

static void tester_on_signal(int signum)
{
    (void)signum;
    tester_abort = 1;
}

static void tester_install_signals(void)
{
    struct sigaction action;

    memset(&action, 0, sizeof(action));
    action.sa_handler = tester_on_signal;
    sigemptyset(&action.sa_mask);
    sigaction(SIGINT, &action, NULL);
    sigaction(SIGTERM, &action, NULL);
}

static int tester_is_quit_key(char ch)
{
    return ch == TEST_KEY_QUIT || ch == TEST_KEY_QUIT_UPPER || ch == TEST_KEY_ESC;
}

static int tester_poll_quit_key(void)
{
    struct pollfd pfd;
    char ch;
    ssize_t nread;

    if (tester_stdin_raw == 0)
    {
        return 0;
    }
    pfd.fd = STDIN_FILENO;
    pfd.events = POLLIN;
    pfd.revents = 0;
    if (poll(&pfd, TEST_STDIN_POLL_FDS, TEST_STDIN_POLL_MS) <= 0)
    {
        return 0;
    }
    nread = read(STDIN_FILENO, &ch, TEST_STDIN_READ_BYTES);
    if (nread != TEST_STDIN_READ_BYTES)
    {
        return 0;
    }
    if (!tester_is_quit_key(ch))
    {
        return 0;
    }
    tester_abort = 1;
    return 1;
}

static int tester_should_stop(void)
{
    if (tester_abort != 0)
    {
        return 1;
    }
    return tester_poll_quit_key();
}

typedef void (*tester_mod_fn)(SimData* simdata, int tick);

static void tester_drive(
    SimDevice* devices,
    int numdevices,
    SimData* simdata,
    SimMap* testsimmap,
    int ticks,
    tester_mod_fn modify)
{
    int tick;
    for (tick = 0; tick < ticks; tick++)
    {
        if (tester_should_stop())
        {
            return;
        }
        if (modify != NULL)
        {
            modify(simdata, tick);
        }
        update_devices(devices, numdevices, simdata, testsimmap);
        usleep(TEST_TICK_US);
    }
}

static void tester_mod_abs(SimData* simdata, int tick)
{
    double slip = TEST_SLIP_ABS_A;
    if ((tick / TEST_ABS_PULSE_TICKS) % 2 == 1)
    {
        slip = TEST_SLIP_ABS_B;
    }
    tester_fill_corners(simdata->tyreslipratio, slip);
}

static void tester_mod_suspension(SimData* simdata, int tick)
{
    double vel = TEST_SUSP_VEL_A;
    if ((tick % 2) == 1)
    {
        vel = TEST_SUSP_VEL_B;
    }
    tester_fill_corners(simdata->suspvelocity, vel);
}

static uint32_t tester_pulse_gear;
static char tester_pulse_gear_char;

static void tester_mod_gear(SimData* simdata, int tick)
{
    if ((tick / TEST_GEAR_PULSE_TICKS) % 2 == 0)
    {
        tester_set_gear(simdata, tester_pulse_gear, tester_pulse_gear_char);
        return;
    }
    tester_set_gear(simdata, SIMAPI_GEAR_NEUTRAL, TEST_GEAR_CHAR_NEUTRAL);
}

static const char* tester_effect_name(VibrationEffectType effect)
{
    switch (effect)
    {
        case EFFECT_GEARSHIFT:
            return "gear";
        case EFFECT_TYRELOCK:
            return "tyre lock";
        case EFFECT_ABSBRAKES:
            return "ABS";
        case EFFECT_TYRESLIP:
            return "tyre slip";
        case EFFECT_SUSPENSION:
            return "suspension";
        default:
            return "effect";
    }
}

static int tester_has_effect(SimDevice* devices, int numdevices, VibrationEffectType effect)
{
    int i;
    for (i = 0; i < numdevices; i++)
    {
        if (devices[i].initialized == false)
        {
            continue;
        }
        if (devices[i].hapticeffect.effecttype != effect)
        {
            continue;
        }
        return 1;
    }
    return 0;
}

static int tester_is_haptic_effect_device(const SimDevice* device)
{
    if (device->initialized == false)
    {
        return 0;
    }
    if (device->type == SIMDEV_SOUND)
    {
        return 1;
    }
    switch (device->hapticeffect.effecttype)
    {
        case EFFECT_GEARSHIFT:
        case EFFECT_TYRELOCK:
        case EFFECT_TYRESLIP:
        case EFFECT_ABSBRAKES:
        case EFFECT_SUSPENSION:
            return 1;
        default:
            return 0;
    }
}

static const char* tester_device_label(const SimDevice* device)
{
    if (tester_is_haptic_effect_device(device))
    {
        return tester_effect_name(device->hapticeffect.effecttype);
    }
    if (device->type == SIMDEV_SERIAL)
    {
        return TEST_LABEL_SERIAL_LIGHTS;
    }
    if (device->type == SIMDEV_USB)
    {
        return TEST_LABEL_USB_LIGHTS;
    }
    return TEST_LABEL_DEVICE;
}

static void tester_warn_if_missing(SimDevice* devices, int numdevices, VibrationEffectType effect)
{
    if (tester_has_effect(devices, numdevices, effect))
    {
        return;
    }
    slogw("No enabled %s device in this config; that test step will not shake", tester_effect_name(effect));
}

static void tester_log_slip_play(
    SimDevice* devices,
    int numdevices,
    SimData* simdata,
    VibrationEffectType effect)
{
    int i;
    for (i = 0; i < numdevices; i++)
    {
        double play;
        if (devices[i].initialized == false)
        {
            continue;
        }
        if (devices[i].hapticeffect.effecttype != effect)
        {
            continue;
        }
        /* Probe a copy so logging does not advance the device's own filter state. */
        HapticEffect probe = devices[i].hapticeffect;
        play = slipeffect(simdata, &probe);
        slogi(
            "%s%s play=%f threshold=%f brake=%f gas=%f yvel=%f slip=%f",
            TEST_STEP_PREFIX,
            tester_effect_name(effect),
            play,
            devices[i].hapticeffect.threshold,
            simdata->brake,
            simdata->gas,
            simdata->Yvelocity,
            simdata->tyreslipratio[0]);
        return;
    }
    tester_warn_if_missing(devices, numdevices, effect);
}

static uint32_t tester_next_rpm(uint32_t rpm, uint32_t to, int signed_step)
{
    int next = (int)rpm + signed_step;
    if (signed_step > 0 && next >= (int)to)
    {
        return to;
    }
    if (signed_step < 0 && next <= (int)to)
    {
        return to;
    }
    return (uint32_t)next;
}

static void tester_sweep_rpm(
    SimDevice* devices,
    int numdevices,
    SimData* simdata,
    SimMap* testsimmap,
    uint32_t from,
    uint32_t to,
    int step)
{
    int signed_step = step;
    uint32_t rpm = from;
    if (to < from)
    {
        signed_step = -step;
    }
    while (1)
    {
        if (tester_should_stop())
        {
            return;
        }
        simdata->rpms = rpm;
        update_devices(devices, numdevices, simdata, testsimmap);
        usleep(TEST_TICK_US);
        if (rpm == to)
        {
            return;
        }
        rpm = tester_next_rpm(rpm, to, signed_step);
    }
}

static SimMap* tester_open_map(SimData* simdata)
{
    if (simdata == NULL)
    {
        return NULL;
    }

    SimMap* testsimmap = calloc(1, sizeof(*testsimmap));
    if (testsimmap == NULL)
    {
        return NULL;
    }

    int shmerr;
    shmerr = simapi_universalmap_open(testsimmap, simdata);
    if (shmerr == SIMAPI_ERROR_NONE)
    {
        return testsimmap;
    }
    slog_warn("Could not open shared telemetry memory for test mode (error %i) - test sequence will still drive local devices, but external tools won't see it", shmerr);
    free(testsimmap);
    return NULL;
}

static void tester_close_map(
    SimMap* testsimmap,
    SimDevice* devices,
    int numdevices,
    SimData* simdata)
{
    if (simdata == NULL)
    {
        return;
    }

    simdata->simon = false;
    simdata->simstatus = SIMAPI_STATUS_OFF;
    update_devices(devices, numdevices, simdata, testsimmap);
    if (testsimmap != NULL)
    {
        if (testsimmap->addr != NULL)
        {
            munmap(testsimmap->addr, sizeof(SimData));
        }
        if (testsimmap->fd >= 0)
        {
            close(testsimmap->fd);
        }
        free(testsimmap);
    }
}

static void tester_phase(
    SimDevice* devices,
    int numdevices,
    SimData* simdata,
    SimMap* testsimmap,
    const char* msg,
    unsigned int hold_us,
    tester_mod_fn modify)
{
    if (tester_should_stop())
    {
        return;
    }
    tester_announce(msg);
    tester_drive(devices, numdevices, simdata, testsimmap, tester_hold_ticks(hold_us), modify);
}

static void tester_phase_gear(
    SimDevice* devices,
    int numdevices,
    SimData* simdata,
    SimMap* testsimmap,
    const char* msg,
    uint32_t gear,
    char gear_char)
{
    tester_pulse_gear = gear;
    tester_pulse_gear_char = gear_char;
    tester_set_gear(simdata, gear, gear_char);
    tester_phase(devices, numdevices, simdata, testsimmap, msg, TEST_PHASE_HOLD_US, tester_mod_gear);
}

static void tester_reset_idle(SimData* simdata)
{
    tester_clear_effects(simdata);
    simdata->velocity = 0;
    simdata->rpms = TEST_RPM_IDLE;
    simdata->playerflag = SIMAPI_FLAG_GREEN;
    tester_set_gear(simdata, SIMAPI_GEAR_NEUTRAL, TEST_GEAR_CHAR_NEUTRAL);
}

static void tester_run_engine(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    simdata->gas = TEST_GAS_CRUISE;
    tester_announce(TEST_MSG_REV);
    tester_sweep_rpm(device, 1, simdata, testsimmap, TEST_RPM_IDLE, TEST_RPM_MAX, TEST_RPM_SWEEP_STEP);
    tester_sweep_rpm(device, 1, simdata, testsimmap, TEST_RPM_MAX, TEST_RPM_IDLE, TEST_RPM_SWEEP_STEP);
    simdata->rpms = TEST_RPM_IDLE;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_RPM_IDLE, TEST_PHASE_HOLD_US, NULL);
}

static void tester_run_gear(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    tester_phase_gear(device, 1, simdata, testsimmap, TEST_MSG_FIRST, SIMAPI_GEAR_FIRST, TEST_GEAR_CHAR_FIRST);
    tester_phase_gear(device, 1, simdata, testsimmap, TEST_MSG_SECOND, SIMAPI_GEAR_SECOND, TEST_GEAR_CHAR_SECOND);
    tester_phase_gear(device, 1, simdata, testsimmap, TEST_MSG_THIRD, SIMAPI_GEAR_THIRD, TEST_GEAR_CHAR_THIRD);
    tester_phase_gear(device, 1, simdata, testsimmap, TEST_MSG_FOURTH, SIMAPI_GEAR_FOURTH, TEST_GEAR_CHAR_FOURTH);
}

static void tester_run_spin(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    set_wheel_spin_simdata(simdata);
    tester_log_slip_play(device, 1, simdata, EFFECT_TYRESLIP);
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_SPIN, TEST_PHASE_HOLD_US, NULL);
}

static void tester_run_lock(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    set_wheel_lock_simdata(simdata);
    tester_log_slip_play(device, 1, simdata, EFFECT_TYRELOCK);
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_LOCK, TEST_PHASE_HOLD_US, NULL);
}

static void tester_run_abs(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    tester_set_abs(simdata);
    tester_log_slip_play(device, 1, simdata, EFFECT_ABSBRAKES);
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_ABS, TEST_PHASE_HOLD_US, tester_mod_abs);
}

static void tester_run_suspension(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    simdata->gas = TEST_GAS_CRUISE;
    tester_fill_corners(simdata->suspvelocity, TEST_SUSP_VEL_A);
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_SUSPENSION, TEST_PHASE_HOLD_US, tester_mod_suspension);
}

static void tester_run_haptic(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    switch (device->hapticeffect.effecttype)
    {
        case EFFECT_ENGINERPM:
            tester_run_engine(device, simdata, testsimmap);
            return;
        case EFFECT_GEARSHIFT:
            tester_run_gear(device, simdata, testsimmap);
            return;
        case EFFECT_TYRESLIP:
            tester_run_spin(device, simdata, testsimmap);
            return;
        case EFFECT_TYRELOCK:
            tester_run_lock(device, simdata, testsimmap);
            return;
        case EFFECT_ABSBRAKES:
            tester_run_abs(device, simdata, testsimmap);
            return;
        case EFFECT_SUSPENSION:
            tester_run_suspension(device, simdata, testsimmap);
            return;
        default:
            return;
    }
}

static void tester_run_lights(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    simdata->gas = TEST_GAS_CRUISE;
    tester_announce(TEST_MSG_REV);
    tester_sweep_rpm(device, 1, simdata, testsimmap, TEST_RPM_IDLE, TEST_RPM_MAX, TEST_RPM_SWEEP_STEP);
    tester_sweep_rpm(device, 1, simdata, testsimmap, TEST_RPM_MAX, TEST_RPM_IDLE, TEST_RPM_SWEEP_STEP);
    simdata->rpms = TEST_RPM_IDLE;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_RPM_IDLE, TEST_PHASE_HOLD_US, NULL);

    simdata->playerflag = SIMAPI_FLAG_GREEN;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_GREEN, TEST_PHASE_HOLD_US, NULL);
    simdata->playerflag = SIMAPI_FLAG_YELLOW;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_YELLOW, TEST_PHASE_HOLD_US, NULL);
    simdata->playerflag = SIMAPI_FLAG_BLUE;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_BLUE, TEST_PHASE_HOLD_US, NULL);
    simdata->playerflag = SIMAPI_FLAG_RED;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_RED, TEST_PHASE_HOLD_US, NULL);

    tester_set_brake_heat(simdata);
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_BUTTONS, TEST_PHASE_HOLD_US, NULL);

    simdata->velocity = TEST_VELOCITY_SLOW;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_SPEED_SLOW, TEST_PHASE_HOLD_US, NULL);
    simdata->velocity = TEST_VELOCITY_FAST;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_SPEED_FAST, TEST_PHASE_HOLD_US, NULL);
    simdata->velocity = TEST_VELOCITY_TOP;
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_SPEED_TOP, TEST_PHASE_HOLD_US, NULL);
}

static void tester_idle_all(
    SimDevice* devices,
    int numdevices,
    SimData* simdata,
    SimMap* testsimmap)
{
    tester_reset_idle(simdata);
    update_devices(devices, numdevices, simdata, testsimmap);
}

static void tester_run_one_device(SimDevice* device, SimData* simdata, SimMap* testsimmap)
{
    if (device->initialized == false)
    {
        return;
    }
    if (tester_should_stop())
    {
        return;
    }

    tester_announce_named(TEST_MSG_STARTING, tester_device_label(device));
    set_basic_simdata(simdata);
    if (tester_is_haptic_effect_device(device))
    {
        tester_run_haptic(device, simdata, testsimmap);
    }
    else
    {
        tester_run_lights(device, simdata, testsimmap);
    }
    if (tester_should_stop())
    {
        return;
    }
    tester_reset_idle(simdata);
    tester_phase(device, 1, simdata, testsimmap, TEST_MSG_COAST, TEST_IDLE_HOLD_US, NULL);
    tester_announce_named(TEST_MSG_FINISHED, tester_device_label(device));
}

int tester(SimDevice* devices, int numdevices)
{
    SimData* simdata;
    SimMap* testsimmap;
    struct termios canonicalmode;
    int raw_applied;
    int i;

    tester_abort = 0;
    tester_stdin_raw = 0;
    tester_install_signals();
    tester_enable_log_flush();
    slogi(TEST_STEP_PREFIX TEST_MSG_PREPARING, numdevices);

    simdata = calloc(1, sizeof(*simdata));
    if (simdata == NULL)
    {
        sloge("Could not allocate test telemetry state");
        return 1;
    }
    tester_set_identity(simdata);
    testsimmap = tester_open_map(simdata);
    raw_applied = tester_enter_raw_stdin(&canonicalmode);
    tester_stdin_raw = raw_applied;

    for (i = 0; i < numdevices; i++)
    {
        if (tester_should_stop())
        {
            break;
        }
        tester_run_one_device(&devices[i], simdata, testsimmap);
    }

    if (tester_should_stop())
    {
        tester_announce(TEST_MSG_STOPPED);
        tester_idle_all(devices, numdevices, simdata, testsimmap);
    }

    tester_restore_stdin(raw_applied, &canonicalmode);
    tester_close_map(testsimmap, devices, numdevices, simdata);
    free(simdata);
    return 0;
}

static void free_loaded_device_settings(DeviceSettings* ds, int configureddevices)
{
    int i;
    if (ds == NULL)
    {
        return;
    }
    for (i = 0; i < configureddevices; i++)
    {
        settingsfree(ds[i]);
    }
    free(ds);
}

int run_hardware_test(CargopitSettings* ms, int config_index, int device_index)
{
    int confignum;
    DeviceSettings* ds;
    int configureddevices;
    int numdevices;
    SimDevice* simdevices;
    SimInfo* siminfo;
    int error;
    int i;

    tester_enable_log_flush();
    confignum = resolve_config_index(ms->config_str, config_index);
    if (confignum < 0)
    {
        sloge("Could not resolve config index for test");
        return CARGOPIT_ERROR_INVALID_DEV;
    }

    ds = NULL;
    configureddevices = 0;
    numdevices = load_devices_for_test(
        ms->config_str, confignum, device_index, ms, &ds, &configureddevices);
    slogd("loading confignum %i, with %i devices.", confignum, configureddevices);
    if (numdevices <= 0)
    {
        sloge("No devices loaded for test");
        free_loaded_device_settings(ds, configureddevices);
        return CARGOPIT_ERROR_INVALID_DEV;
    }

    simdevices = malloc(numdevices * sizeof(SimDevice));
    siminfo = malloc(sizeof(SimInfo));
    simapi_set_faux_siminfo(siminfo);
    (void)devinit(simdevices, siminfo, numdevices, ds, ms);
    free_loaded_device_settings(ds, configureddevices);

    error = tester(simdevices, numdevices);
    for (i = 0; i < numdevices; i++)
    {
        if (simdevices[i].initialized == true)
        {
            simdevices[i].free(&simdevices[i]);
        }
    }
    free(simdevices);
    free(siminfo);
    return error;
}
