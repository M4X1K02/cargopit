#include "capture_log.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "simdevice.h"
#include "sound.h"
#include "slog.h"

#define SIMDATA_CONTRACT_BYTES 46044
#define STREAM_MAGIC "CPITSCN1"
#define STREAM_MAGIC_LEN 8
#define CAPTURE_PORT "/dev/ttyPARITY0"
#define CAPTURE_BAUD_DEFAULT 115200
#define CAPTURE_BAUD_FIRST_OPEN 9600
#define CAPTURE_SHIFT_LIGHTS 8
#define CAPTURE_TACH_POINTS 9
#define CAPTURE_HAPTIC_FREQUENCY 40
#define CAPTURE_HAPTIC_AMPLITUDE 100
#define CAPTURE_HAPTIC_THRESHOLD 0.2
#define CAPTURE_HAPTIC_DURATION_S 0.10
#define CAPTURE_SOUND_CHANNELS 2
#define CAPTURE_SOUND_VOLUME 100
#define CAPTURE_FAN_POWER 0.5
#define CAPTURE_AMP_FACTOR 1.0

static uint32_t tach_rpm_points[CAPTURE_TACH_POINTS];
static uint32_t tach_pulse_points[CAPTURE_TACH_POINTS];

static void fail(const char* message)
{
    fprintf(stderr, "parity capture: %s\n", message);
}

static int read_stream(const char* path, SimData** frames, uint32_t* count)
{
    FILE* file;
    char magic[STREAM_MAGIC_LEN];
    uint32_t frame_count = 0;
    SimData* data;

    file = fopen(path, "rb");
    if (file == NULL)
    {
        fail("could not open scenario");
        return -1;
    }
    if (fread(magic, 1, STREAM_MAGIC_LEN, file) != STREAM_MAGIC_LEN ||
        memcmp(magic, STREAM_MAGIC, STREAM_MAGIC_LEN) != 0 ||
        fread(&frame_count, sizeof(frame_count), 1, file) != 1 ||
        frame_count == 0)
    {
        fclose(file);
        fail("scenario header is invalid");
        return -1;
    }
    data = calloc(frame_count, sizeof(*data));
    if (data == NULL || fread(data, sizeof(*data), frame_count, file) != frame_count)
    {
        free(data);
        fclose(file);
        fail("scenario frames are short");
        return -1;
    }
    if (sizeof(SimData) != SIMDATA_CONTRACT_BYTES)
    {
        free(data);
        fclose(file);
        fail("SimData size is not 46044");
        return -1;
    }
    fclose(file);
    *frames = data;
    *count = frame_count;
    return 0;
}

static void prepare_tach_tables(int granularity)
{
    int i;

    for (i = 0; i < CAPTURE_TACH_POINTS; i++)
    {
        tach_rpm_points[i] = (uint32_t)(i * 1000);
        tach_pulse_points[i] = (uint32_t)((i + 1) * granularity);
    }
}

static void base_settings(DeviceSettings* ds, CargopitSettings* ms, SimInfo* info)
{
    memset(ds, 0, sizeof(*ds));
    memset(ms, 0, sizeof(*ms));
    memset(info, 0, sizeof(*info));
    ds->enabled = true;
    ds->is_valid = true;
    ds->fps = 60;
    info->SimSupportsHapticEffects = true;
    info->SimSupportsBasicTelemetry = true;
    ms->tyre_diameter_config = NULL;
}

static void set_haptic(DeviceSettings* ds, VibrationEffectType effect)
{
    ds->has_haptic_effects = true;
    ds->hapticsettings.effect_type = effect;
    ds->hapticsettings.tyre = ALLFOUR;
    ds->hapticsettings.frequency = CAPTURE_HAPTIC_FREQUENCY;
    ds->hapticsettings.amplitude = CAPTURE_HAPTIC_AMPLITUDE;
    ds->hapticsettings.threshold = CAPTURE_HAPTIC_THRESHOLD;
    ds->hapticsettings.duration = CAPTURE_HAPTIC_DURATION_S;
    ds->hapticsettings.motorposition = MOTOR_1;
}

