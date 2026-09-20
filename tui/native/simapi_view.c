#include "simapi_view.h"

#include "simdata.h"
#include "simapi.h"

#include <string.h>

static void copy_volatile_chars(char* dest, size_t dest_len, const volatile char* src, size_t src_len)
{
    size_t i;
    size_t last;
    size_t n;

    if (dest == NULL || dest_len == 0)
    {
        return;
    }
    last = dest_len - 1;
    n = last < src_len ? last : src_len;
    for (i = 0; i < n; i++)
    {
        char ch = src[i];
        dest[i] = ch;
        if (ch == '\0')
        {
            return;
        }
    }
    dest[n] = '\0';
}

size_t cargopit_tui_simdata_size(void)
{
    return sizeof(SimData);
}

int cargopit_tui_read_simdata(const void* mem, size_t len, CargopitTelemetryView* out)
{
    const volatile SimData* data;

    if (out == NULL)
    {
        return -1;
    }
    memset(out, 0, sizeof(*out));
    if (mem == NULL || len < sizeof(SimData))
    {
        return -1;
    }
    data = (const volatile SimData*)mem;
    out->mtick = data->mtick;
    out->simexe = data->simexe;
    out->simstatus = data->simstatus;
    out->velocity = data->velocity;
    out->rpms = data->rpms;
    out->gear = data->gear;
    out->maxrpm = data->maxrpm;
    out->idlerpm = data->idlerpm;
    out->lap = data->lap;
    out->position = data->position;
    out->numlaps = data->numlaps;
    out->simapi = data->simapi;
    out->simon = data->simon ? 1 : 0;
    out->simapiversion = data->simapiversion;
    out->valid = 1;
    copy_volatile_chars(out->gearc, sizeof(out->gearc), data->gearc, sizeof(data->gearc));
    copy_volatile_chars(out->car, sizeof(out->car), data->car, sizeof(data->car));
    copy_volatile_chars(out->track, sizeof(out->track), data->track, sizeof(data->track));
    out->gas = data->gas;
    out->brake = data->brake;
    out->clutch = data->clutch;
    out->steer = data->steer;
    out->fuel = data->fuel;
    out->fuelcapacity = data->fuelcapacity;
    out->abs = data->abs;
    out->xvelocity = data->Xvelocity;
    out->yvelocity = data->Yvelocity;
    out->zvelocity = data->Zvelocity;
    return 0;
}

int cargopit_tui_write_fixture(void* mem, size_t len, const CargopitTelemetryView* in)
{
    SimData* data;

    if (mem == NULL || in == NULL || len < sizeof(SimData))
    {
        return -1;
    }
    data = (SimData*)mem;
    memset(data, 0, sizeof(*data));
    data->mtick = in->mtick;
    data->simexe = in->simexe;
    data->simstatus = in->simstatus;
    data->velocity = in->velocity;
    data->rpms = in->rpms;
    data->gear = in->gear;
    data->maxrpm = in->maxrpm;
    data->idlerpm = in->idlerpm;
    data->lap = in->lap;
    data->position = in->position;
    data->numlaps = in->numlaps;
    data->simapi = in->simapi;
    data->simon = in->simon != 0;
    data->simapiversion = in->simapiversion != 0 ? in->simapiversion : SIMAPI_VERSION;
    memcpy(data->gearc, in->gearc, sizeof(data->gearc));
    memcpy(data->car, in->car, CARGOPIT_TUI_NAME_LEN - 1);
    memcpy(data->track, in->track, CARGOPIT_TUI_NAME_LEN - 1);
    data->gas = in->gas;
    data->brake = in->brake;
    data->clutch = in->clutch;
    data->steer = in->steer;
    data->fuel = in->fuel;
    data->fuelcapacity = in->fuelcapacity;
    data->abs = in->abs;
    data->Xvelocity = in->xvelocity;
    data->Yvelocity = in->yvelocity;
    data->Zvelocity = in->zvelocity;
    return 0;
}
