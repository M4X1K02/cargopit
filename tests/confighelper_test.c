// Proof-of-plumbing test for the new CTest wiring — not an attempt at
// comprehensive coverage. Exercises strtodevsubsubtype (helper/confighelper.c),
// a pure string->enum mapping with no I/O/hardware, including the R8/R3
// aliasing onto SIMDEVSUBTYPE_MOZAR5 and the fallback for unrecognized input.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "../src/cargopit/helper/confighelper.h"
#include "../src/cargopit/helper/devicenames.h"

static int failures = 0;

static void check(const char* input, DeviceSubSubType expected)
{
    DeviceSettings ds = {0};
    strtodevsubsubtype(input, &ds);
    if (ds.dev_subsubtype != expected)
    {
        fprintf(stderr, "FAIL: strtodevsubsubtype(\"%s\") = %d, expected %d\n",
                input, ds.dev_subsubtype, expected);
        failures++;
    }
}

static void check_index(const char* path, int requested, int expected)
{
    int got = resolve_config_index(path, requested);
    if (got != expected)
    {
        fprintf(stderr, "FAIL: resolve_config_index(%s, %d) = %d, expected %d\n",
                path, requested, got, expected);
        failures++;
    }
}

static void check_u32(const char* name, uint32_t got, uint32_t expected)
{
    if (got != expected)
    {
        fprintf(stderr, "FAIL: %s = %u, expected %u\n", name, got, expected);
        failures++;
    }
}

static void check_int(const char* name, int got, int expected)
{
    if (got != expected)
    {
        fprintf(stderr, "FAIL: %s = %d, expected %d\n", name, got, expected);
        failures++;
    }
}

static void check_device_mapping(
    const char* device_type,
    const char* device_subtype,
    int expected_error,
    bool expected_valid)
{
    DeviceSettings ds = {0};
    int error = strtodev(device_type, device_subtype, &ds);
    if (error != expected_error || ds.is_valid != expected_valid)
    {
        fprintf(stderr,
                "FAIL: strtodev(%s, %s) = error %d valid %d, expected error %d valid %d\n",
                device_type,
                device_subtype,
                error,
                ds.is_valid,
                expected_error,
                expected_valid);
        failures++;
    }
}

static void check_effect_mapping(const char* effect, int expected_error, bool expected_valid)
{
    DeviceSettings ds = {0};
    int error = strtoeffecttype(effect, &ds);
    if (error != expected_error || ds.is_valid != expected_valid)
    {
        fprintf(stderr,
                "FAIL: strtoeffecttype(%s) = error %d valid %d, expected error %d valid %d\n",
                effect,
                error,
                ds.is_valid,
                expected_error,
                expected_valid);
        failures++;
    }
}

static void check_sound_channel_helpers(void)
{
    uint32_t stereo = sound_channel_mask_all(SOUND_CHANNELS_STEREO);
    uint32_t surround = sound_channel_mask_all(SOUND_CHANNELS_SURROUND_71);

    check_u32("mask_all stereo", stereo, SOUND_CHANNEL_BIT(0) | SOUND_CHANNEL_BIT(1));
    check_u32(
        "mask_all 7.1",
        surround,
        SOUND_CHANNEL_BIT(0) | SOUND_CHANNEL_BIT(1) | SOUND_CHANNEL_BIT(2) | SOUND_CHANNEL_BIT(3)
            | SOUND_CHANNEL_BIT(4) | SOUND_CHANNEL_BIT(5) | SOUND_CHANNEL_BIT(6)
            | SOUND_CHANNEL_BIT(7));
    check_u32("mask_all below min", sound_channel_mask_all(0), SOUND_CHANNEL_BIT(0));
    check_u32(
        "missing pan and mask",
        sound_resolve_channel_mask(0, 0, 0, 0, SOUND_CHANNELS_STEREO),
        stereo);
    check_u32(
        "legacy pan all",
        sound_resolve_channel_mask(1, SOUND_PAN_ALL_CHANNELS, 0, 0, SOUND_CHANNELS_SURROUND_71),
        surround);
    check_u32(
        "legacy pan index",
        sound_resolve_channel_mask(1, 3, 0, 0, SOUND_CHANNELS_SURROUND_71),
        SOUND_CHANNEL_BIT(3));
    check_u32(
        "channelMask wins",
        sound_resolve_channel_mask(1, 0, 1, 5, SOUND_CHANNELS_STEREO),
        SOUND_CHANNEL_BIT(0));
    check_u32(
        "zero mask becomes all",
        sound_resolve_channel_mask(0, 0, 1, 0, SOUND_CHANNELS_STEREO),
        stereo);
    check_int("first channel of bit 3", sound_first_channel(SOUND_CHANNEL_BIT(3)), 3);
    check_int("first channel of empty", sound_first_channel(0), 0);
}

