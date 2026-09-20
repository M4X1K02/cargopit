#include "simapi_view.h"

#include "simdata.h"
#include "simapi.h"

#include <string.h>

size_t cargopit_tui_simdata_size(void)
{
    return sizeof(SimData);
}

int cargopit_tui_read_simdata(const void* mem, size_t len, CargopitTelemetryView* out)
{
    if (out == NULL)
    {
        return -1;
    }
    memset(out, 0, sizeof(*out));
    if (mem == NULL || len < sizeof(SimData))
    {
        return -1;
    }
    const SimData* data = (const SimData*)mem;
    out->mtick = data->mtick;
    out->simexe = data->simexe;
    out->simstatus = data->simstatus;
    out->velocity = data->velocity;
    out->rpms = data->rpms;
    out->simapi = data->simapi;
    out->simon = data->simon ? 1 : 0;
    out->simapiversion = data->simapiversion;
    out->valid = 1;
    return 0;
}

int cargopit_tui_write_fixture(void* mem, size_t len, const CargopitTelemetryView* in)
{
    if (mem == NULL || in == NULL || len < sizeof(SimData))
    {
        return -1;
    }
    SimData* data = (SimData*)mem;
    memset(data, 0, sizeof(*data));
    data->mtick = in->mtick;
    data->simexe = in->simexe;
    data->simstatus = in->simstatus;
    data->velocity = in->velocity;
    data->rpms = in->rpms;
    data->simapi = in->simapi;
    data->simon = in->simon != 0;
    data->simapiversion = in->simapiversion != 0 ? in->simapiversion : SIMAPI_VERSION;
    return 0;
}
