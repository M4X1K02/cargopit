#include "simapi_view.h"

#include <string.h>

size_t cargopit_tui_simdata_size(void)
{
    return 0;
}

int cargopit_tui_read_simdata(const void* mem, size_t len, CargopitTelemetryView* out)
{
    (void)mem;
    (void)len;
    if (out == NULL)
    {
        return -1;
    }
    memset(out, 0, sizeof(*out));
    return -1;
}

int cargopit_tui_write_fixture(void* mem, size_t len, const CargopitTelemetryView* in)
{
    (void)mem;
    (void)len;
    (void)in;
    return -1;
}
