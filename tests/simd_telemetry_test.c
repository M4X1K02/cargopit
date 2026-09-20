#define _GNU_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "../src/cargopit/helper/simd_telemetry.h"

static int failures;

static void fail(const char* msg)
{
    fprintf(stderr, "FAIL: %s\n", msg);
    failures++;
}

static char* write_config(const char* body)
{
    char tmpl[] = "/tmp/cargopit-simd-telem-XXXXXX";
    char* dir = mkdtemp(tmpl);
    char* path = NULL;
    FILE* f;

    if (dir == NULL)
    {
        perror("mkdtemp");
        return NULL;
    }
    dir = strdup(dir);
    if (asprintf(&path, "%s/%s", dir, SIMD_CONFIG_FILE_NAME) < 0)
    {
        return NULL;
    }
    f = fopen(path, "w");
    if (f == NULL)
    {
        perror("fopen");
        return NULL;
    }
    fputs(body, f);
    fclose(f);
    free(dir);
    return path;
}

int main(void)
{
    char* path;
    const uint64_t acr = (uint64_t)SIMULATOREXE_ASSETTO_CORSA_RALLY;
    const uint64_t dr2 = (uint64_t)SIMULATOREXE_DIRT_RALLY_2;

    path = write_config(
        "sims = (\n"
        "  { name = \"AssettoCorsaRally\"; gameid = 3917090; telemetry = \"udp\"; },\n"
        "  { name = \"DirtRally2\"; gameid = \"690790\"; useudp = false; }\n"
        ");\n");
    if (path == NULL)
    {
        return 1;
    }
    if (simd_telemetry_source_for_game(path, acr) != TELEMETRY_SOURCE_UDP)
    {
        fail("ACR telemetry=udp");
    }
    if (simd_telemetry_source_for_game(path, dr2) != TELEMETRY_SOURCE_SHM)
    {
        fail("DR2 useudp=false is shm");
    }
    if (simd_telemetry_source_for_game(path, 1) != TELEMETRY_SOURCE_AUTO)
    {
        fail("unknown game is auto");
    }
    if (simd_telemetry_source_for_game("/no/such/simd.config", acr) != TELEMETRY_SOURCE_AUTO)
    {
        fail("missing file is auto");
    }
    unlink(path);
    free(path);
    if (failures != 0)
    {
        fprintf(stderr, "%i failures\n", failures);
        return 1;
    }
    return 0;
}
