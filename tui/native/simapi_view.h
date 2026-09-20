#ifndef CARGOPIT_TUI_SIMAPI_VIEW_H
#define CARGOPIT_TUI_SIMAPI_VIEW_H

#include <stddef.h>
#include <stdint.h>

#define CARGOPIT_TUI_GEARC_LEN 4
#define CARGOPIT_TUI_NAME_LEN 32

typedef struct CargopitTelemetryView
{
    uint64_t mtick;
    uint64_t simexe;
    uint32_t simstatus;
    uint32_t velocity;
    uint32_t rpms;
    uint32_t gear;
    uint32_t maxrpm;
    uint32_t idlerpm;
    uint32_t lap;
    uint32_t position;
    uint32_t numlaps;
    uint8_t simapi;
    uint8_t simon;
    uint8_t simapiversion;
    uint8_t valid;
    char gearc[CARGOPIT_TUI_GEARC_LEN];
    char car[CARGOPIT_TUI_NAME_LEN];
    char track[CARGOPIT_TUI_NAME_LEN];
    double gas;
    double brake;
    double clutch;
    double steer;
    double fuel;
    double fuelcapacity;
    double abs;
    double xvelocity;
    double yvelocity;
    double zvelocity;
} CargopitTelemetryView;

size_t cargopit_tui_simdata_size(void);
int cargopit_tui_read_simdata(const void* mem, size_t len, CargopitTelemetryView* out);
int cargopit_tui_write_fixture(void* mem, size_t len, const CargopitTelemetryView* in);

#endif