static void set_serial_common(DeviceSettings* ds, const char* port, uint32_t baud)
{
    ds->dev_type = SIMDEV_SERIAL;
    ds->dev = strdup(port);
    ds->serialdevsettings.baud = baud;
    ds->serialdevsettings.numlights = CAPTURE_SHIFT_LIGHTS;
    ds->serialdevsettings.numleds = CAPTURE_SIMLED_LED_COUNT;
    ds->serialdevsettings.startled = 1;
    ds->serialdevsettings.endled = CAPTURE_SIMLED_LED_COUNT;
    ds->serialdevsettings.fanpower = CAPTURE_FAN_POWER;
    ds->serialdevsettings.ampfactor = CAPTURE_AMP_FACTOR;
}

static SimDevice* open_named(const char* name, DeviceSettings* ds, CargopitSettings* ms, SimInfo* info)
{
    const char* lua_path = getenv("PARITY_LUA_FIXTURE");

    base_settings(ds, ms, info);
    if (strcmp(name, "revburner") == 0 || strcmp(name, "revburner_granularity_2") == 0 ||
        strcmp(name, "revburner_granularity_4") == 0 || strcmp(name, "revburner_pulses") == 0)
    {
        int granularity = 1;
        if (strcmp(name, "revburner_granularity_2") == 0)
        {
            granularity = 2;
        }
        if (strcmp(name, "revburner_granularity_4") == 0)
        {
            granularity = 4;
        }
        prepare_tach_tables(granularity);
        ds->dev_type = SIMDEV_USB;
        ds->dev_subtype = SIMDEVTYPE_TACHOMETER;
        ds->dev_subsubtype = SIMDEVSUBTYPE_REVBURNERTACHOMETER;
        ds->tachsettings.use_pulses = strcmp(name, "revburner_pulses") == 0;
        ds->tachsettings.granularity = granularity;
        ds->tachsettings.size = CAPTURE_TACH_POINTS;
        ds->tachsettings.rpms_array = tach_rpm_points;
        ds->tachsettings.pulses_array = tach_pulse_points;
        return (SimDevice*)new_usb_device(ds, ms, info);
    }
    if (strcmp(name, "cammus_c5") == 0 || strcmp(name, "logitech_g29") == 0 ||
        strcmp(name, "cammus_c12") == 0 || strcmp(name, "simagic_gt_neo") == 0)
    {
        ds->dev_type = SIMDEV_USB;
        ds->dev_subtype = SIMDEVTYPE_USBWHEEL;
        if (strcmp(name, "cammus_c5") == 0)
        {
            ds->dev_subsubtype = SIMDEVSUBTYPE_CAMMUSC5;
        }
        if (strcmp(name, "logitech_g29") == 0)
        {
            ds->dev_subsubtype = SIMDEVSUBTYPE_LOGITECH_G29;
        }
        if (strcmp(name, "cammus_c12") == 0)
        {
            ds->dev_subsubtype = SIMDEVSUBTYPE_CAMMUSC12;
        }
        if (strcmp(name, "simagic_gt_neo") == 0)
        {
            ds->dev_subsubtype = SIMDEVSUBTYPE_SIMAGICGTNEO;
            ds->has_config = true;
            ds->specific_config_file = strdup(lua_path != NULL ? lua_path : "");
        }
        return (SimDevice*)new_usb_device(ds, ms, info);
    }
    if (strcmp(name, "csl_elite_v3") == 0 || strcmp(name, "simagic_p1000") == 0 ||
        strcmp(name, "simnet_pedals") == 0)
    {
        ds->dev_type = SIMDEV_USB;
        ds->dev_subtype = SIMDEVTYPE_USBWHEEL;
        set_haptic(ds, EFFECT_TYRESLIP);
        if (strcmp(name, "csl_elite_v3") == 0)
        {
            ds->dev_subsubtype = SIMDEVSUBTYPE_CSLELITEV3PEDALS;
        }
        if (strcmp(name, "simagic_p1000") == 0)
        {
            ds->dev_subsubtype = SIMDEVSUBTYPE_SIMAGICP1000PEDALS;
        }
        if (strcmp(name, "simnet_pedals") == 0)
        {
            ds->dev_subsubtype = SIMDEVSUBTYPE_SIMNETPEDALS;
        }
        return (SimDevice*)new_usb_device(ds, ms, info);
    }
    if (strcmp(name, "sound_engine") == 0 || strcmp(name, "sound_gear") == 0 ||
        strcmp(name, "sound_slip") == 0 || strcmp(name, "sound_lock") == 0 ||
        strcmp(name, "sound_abs") == 0 || strcmp(name, "sound_suspension") == 0)
    {
        ds->dev_type = SIMDEV_SOUND;
        ds->dev = strdup("parity-shaker");
        ds->sounddevsettings.volume = CAPTURE_SOUND_VOLUME;
        ds->sounddevsettings.channels = CAPTURE_SOUND_CHANNELS;
        ds->sounddevsettings.channelmask = sound_channel_mask_all(CAPTURE_SOUND_CHANNELS);
        ds->sounddevsettings.noise = 0;
        if (strcmp(name, "sound_engine") == 0)
        {
            set_haptic(ds, EFFECT_ENGINERPM);
        }
        if (strcmp(name, "sound_gear") == 0)
        {
            set_haptic(ds, EFFECT_GEARSHIFT);
        }
        if (strcmp(name, "sound_slip") == 0)
        {
            set_haptic(ds, EFFECT_TYRESLIP);
        }
        if (strcmp(name, "sound_lock") == 0)
        {
            set_haptic(ds, EFFECT_TYRELOCK);
        }
        if (strcmp(name, "sound_abs") == 0)
        {
            set_haptic(ds, EFFECT_ABSBRAKES);
        }
        if (strcmp(name, "sound_suspension") == 0)
        {
            set_haptic(ds, EFFECT_SUSPENSION);
        }
        if (setupsound() != 0)
        {
            return NULL;
        }
        return (SimDevice*)new_sound_device(ds, ms, info);
    }
    if (strcmp(name, "shiftlights") == 0 || strcmp(name, "simwind") == 0 ||
        strcmp(name, "serial_haptic") == 0 || strcmp(name, "simled") == 0 ||
        strcmp(name, "simled_custom") == 0 || strcmp(name, "arduino_custom") == 0 ||
        strcmp(name, "moza_r5") == 0 || strcmp(name, "moza_new") == 0 ||
        strcmp(name, "moza_ks_pro") == 0)
    {
        set_serial_common(ds, CAPTURE_PORT, CAPTURE_BAUD_DEFAULT);
        if (strcmp(name, "shiftlights") == 0)
        {
            ds->dev_subtype = SIMDEVTYPE_SHIFTLIGHTS;
        }
        if (strcmp(name, "simwind") == 0)
        {
            ds->dev_subtype = SIMDEVTYPE_SIMWIND;
        }
        if (strcmp(name, "serial_haptic") == 0)
        {
            ds->dev_subtype = SIMDEVTYPE_SERIALHAPTIC;
            set_haptic(ds, EFFECT_TYRESLIP);
        }
        if (strcmp(name, "simled") == 0)
        {
            ds->dev_subtype = SIMDEVTYPE_SIMLED;
        }
        if (strcmp(name, "simled_custom") == 0 || strcmp(name, "arduino_custom") == 0)
        {
            ds->has_config = true;
            ds->specific_config_file = strdup(lua_path != NULL ? lua_path : "");
            ds->dev_subtype = strcmp(name, "simled_custom") == 0 ? SIMDEVTYPE_SIMLED : SIMDEVTYPE_ARDUINOCUSTOM;
        }
        if (strcmp(name, "moza_r5") == 0)
        {
            ds->dev_subtype = SIMDEVTYPE_SERIALWHEEL;
            ds->dev_subsubtype = SIMDEVSUBTYPE_MOZAR5;
        }
        if (strcmp(name, "moza_new") == 0)
        {
            ds->dev_subtype = SIMDEVTYPE_SERIALWHEEL;
            ds->dev_subsubtype = SIMDEVSUBTYPE_MOZA_NEW;
        }
        if (strcmp(name, "moza_ks_pro") == 0)
        {
            ds->dev_subtype = SIMDEVTYPE_SERIALWHEEL;
            ds->dev_subsubtype = SIMDEVSUBTYPE_MOZA_KS_PRO_WHEEL;
        }
        return (SimDevice*)new_serial_device(ds, ms, info);
    }
    return NULL;
}