/* Every name cargopit writes must parse back to the same value. */
static void check_name_table_round_trips(const char* label, const CargopitNameTable* table)
{
    for (size_t i = 0; i < table->count; i++)
    {
        int value = -1;
        const char* written = cargopit_name_for(table, table->names[i].value);
        if (written == NULL || cargopit_name_lookup(table, written, &value) != 0 || value != table->names[i].value)
        {
            fprintf(stderr, "FAIL: %s name %s does not round-trip\n", label, table->names[i].name);
            failures++;
        }
    }
}

static void check_name_tables(void)
{
    int value = -1;

    check_name_table_round_trips("class", &CARGOPIT_DEVICE_CLASSES);
    check_name_table_round_trips("usb type", &CARGOPIT_USB_TYPES);
    check_name_table_round_trips("serial type", &CARGOPIT_SERIAL_TYPES);
    check_name_table_round_trips("sound type", &CARGOPIT_SOUND_TYPES);
    check_name_table_round_trips("hardware", &CARGOPIT_HARDWARE);
    check_name_table_round_trips("effect", &CARGOPIT_EFFECTS);
    check_name_table_round_trips("tyre", &CARGOPIT_TYRES);
    check_name_table_round_trips("modulation", &CARGOPIT_MODULATIONS);

    cargopit_name_lookup(&CARGOPIT_MODULATIONS, "Amplitude", &value);
    check_int("TUI modulation Amplitude", value, EFFECT_MODULATION_AMPLIFY);
    cargopit_name_lookup(&CARGOPIT_MODULATIONS, "AMPLIFY", &value);
    check_int("legacy modulation AMPLIFY", value, EFFECT_MODULATION_AMPLIFY);
    cargopit_name_lookup(&CARGOPIT_TYRES, "fronts", &value);
    check_int("tyre names are case-insensitive", value, FRONTS);
    check_int("unknown name", cargopit_name_lookup(&CARGOPIT_EFFECTS, "Warp", &value), -1);
}

static char* write_temp_file(const char* body)
{
    char tmpl[] = "/tmp/cargopit-config-test-XXXXXX";
    int fd = mkstemp(tmpl);
    if (fd < 0)
    {
        perror("mkstemp");
        return NULL;
    }
    if (write(fd, body, strlen(body)) < 0)
    {
        perror("write");
        close(fd);
        return NULL;
    }
    close(fd);
    return strdup(tmpl);
}

static void check_setting(config_t* cfg, const char* path, const char* expected)
{
    const char* got = NULL;
    if (!config_lookup_string(cfg, path, &got) || strcmp(got, expected) != 0)
    {
        fprintf(stderr, "FAIL: %s = %s, expected %s\n", path, got ? got : "(missing)", expected);
        failures++;
    }
}

