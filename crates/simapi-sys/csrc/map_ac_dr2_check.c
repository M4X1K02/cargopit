#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <uchar.h>

#include "simapi.h"
#include "simdata.h"
#include "simmap.h"
#include "simmapper.h"

#define AC_SAMPLE_RPM 4500
#define AC_SAMPLE_GEAR 4
#define AC_SAMPLE_GEAR_CHAR '3'
#define AC_SAMPLE_SPEED_KMH 142.4f
#define AC_SAMPLE_SPEED_KMH_ROUNDED 142
#define AC_SAMPLE_GAS 0.65f
#define AC_SAMPLE_MAX_RPM 8500
#define AC_SAMPLE_CAR "ks_mazda_mx5"
#define AC_SAMPLE_TRACK "magione"
#define DR2_SAMPLE_RPM_RAW 450.0f
#define DR2_SAMPLE_RPM 4500
#define DR2_SAMPLE_MAX_RPM_RAW 800.0f
#define DR2_SAMPLE_MAX_RPM 8000
#define DR2_SAMPLE_SPEED_MS 30.0f
#define DR2_SAMPLE_SPEED_KMH 108
#define DR2_SAMPLE_GEAR_RAW 3.0f
#define DR2_SAMPLE_GEAR 4
#define DR2_SAMPLE_GEAR_TEXT "3"
#define DR2_SAMPLE_THROTTLE 0.40f
#define DR2_FORWARD_Z 1.0f
#define DR2_CAR_NAME "DR2"
#define UTF16_NAME_LIMIT 33

static int failures;

static void expect_int(const char* label, int actual, int expected)
{
    if (actual == expected)
    {
        printf("ok %s = %d\n", label, actual);
        return;
    }
    fprintf(stderr, "fail %s: got %d expected %d\n", label, actual, expected);
    failures++;
}

static void expect_text(const char* label, const char* actual, const char* expected)
{
    if (strcmp(actual, expected) == 0)
    {
        printf("ok %s = %s\n", label, actual);
        return;
    }
    fprintf(stderr, "fail %s: got '%s' expected '%s'\n", label, actual, expected);
    failures++;
}

static void set_utf16(char16_t* dest, const char* text)
{
    size_t i;

    memset(dest, 0, UTF16_NAME_LIMIT * sizeof(*dest));
    for (i = 0; text[i] != '\0' && i + 1 < UTF16_NAME_LIMIT; i++)
    {
        dest[i] = (char16_t)(unsigned char)text[i];
    }
}

static void map_assetto_corsa(SimData* simdata)
{
    struct SPageFilePhysics physics;
    struct SPageFileGraphic graphic;
    struct SPageFileStatic statics;
    SimMap* simmap;

    memset(&physics, 0, sizeof(physics));
    memset(&graphic, 0, sizeof(graphic));
    memset(statics.carModel, 0, sizeof(statics.carModel));
    memset(&statics, 0, sizeof(statics));
    memset(simdata, 0, sizeof(*simdata));

    physics.rpms = AC_SAMPLE_RPM;
    physics.gear = AC_SAMPLE_GEAR;
    physics.speedKmh = AC_SAMPLE_SPEED_KMH;
    physics.gas = AC_SAMPLE_GAS;
    graphic.status = AC_LIVE;
    graphic.GlobalYellow = 1;
    statics.maxRpm = AC_SAMPLE_MAX_RPM;
    set_utf16(statics.carModel, AC_SAMPLE_CAR);
    set_utf16(statics.track, AC_SAMPLE_TRACK);

    simmap = calloc(1, sizeof(*simmap));
    if (simmap == NULL)
    {
        fprintf(stderr, "fail ac: could not allocate SimMap\n");
        failures++;
        return;
    }
    simmap->ac.has_physics = true;
    simmap->ac.has_graphic = true;
    simmap->ac.has_static = true;
    simmap->ac.physics_map_addr = &physics;
    simmap->ac.graphic_map_addr = &graphic;
    simmap->ac.static_map_addr = &statics;

    map_assetto_corsa_data(simdata, simmap, SIMULATOREXE_ASSETTO_CORSA);
    free(simmap);

    expect_int("ac.rpms", (int)simdata->rpms, AC_SAMPLE_RPM);
    expect_int("ac.velocity_kmh", (int)simdata->velocity, AC_SAMPLE_SPEED_KMH_ROUNDED);
    expect_int("ac.gear", (int)simdata->gear, AC_SAMPLE_GEAR);
    expect_int("ac.gear_char", simdata->gearc[0], AC_SAMPLE_GEAR_CHAR);
    expect_int("ac.maxrpm", (int)simdata->maxrpm, AC_SAMPLE_MAX_RPM);
    expect_int("ac.status", (int)simdata->simstatus, AC_LIVE);
    expect_int("ac.course_flag", simdata->courseflag, SIMAPI_FLAG_YELLOW);
    expect_text("ac.car", simdata->car, AC_SAMPLE_CAR);
    expect_text("ac.track", simdata->track, AC_SAMPLE_TRACK);
}