static int play_frames(SimDevice* device, SimData* frames, uint32_t count, int render_pcm)
{
    uint32_t i;

    if (device == NULL)
    {
        fail("device did not initialize");
        return -1;
    }
    for (i = 0; i < count; i++)
    {
        capture_set_tick((int)i);
        update(device, &frames[i]);
        if (render_pcm)
        {
            capture_render_pcm();
        }
        capture_clock_advance_ms(CAPTURE_TICK_MS);
    }
    simdevfree(device);
    return 0;
}

static int run_shared_port(SimData* frames, uint32_t count)
{
    DeviceSettings first_settings;
    DeviceSettings second_settings;
    CargopitSettings ms;
    SimInfo info;
    SimDevice* first;
    SimDevice* second;

    base_settings(&first_settings, &ms, &info);
    set_serial_common(&first_settings, CAPTURE_PORT, CAPTURE_BAUD_FIRST_OPEN);
    first_settings.dev_subtype = SIMDEVTYPE_SHIFTLIGHTS;
    first = (SimDevice*)new_serial_device(&first_settings, &ms, &info);
    base_settings(&second_settings, &ms, &info);
    set_serial_common(&second_settings, CAPTURE_PORT, CAPTURE_BAUD_DEFAULT);
    second_settings.dev_subtype = SIMDEVTYPE_SIMWIND;
    second = (SimDevice*)new_serial_device(&second_settings, &ms, &info);
    if (play_frames(first, frames, count, 0) != 0)
    {
        return -1;
    }
    return play_frames(second, frames, count, 0);
}