/* save_device_config used to write the class offset instead of the device's own type. */
static void check_save_writes_device_names(void)
{
    char* path = write_temp_file("configs = ( { sim = \"default\"; car = \"default\"; devices = (); } );\n");
    DeviceSettings ds = {0};
    config_t* cfg;

    if (path == NULL)
    {
        failures++;
        return;
    }
    ds.dev_type = SIMDEV_SERIAL;
    ds.dev_subtype = SIMDEVTYPE_SHIFTLIGHTS;
    ds.dev_subsubtype = SIMDEVSUBTYPE_MOZA_NEW;
    ds.has_haptic_effects = true;
    ds.hapticsettings.effect_type = EFFECT_TYRESLIP;
    ds.hapticsettings.tyre = FRONTS;
    ds.hapticsettings.modulation = EFFECT_MODULATION_AMPLIFY;
    cfg = open_cargopit_config(path);
    save_device_config(cfg, path, 0, 0, &ds);
    close_cargopit_config(cfg);

    cfg = open_cargopit_config(path);
    check_setting(cfg, "configs.[0].devices.[0].device", "Serial");
    check_setting(cfg, "configs.[0].devices.[0].type", "ShiftLights");
    check_setting(cfg, "configs.[0].devices.[0].subtype", "MozaNew");
    check_setting(cfg, "configs.[0].devices.[0].effect", "TyreSlip");
    check_setting(cfg, "configs.[0].devices.[0].tyre", "Fronts");
    check_setting(cfg, "configs.[0].devices.[0].modulation", "Amplitude");
    close_cargopit_config(cfg);
    unlink(path);
    free(path);
}

static char* write_two_profiles(void)
{
    char tmpl[] = "/tmp/cargopit-config-index-XXXXXX";
    int fd = mkstemp(tmpl);
    if (fd < 0)
    {
        perror("mkstemp");
        return NULL;
    }
    const char* body =
        "configs = (\n"
        "  { sim = \"ac\"; car = \"one\"; devices = (); },\n"
        "  { sim = \"acc\"; car = \"two\"; devices = (); }\n"
        ");\n";
    if (write(fd, body, strlen(body)) < 0)
    {
        perror("write");
        close(fd);
        return NULL;
    }
    close(fd);
    return strdup(tmpl);
}

int main(void)
{
    check("MozaR5", SIMDEVSUBTYPE_MOZAR5);
    check("MozaR8", SIMDEVSUBTYPE_MOZAR5);   // aliased onto the same protocol as R5
    check("MozaR3", SIMDEVSUBTYPE_MOZAR5);   // same
    check("MozaNew", SIMDEVSUBTYPE_MOZA_NEW);
    check("MozaKSProWheel", SIMDEVSUBTYPE_MOZA_KS_PRO_WHEEL);
    check("CammusC5", SIMDEVSUBTYPE_CAMMUSC5);
    check("CammusC12", SIMDEVSUBTYPE_CAMMUSC12);
    check("LogitechG29", SIMDEVSUBTYPE_LOGITECH_G29);
    check("not-a-real-subtype", SIMDEVSUBTYPE_UNKNOWN);
    check("", SIMDEVSUBTYPE_UNKNOWN);
    check_device_mapping("USB", "Tachometer", CARGOPIT_ERROR_NONE, true);
    check_device_mapping("USB", "not-a-real-subtype", CARGOPIT_ERROR_INVALID_DEV, false);
    check_device_mapping("Serial", "not-a-real-subtype", CARGOPIT_ERROR_INVALID_DEV, false);
    check_effect_mapping("Engine", CARGOPIT_ERROR_NONE, true);
    check_effect_mapping("not-a-real-effect", CARGOPIT_ERROR_INVALID_DEV, false);

    char* path = write_two_profiles();
    if (path == NULL)
    {
        fprintf(stderr, "FAIL: could not write fixture\n");
        return 1;
    }
    check_index(path, 1, 1);
    check_index(path, CONFIG_INDEX_UNSET, CONFIG_INDEX_FIRST);
    check_index(path, 99, CONFIG_INDEX_UNSET);
    unlink(path);
    free(path);

    check_sound_channel_helpers();
    check_name_tables();
    check_save_writes_device_names();

    check_int("fps zero clamps to min", cargopit_clamp_fps(0), CARGOPIT_FPS_MIN);
    check_int("fps negative clamps to min", cargopit_clamp_fps(-5), CARGOPIT_FPS_MIN);
    check_int("fps default is kept", cargopit_clamp_fps(CARGOPIT_FPS_DEFAULT), CARGOPIT_FPS_DEFAULT);
    check_int("fps above max clamps to max", cargopit_clamp_fps(CARGOPIT_FPS_MAX + 1), CARGOPIT_FPS_MAX);

    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }

    printf("confighelper_test: all checks passed\n");
    return 0;
}
