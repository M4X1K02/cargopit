#ifndef CARGOPIT_TUI_SIMAPI_VIEW_H
#define CARGOPIT_TUI_SIMAPI_VIEW_H

#include <stddef.h>
#include <stdint.h>

typedef struct CargopitTelemetryView
{
    uint64_t mtick;
    uint64_t simexe;
    uint32_t simstatus;
    uint32_t velocity;
    uint32_t rpms;
    uint8_t simapi;
    uint8_t simon;
    uint8_t simapiversion;
    uint8_t valid;
} CargopitTelemetryView;

size_t cargopit_tui_simdata_size(void);
int cargopit_tui_read_simdata(const void* mem, size_t len, CargopitTelemetryView* out);
int cargopit_tui_write_fixture(void* mem, size_t len, const CargopitTelemetryView* in);

#endif
