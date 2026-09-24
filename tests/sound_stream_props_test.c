#include <stdio.h>
#include <string.h>

#include "../src/cargopit/devices/sound/streamprops.h"

#define UNKNOWN_EFFECT ((VibrationEffectType) 99)

static int failures;

static void check_str(const char* name, const char* got, const char* expected)
{
    if (got == NULL && expected == NULL)
    {
        return;
    }
    if (got == NULL || expected == NULL || strcmp(got, expected) != 0)
    {
        fprintf(stderr, "FAIL: %s = %s, expected %s\n", name,
                got != NULL ? got : "(null)", expected != NULL ? expected : "(null)");
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

static void check_engine_has_no_tyre(void)
{
    SoundStreamProps props;
    check_int("engine builds", sound_stream_props_build(&props, EFFECT_ENGINERPM, FRONTLEFT), 0);
    check_str("engine node", props.node_name, "cargopit.Engine");
    check_str("engine stream name", props.stream_name, "Engine");
    check_str("engine media.name", sound_stream_props_get(&props, PA_PROP_MEDIA_NAME), "Engine");
    check_str("engine effect", sound_stream_props_get(&props, SOUND_STREAM_PROP_EFFECT), "Engine");
    check_str("engine tyre", sound_stream_props_get(&props, SOUND_STREAM_PROP_TYRE), NULL);
    check_str("engine role", sound_stream_props_get(&props, PA_PROP_MEDIA_ROLE), SOUND_STREAM_MEDIA_ROLE);
    check_str("engine app", sound_stream_props_get(&props, PA_PROP_APPLICATION_NAME), SOUND_STREAM_APPLICATION_NAME);
    check_str("engine app id", sound_stream_props_get(&props, PA_PROP_APPLICATION_ID), SOUND_STREAM_APPLICATION_ID);
    check_str("engine node.name prop", sound_stream_props_get(&props, SOUND_STREAM_PROP_NODE_NAME), props.node_name);
}

static void check_gear_has_no_tyre(void)
{
    SoundStreamProps props;
    check_int("gear builds", sound_stream_props_build(&props, EFFECT_GEARSHIFT, REARS), 0);
    check_str("gear node", props.node_name, "cargopit.Gear");
    check_str("gear tyre", sound_stream_props_get(&props, SOUND_STREAM_PROP_TYRE), NULL);
}

static void check_tyre_effects(void)
{
    SoundStreamProps props;
    check_int("slip builds", sound_stream_props_build(&props, EFFECT_TYRESLIP, FRONTLEFT), 0);
    check_str("slip node", props.node_name, "cargopit.TyreSlip.FrontLeft");
    check_str("slip tyre", sound_stream_props_get(&props, SOUND_STREAM_PROP_TYRE), "FrontLeft");

    check_int("lock builds", sound_stream_props_build(&props, EFFECT_TYRELOCK, REARS), 0);
    check_str("lock node", props.node_name, "cargopit.TyreLock.Rears");

    check_int("abs builds", sound_stream_props_build(&props, EFFECT_ABSBRAKES, ALLFOUR), 0);
    check_str("abs node", props.node_name, "cargopit.ABS.All");

    check_int("suspension builds", sound_stream_props_build(&props, EFFECT_SUSPENSION, REARRIGHT), 0);
    check_str("suspension node", props.node_name, "cargopit.Suspension.RearRight");
    check_str("suspension media.name", sound_stream_props_get(&props, PA_PROP_MEDIA_NAME), "Suspension");
}

static void check_edge_cases(void)
{
    SoundStreamProps props;
    check_int("null out rejected", sound_stream_props_build(NULL, EFFECT_ENGINERPM, ALLFOUR), -1);
    check_int("unknown effect falls back", sound_stream_props_build(&props, UNKNOWN_EFFECT, ALLFOUR), 0);
    check_str("unknown effect node", props.node_name, "cargopit.Engine");
    check_str("missing key", sound_stream_props_get(&props, "no.such.key"), NULL);
    check_str("null props", sound_stream_props_get(NULL, PA_PROP_MEDIA_NAME), NULL);
}

int main(void)
{
    check_engine_has_no_tyre();
    check_gear_has_no_tyre();
    check_tyre_effects();
    check_edge_cases();
    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    return 0;
}
