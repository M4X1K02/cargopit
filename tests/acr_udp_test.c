#include <stdio.h>
#include <string.h>

#include "../src/cargopit/gameloop/acr_udp.h"
#include "../src/cargopit/simulatorapi/simapi/simapi/simdata.h"

static int failures;

static void fail(const char* msg)
{
    fprintf(stderr, "FAIL: %s\n", msg);
    failures++;
}

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
} TestAcrPacket;
#pragma pack(pop)

static void fill_packet(TestAcrPacket* pkt)
{
    memset(pkt, 0, sizeof(*pkt));
    pkt->magic[0] = ACR_UDP_MAGIC0;
    pkt->magic[1] = ACR_UDP_MAGIC1;
    pkt->magic[2] = ACR_UDP_MAGIC2;
    pkt->magic[3] = ACR_UDP_MAGIC3;
    pkt->rpm = 4200.4f;
    pkt->redline = 7500.0f;
    pkt->idle = 900.0f;
    pkt->brake_temp[0] = 410.0f;
    pkt->brake = 0.35f;
}

int main(void)
{
    TestAcrPacket pkt;
    SimData simdata;
    unsigned char short_buf[4];

    fill_packet(&pkt);
    memset(&simdata, 0, sizeof(simdata));
    if (sizeof(pkt) != ACR_UDP_PACKET_BYTES)
    {
        fail("packet size");
    }
    if (!acr_udp_packet_ok(&pkt, sizeof(pkt)))
    {
        fail("valid ACRM packet rejected");
    }
    memset(short_buf, 0, sizeof(short_buf));
    if (acr_udp_packet_ok(short_buf, sizeof(short_buf)))
    {
        fail("short buffer accepted");
    }
    acr_udp_apply(&simdata, &pkt);
    if (simdata.rpms != 4200)
    {
        fail("rpm mapping");
    }
    if (simdata.maxrpm != 7500)
    {
        fail("redline mapping");
    }
    if (simdata.idlerpm != 900)
    {
        fail("idle mapping");
    }
    if (simdata.gas < 0.55 || simdata.gas > 0.57)
    {
        fail("gas from rpm fraction");
    }
    if (simdata.brake < 0.34 || simdata.brake > 0.36)
    {
        fail("brake mapping");
    }
    if (simdata.braketemp[0] < 409.0 || simdata.braketemp[0] > 411.0)
    {
        fail("brake temp mapping");
    }
    if (simdata.simexe != SIMULATOREXE_ASSETTO_CORSA_RALLY)
    {
        fail("simexe");
    }
    if (simdata.simstatus != SIMAPI_STATUS_ACTIVEPLAY)
    {
        fail("simstatus");
    }
    if (acr_needs_udp_bridge(SIMULATOREXE_DIRT_RALLY_2))
    {
        fail("DR2 must not use ACR UDP");
    }
    if (failures != 0)
    {
        fprintf(stderr, "%i failures\n", failures);
        return 1;
    }
    return 0;
}
