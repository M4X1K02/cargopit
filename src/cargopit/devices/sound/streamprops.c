#include <stdio.h>
#include <string.h>

#include "streamprops.h"
#include "../../helper/devicenames.h"

static void add_prop(SoundStreamProps* out, const char* key, const char* value)
{
    if (value == NULL || out->count >= SOUND_STREAM_PROP_COUNT_MAX)
    {
        return;
    }
    out->props[out->count].key = key;
    out->props[out->count].value = value;
    out->count++;
}

static const char* effect_name(VibrationEffectType effect)
{
    const char* name = cargopit_name_for(&CARGOPIT_EFFECTS, effect);
    if (name == NULL)
    {
        return cargopit_name_for(&CARGOPIT_EFFECTS, SOUND_STREAM_DEFAULT_EFFECT);
    }
    return name;
}

static const char* tyre_name(VibrationEffectType effect, CargopitTyreIdentifier tyre)
{
    if (!haptic_effect_uses_tyre(effect))
    {
        return NULL;
    }
    return cargopit_name_for(&CARGOPIT_TYRES, tyre);
}

static int format_node_name(char* out, const char* effect, const char* tyre)
{
    int written;
    if (tyre == NULL)
    {
        written = snprintf(out, SOUND_STREAM_NODE_NAME_MAX, "%s%s%s",
                           SOUND_STREAM_NODE_PREFIX, SOUND_STREAM_NODE_SEPARATOR, effect);
    }
    else
    {
        written = snprintf(out, SOUND_STREAM_NODE_NAME_MAX, "%s%s%s%s%s",
                           SOUND_STREAM_NODE_PREFIX, SOUND_STREAM_NODE_SEPARATOR, effect,
                           SOUND_STREAM_NODE_SEPARATOR, tyre);
    }
    if (written < 0 || written >= SOUND_STREAM_NODE_NAME_MAX)
    {
        return -1;
    }
    return 0;
}

int sound_stream_props_build(SoundStreamProps* out, VibrationEffectType effect, CargopitTyreIdentifier tyre)
{
    if (out == NULL)
    {
        return -1;
    }
    memset(out, 0, sizeof(*out));

    const char* effect_value = effect_name(effect);
    const char* tyre_value = tyre_name(effect, tyre);
    if (format_node_name(out->node_name, effect_value, tyre_value) != 0)
    {
        return -1;
    }

    out->stream_name = effect_value;
    add_prop(out, PA_PROP_MEDIA_NAME, effect_value);
    add_prop(out, PA_PROP_MEDIA_ROLE, SOUND_STREAM_MEDIA_ROLE);
    add_prop(out, PA_PROP_APPLICATION_NAME, SOUND_STREAM_APPLICATION_NAME);
    add_prop(out, PA_PROP_APPLICATION_ID, SOUND_STREAM_APPLICATION_ID);
    add_prop(out, SOUND_STREAM_PROP_NODE_NAME, out->node_name);
    add_prop(out, SOUND_STREAM_PROP_EFFECT, effect_value);
    add_prop(out, SOUND_STREAM_PROP_TYRE, tyre_value);
    return 0;
}

const char* sound_stream_props_get(const SoundStreamProps* props, const char* key)
{
    if (props == NULL || key == NULL)
    {
        return NULL;
    }
    for (size_t i = 0; i < props->count; i++)
    {
        if (strcmp(props->props[i].key, key) == 0)
        {
            return props->props[i].value;
        }
    }
    return NULL;
}
