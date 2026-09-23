#ifndef _STREAMPROPS_H
#define _STREAMPROPS_H

#include <stddef.h>

#include <pulse/proplist.h>

#include "../../helper/confighelper.h"

/*
 * Properties attached to every haptic stream so external processors
 * (Easy Effects, Carla, PipeWire filter-chain, WirePlumber rules) can find
 * and route cargopit streams by effect and tyre. Values are the config-file
 * spellings from devicenames.h.
 */

#define SOUND_STREAM_APPLICATION_NAME "Cargopit"
#define SOUND_STREAM_APPLICATION_ID   "io.github.M4X1K02.cargopit"
#define SOUND_STREAM_MEDIA_ROLE       "game"
#define SOUND_STREAM_DEFAULT_EFFECT   EFFECT_ENGINERPM

#define SOUND_STREAM_PROP_NODE_NAME   "node.name"
#define SOUND_STREAM_PROP_EFFECT      "cargopit.effect"
#define SOUND_STREAM_PROP_TYRE        "cargopit.tyre"

/* node.name is cargopit.<effect>[.<tyre>] */
#define SOUND_STREAM_NODE_PREFIX      "cargopit"
#define SOUND_STREAM_NODE_SEPARATOR   "."
#define SOUND_STREAM_NODE_NAME_MAX    64

#define SOUND_STREAM_PROP_COUNT_MAX   8

typedef struct
{
    const char* key;
    const char* value;
}
SoundStreamProp;

typedef struct
{
    const char* stream_name;
    char node_name[SOUND_STREAM_NODE_NAME_MAX];
    SoundStreamProp props[SOUND_STREAM_PROP_COUNT_MAX];
    size_t count;
}
SoundStreamProps;

/*
 * Returns 0 on success, -1 when `out` is NULL or the node name does not fit.
 * props[] points into node_name, so use the struct where it was built.
 */
int sound_stream_props_build(SoundStreamProps* out, VibrationEffectType effect, CargopitTyreIdentifier tyre);
/* The value stored for `key`, or NULL when the stream has none. */
const char* sound_stream_props_get(const SoundStreamProps* props, const char* key);

#endif