static int run_device(const char* name, const char* scenario, const char* output)
{
    DeviceSettings settings;
    CargopitSettings ms;
    SimInfo info;
    SimData* frames = NULL;
    uint32_t count = 0;
    SimDevice* device;
    int render_pcm = strncmp(name, "sound_", 6) == 0;
    int status;

    capture_clock_reset();
    capture_log_open(output);
    if (read_stream(scenario, &frames, &count) != 0)
    {
        capture_log_close();
        return -1;
    }
    if (strcmp(name, "shared_serial_port") == 0)
    {
        status = run_shared_port(frames, count);
        free(frames);
        capture_log_close();
        if (render_pcm)
        {
            freesound();
        }
        return status;
    }
    device = open_named(name, &settings, &ms, &info);
    status = play_frames(device, frames, count, render_pcm);
    if (render_pcm)
    {
        freesound();
    }
    free(frames);
    free(settings.dev);
    free(settings.specific_config_file);
    capture_log_close();
    return status;
}

static const char* device_names[] = {
    "revburner",
    "revburner_granularity_2",
    "revburner_granularity_4",
    "revburner_pulses",
    "cammus_c5",
    "cammus_c12",
    "logitech_g29",
    "simagic_gt_neo",
    "csl_elite_v3",
    "simagic_p1000",
    "simnet_pedals",
    "shiftlights",
    "simwind",
    "serial_haptic",
    "simled",
    "simled_custom",
    "arduino_custom",
    "moza_r5",
    "moza_new",
    "moza_ks_pro",
    "shared_serial_port",
    "sound_engine",
    "sound_gear",
    "sound_slip",
    "sound_lock",
    "sound_abs",
    "sound_suspension",
};

static void print_devices(void)
{
    size_t i;
    for (i = 0; i < sizeof(device_names) / sizeof(device_names[0]); i++)
    {
        puts(device_names[i]);
    }
}

static void usage(void)
{
    fprintf(stderr, "usage: parity_c_capture --list\n");
    fprintf(stderr, "       parity_c_capture --device NAME --scenario FILE --output FILE\n");
}

int main(int argc, char** argv)
{
    const char* device = NULL;
    const char* scenario = NULL;
    const char* output = NULL;
    int i;

    slog_init("parity-capture", 0, 0);
    if (argc == 2 && strcmp(argv[1], "--list") == 0)
    {
        print_devices();
        return 0;
    }
    for (i = 1; i < argc; i++)
    {
        if (strcmp(argv[i], "--device") == 0 && i + 1 < argc)
        {
            device = argv[++i];
            continue;
        }
        if (strcmp(argv[i], "--scenario") == 0 && i + 1 < argc)
        {
            scenario = argv[++i];
            continue;
        }
        if (strcmp(argv[i], "--output") == 0 && i + 1 < argc)
        {
            output = argv[++i];
            continue;
        }
        usage();
        return 2;
    }
    if (device == NULL || scenario == NULL || output == NULL)
    {
        usage();
        return 2;
    }
    if (run_device(device, scenario, output) != 0)
    {
        return 1;
    }
    return 0;
}
