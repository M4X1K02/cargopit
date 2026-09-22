#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/mman.h>
#include <poll.h>
#include <termios.h>
#include <signal.h>
#include <pthread.h>
#include <uv.h>

#include "gameloop.h"
#include "loopdata.h"
#include "../helper/confighelper.h"
#include "../devices/simdevice.h"
#include "../devices/hapticeffect.h"
#include "../simulatorapi/simapi/simapi/simdata.h"
#include "../simulatorapi/simapi/simapi/simmapper.h"
#include "../simulatorapi/simapi/simapi/simmap.h"
#include "../slog/slog.h"

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

static volatile sig_atomic_t tester_abort;
static int tester_stdin_raw;

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

#define TEST_LOOP_START_DELAY_MS 1000
#define TEST_LOOP_INTERVAL_MS    16

static uv_loop_t* test_loop;
static pthread_t test_loop_thread;
static uv_timer_t test_device_timer;
static uv_async_t test_stop_async;
static device_loop_data test_baton;
static SimDevice* test_simdevice;
static SimInfo test_siminfo;

SimData* get_test_simdata(void)
{
    if (test_loop == NULL)
    {
        return NULL;
    }
    return test_baton.simdata;
}

static void test_device_tick(uv_timer_t* handle)
{
    device_loop_data* d = (device_loop_data*) handle->data;
    d->simdevice->update(d->simdevice, d->simdata);
}

static void on_test_stop(uv_async_t* handle)
{
    uv_timer_stop(&test_device_timer);
    uv_close((uv_handle_t*) &test_device_timer, NULL);
    uv_close((uv_handle_t*) handle, NULL);
}

static void* run_test_loop(void* arg)
{
    uv_run((uv_loop_t*) arg, UV_RUN_DEFAULT);
    return NULL;
}

static void free_test_loop(void)
{
    free(test_simdevice);
    free(test_loop);
    test_simdevice = NULL;
    test_loop = NULL;
}

int start_test(test_loop_args* args)
{
    if (args == NULL || args->simdata == NULL || test_loop != NULL)
    {
        return CARGOPIT_ERROR_UNKNOWN;
    }
    test_simdevice = calloc(1, sizeof(SimDevice));
    test_loop = malloc(sizeof(uv_loop_t));
    if (test_simdevice == NULL || test_loop == NULL || uv_loop_init(test_loop) != 0)
    {
        free_test_loop();
        return CARGOPIT_ERROR_UNKNOWN;
    }

    simapi_set_faux_siminfo(&test_siminfo);
    devinit(test_simdevice, &test_siminfo, 1, args->ds, args->ms);
    test_baton.simdevice = test_simdevice;
    test_baton.simdata = args->simdata;

    uv_timer_init(test_loop, &test_device_timer);
    test_device_timer.data = &test_baton;
    uv_async_init(test_loop, &test_stop_async, on_test_stop);
    if (test_simdevice->initialized)
    {
        uv_timer_start(&test_device_timer, test_device_tick, TEST_LOOP_START_DELAY_MS, TEST_LOOP_INTERVAL_MS);
    }
    return pthread_create(&test_loop_thread, NULL, run_test_loop, test_loop);
}

int cargopit_testloop_stop(void)
{
    if (test_loop == NULL)
    {
        return CARGOPIT_ERROR_UNKNOWN;
    }
    uv_async_send(&test_stop_async);
    pthread_join(test_loop_thread, NULL);
    uv_loop_close(test_loop);
    if (test_simdevice->initialized)
    {
        test_simdevice->free(test_simdevice);
    }
    free_test_loop();
    slogi("All threads stopped...");
    return 0;
}
