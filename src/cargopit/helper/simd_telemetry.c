#include "simd_telemetry.h"

#include <libconfig.h>
#include <stdlib.h>
#include <string.h>

#include "dirhelper.h"

static TelemetrySource source_from_name(const char* name)
{
    if (name == NULL)
    {
        return TELEMETRY_SOURCE_AUTO;
    }
    if (strcmp(name, SIMD_TELEMETRY_SHM) == 0)
    {
        return TELEMETRY_SOURCE_SHM;
    }
    if (strcmp(name, SIMD_TELEMETRY_UDP) == 0)
    {
        return TELEMETRY_SOURCE_UDP;
    }
    return TELEMETRY_SOURCE_AUTO;
}

static TelemetrySource source_from_sim(config_setting_t* sim)
{
    const char* telemetry;
    int useudp;

    if (config_setting_lookup_string(sim, SIMD_CONFIG_KEY_TELEMETRY, &telemetry))
    {
        return source_from_name(telemetry);
    }
    if (config_setting_lookup_bool(sim, SIMD_CONFIG_KEY_USEUDP, &useudp))
    {
        if (useudp)
        {
            return TELEMETRY_SOURCE_UDP;
        }
        return TELEMETRY_SOURCE_SHM;
    }
    return TELEMETRY_SOURCE_AUTO;
}

static bool read_game_id(config_setting_t* sim, uint64_t* out)
{
    long long wide;
    int narrow;
    const char* text;
    char* end;
    unsigned long long parsed;

    if (config_setting_lookup_int64(sim, SIMD_CONFIG_KEY_GAMEID, &wide))
    {
        if (wide < 0)
        {
            return false;
        }
        *out = (uint64_t)wide;
        return true;
    }
    if (config_setting_lookup_int(sim, SIMD_CONFIG_KEY_GAMEID, &narrow))
    {
        if (narrow < 0)
        {
            return false;
        }
        *out = (uint64_t)narrow;
        return true;
    }
    if (!config_setting_lookup_string(sim, SIMD_CONFIG_KEY_GAMEID, &text) || text[0] == '\0')
    {
        return false;
    }
    parsed = strtoull(text, &end, 10);
    if (end == text || *end != '\0')
    {
        return false;
    }
    *out = parsed;
    return true;
}

static char* default_simd_config_path(void)
{
    const char* xdg = getenv("XDG_CONFIG_HOME");
    const char* home;
    char* path = NULL;

    if (xdg != NULL && xdg[0] != '\0')
    {
        if (asprintf(&path, "%s/%s/%s", xdg, SIMD_CONFIG_DIR_NAME, SIMD_CONFIG_FILE_NAME) < 0)
        {
            return NULL;
        }
        return path;
    }
    home = gethome();
    if (home == NULL || home[0] == '\0')
    {
        return NULL;
    }
    if (asprintf(&path, "%s/%s/%s/%s", home, SIMD_CONFIG_HOME_FALLBACK,
                 SIMD_CONFIG_DIR_NAME, SIMD_CONFIG_FILE_NAME) < 0)
    {
        return NULL;
    }
    return path;
}

TelemetrySource simd_telemetry_source_for_game(const char* path, uint64_t game_id)
{
    config_t cfg;
    config_setting_t* sims;
    TelemetrySource found = TELEMETRY_SOURCE_AUTO;
    int count;
    int i;

    if (path == NULL || game_id == 0)
    {
        return TELEMETRY_SOURCE_AUTO;
    }
    config_init(&cfg);
    if (config_read_file(&cfg, path) != CONFIG_TRUE)
    {
        config_destroy(&cfg);
        return TELEMETRY_SOURCE_AUTO;
    }
    sims = config_lookup(&cfg, SIMD_CONFIG_KEY_SIMS);
    if (sims == NULL)
    {
        config_destroy(&cfg);
        return TELEMETRY_SOURCE_AUTO;
    }
    count = config_setting_length(sims);
    for (i = 0; i < count; i++)
    {
        config_setting_t* sim = config_setting_get_elem(sims, i);
        uint64_t id;

        if (sim == NULL || !read_game_id(sim, &id))
        {
            continue;
        }
        if (id != game_id)
        {
            continue;
        }
        found = source_from_sim(sim);
        break;
    }
    config_destroy(&cfg);
    return found;
}

TelemetrySource simd_telemetry_source(SimulatorEXE exe)
{
    char* path;
    TelemetrySource source;

    if (exe == SIMULATOREXE_SIMAPI_TEST_NONE)
    {
        return TELEMETRY_SOURCE_AUTO;
    }
    path = default_simd_config_path();
    source = simd_telemetry_source_for_game(path, (uint64_t)exe);
    free(path);
    return source;
}
