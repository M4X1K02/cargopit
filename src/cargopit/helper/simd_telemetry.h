#ifndef _SIMD_TELEMETRY_H
#define _SIMD_TELEMETRY_H

#include <stdint.h>

#include "../simulatorapi/simapi/simapi/simapi.h"

#define SIMD_CONFIG_DIR_NAME "simd"
#define SIMD_CONFIG_FILE_NAME "simd.config"
#define SIMD_CONFIG_KEY_SIMS "sims"
#define SIMD_CONFIG_KEY_GAMEID "gameid"
#define SIMD_CONFIG_KEY_TELEMETRY "telemetry"
#define SIMD_CONFIG_KEY_USEUDP "useudp"
#define SIMD_TELEMETRY_AUTO "auto"
#define SIMD_TELEMETRY_SHM "shm"
#define SIMD_TELEMETRY_UDP "udp"
#define SIMD_CONFIG_HOME_FALLBACK ".config"

typedef enum
{
    TELEMETRY_SOURCE_AUTO = 0,
    TELEMETRY_SOURCE_SHM = 1,
    TELEMETRY_SOURCE_UDP = 2
}
TelemetrySource;

TelemetrySource simd_telemetry_source_for_game(const char* path, uint64_t game_id);
TelemetrySource simd_telemetry_source(SimulatorEXE exe);

#endif
