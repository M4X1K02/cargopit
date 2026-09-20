#include "acr_udp.h"

#include <fcntl.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "../simulatorapi/simapi/simapi/ac.h"
#include "../simulatorapi/simapi/simapi/simmap.h"

#define SHM_DIR "/dev/shm/"
#define AC_PHYSICS_SHM_PATH SHM_DIR AC_PHYSICS_FILE

#pragma pack(push, 1)
typedef struct
{
    char magic[4];
    float rpm;
    float redline;
    float idle;
    float brake_temp[4];
    float locking;
    float abs_active;
    float tc_active;
    float brake;
} AcrUdpPacket;
#pragma pack(pop)

_Static_assert(sizeof(AcrUdpPacket) == ACR_UDP_PACKET_BYTES, "ACR UDP packet size");

static uint32_t rpm_to_u32(float rpm)
{
    if (!(rpm > 0.0f))
    {
        return 0;
    }
    if (rpm > (float)UINT32_MAX)
    {
        return UINT32_MAX;
    }
    return (uint32_t)(rpm + 0.5f);
}

static uint64_t monotonic_ms(void)
{
    struct timespec ts;

    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000ULL + (uint64_t)(ts.tv_nsec / 1000000L);
}

bool ac_physics_shm_is_blank(void)
{
    unsigned char buf[ACR_UDP_PHYSICS_PROBE_BYTES];
    ssize_t nread;
    int fd;
    int i;

    fd = open(AC_PHYSICS_SHM_PATH, O_RDONLY);
    if (fd < 0)
    {
        return true;
    }
    nread = read(fd, buf, sizeof(buf));
    close(fd);
    if (nread <= 0)
    {
        return true;
    }
    for (i = 0; i < nread; i++)
    {
        if (buf[i] != 0)
        {
            return false;
        }
    }
    return true;
}

bool acr_needs_udp_bridge(SimulatorEXE exe)
{
    if (exe != SIMULATOREXE_ASSETTO_CORSA_RALLY)
    {
        return false;
    }
    return ac_physics_shm_is_blank();
}

bool acr_udp_packet_ok(const void* buf, size_t nread)
{
    const AcrUdpPacket* pkt;

    if (buf == NULL || nread < sizeof(AcrUdpPacket))
    {
        return false;
    }
    pkt = buf;
    return pkt->magic[0] == ACR_UDP_MAGIC0
        && pkt->magic[1] == ACR_UDP_MAGIC1
        && pkt->magic[2] == ACR_UDP_MAGIC2
        && pkt->magic[3] == ACR_UDP_MAGIC3;
}

void acr_udp_apply(SimData* simdata, const void* buf)
{
    const AcrUdpPacket* pkt;
    uint32_t redline;
    uint32_t idle;
    int i;

    if (simdata == NULL || !acr_udp_packet_ok(buf, sizeof(AcrUdpPacket)))
    {
        return;
    }
    pkt = buf;
    redline = rpm_to_u32(pkt->redline);
    if (redline == 0)
    {
        redline = ACR_UDP_REDLINE_FLOOR;
    }
    idle = rpm_to_u32(pkt->idle);
    if (idle == 0)
    {
        idle = ACR_UDP_IDLE_DEFAULT;
    }
    simdata->prev_mtick = simdata->mtick;
    simdata->mtick = monotonic_ms();
    simdata->rpms = rpm_to_u32(pkt->rpm);
    simdata->maxrpm = redline;
    simdata->idlerpm = idle;
    simdata->gas = 0.0;
    if (redline > 0)
    {
        simdata->gas = (double)simdata->rpms / (double)redline;
    }
    if (simdata->gas > 1.0)
    {
        simdata->gas = 1.0;
    }
    simdata->brake = pkt->brake;
    if (simdata->brake < 0.0)
    {
        simdata->brake = 0.0;
    }
    if (simdata->brake > 1.0)
    {
        simdata->brake = 1.0;
    }
    for (i = 0; i < 4; i++)
    {
        simdata->braketemp[i] = pkt->brake_temp[i];
    }
    simdata->simon = true;
    simdata->simstatus = SIMAPI_STATUS_ACTIVEPLAY;
    simdata->simapi = SIMULATORAPI_ASSETTO_CORSA;
    simdata->simexe = SIMULATOREXE_ASSETTO_CORSA_RALLY;
}

void acr_udp_publish(SimMap* simmap, const SimData* simdata)
{
    if (simmap == NULL || simmap->addr == NULL || simdata == NULL)
    {
        return;
    }
    memcpy(simmap->addr, simdata, sizeof(SimData));
}
