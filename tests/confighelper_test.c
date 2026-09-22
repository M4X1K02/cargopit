// Proof-of-plumbing test for the new CTest wiring — not an attempt at
// comprehensive coverage. Exercises strtodevsubsubtype (helper/confighelper.c),
// a pure string->enum mapping with no I/O/hardware, including the R8/R3
// aliasing onto SIMDEVSUBTYPE_MOZAR5 and the fallback for unrecognized input.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "../src/cargopit/helper/confighelper.h"

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
