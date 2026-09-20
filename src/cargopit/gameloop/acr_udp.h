#ifndef _ACR_UDP_H
#define _ACR_UDP_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "../simulatorapi/simapi/simapi/simapi.h"
#include "../simulatorapi/simapi/simapi/simdata.h"
#include "../simulatorapi/simapi/simapi/simmapper.h"

#define SIM_UDP_PORT_ASSETTO_CORSA_RALLY 20999
#define ACR_UDP_PACKET_BYTES 48
#define ACR_UDP_MAGIC0 'A'
#define ACR_UDP_MAGIC1 'C'
#define ACR_UDP_MAGIC2 'R'
#define ACR_UDP_MAGIC3 'M'
#define ACR_UDP_PHYSICS_PROBE_BYTES 64
#define ACR_UDP_REDLINE_FLOOR 5000
#define ACR_UDP_IDLE_DEFAULT 900

bool ac_physics_shm_is_blank(void);
bool acr_needs_udp_bridge(SimulatorEXE exe);
bool acr_udp_packet_ok(const void* buf, size_t nread);
void acr_udp_apply(SimData* simdata, const void* buf);
void acr_udp_publish(SimMap* simmap, const SimData* simdata);

#endif