static void map_dirt_rally_2(SimData* simdata)
{
    struct dirt2_udp_packet packet;
    SimMap* simmap;

    memset(&packet, 0, sizeof(packet));
    memset(simdata, 0, sizeof(*simdata));
    packet.fields.engineRPM = DR2_SAMPLE_RPM_RAW;
    packet.fields.maxRPM = DR2_SAMPLE_MAX_RPM_RAW;
    packet.fields.speed = DR2_SAMPLE_SPEED_MS;
    packet.fields.velZ = DR2_SAMPLE_SPEED_MS;
    packet.fields.forwardZ = DR2_FORWARD_Z;
    packet.fields.gear = DR2_SAMPLE_GEAR_RAW;
    packet.fields.throttle = DR2_SAMPLE_THROTTLE;
    packet.fields.wheelSpeedFL = DR2_SAMPLE_SPEED_MS;
    packet.fields.wheelSpeedFR = DR2_SAMPLE_SPEED_MS;
    packet.fields.wheelSpeedRL = DR2_SAMPLE_SPEED_MS;
    packet.fields.wheelSpeedRR = DR2_SAMPLE_SPEED_MS;

    simmap = calloc(1, sizeof(*simmap));
    if (simmap == NULL)
    {
        fprintf(stderr, "fail dr2: could not allocate SimMap\n");
        failures++;
        return;
    }
    map_dirt_rally_2_data(simdata, simmap, (char*)&packet);
    free(simmap);

    expect_int("dr2.rpms", (int)simdata->rpms, DR2_SAMPLE_RPM);
    expect_int("dr2.velocity_kmh", (int)simdata->velocity, DR2_SAMPLE_SPEED_KMH);
    expect_int("dr2.gear", (int)simdata->gear, DR2_SAMPLE_GEAR);
    expect_int("dr2.maxrpm", (int)simdata->maxrpm, DR2_SAMPLE_MAX_RPM);
    expect_text("dr2.gear_text", simdata->gearc, DR2_SAMPLE_GEAR_TEXT);
    expect_text("dr2.car", simdata->car, DR2_CAR_NAME);
}

static int write_scenario(const char* path, const SimData* frame)
{
    FILE* file;
    const char magic[8] = {'C', 'P', 'I', 'T', 'S', 'C', 'N', '1'};
    uint32_t count = 1;

    if (sizeof(*frame) != 46044)
    {
        fprintf(stderr, "SimData size is %zu\n", sizeof(*frame));
        return -1;
    }
    file = fopen(path, "wb");
    if (file == NULL)
    {
        perror(path);
        return -1;
    }
    if (fwrite(magic, 1, sizeof(magic), file) != sizeof(magic) ||
        fwrite(&count, sizeof(count), 1, file) != 1 ||
        fwrite(frame, sizeof(*frame), 1, file) != 1)
    {
        fclose(file);
        fprintf(stderr, "short write %s\n", path);
        return -1;
    }
    fclose(file);
    return 0;
}

int main(int argc, char** argv)
{
    SimData ac;
    SimData dr2;
    const char* ac_path = argc > 1 ? argv[1] : NULL;
    const char* dr2_path = argc > 2 ? argv[2] : NULL;

    printf("mapping Assetto Corsa shared-memory pages\n");
    map_assetto_corsa(&ac);
    printf("mapping Dirt Rally 2.0 UDP packet\n");
    map_dirt_rally_2(&dr2);

    if (ac_path != NULL && write_scenario(ac_path, &ac) != 0)
    {
        return 1;
    }
    if (dr2_path != NULL && write_scenario(dr2_path, &dr2) != 0)
    {
        return 1;
    }
    if (failures != 0)
    {
        fprintf(stderr, "%d mapping check(s) failed\n", failures);
        return 1;
    }
    printf("assetto corsa and dirt rally 2.0 mapping checks passed\n");
    return 0;
}
