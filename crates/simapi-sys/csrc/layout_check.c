#include "simapi.h"
#include "simdata.h"

_Static_assert(sizeof(SimData) == 46044, "SimData size drifted from the parity contract");
_Static_assert(SIMAPI_VERSION == 1, "SIMAPI_VERSION drifted from the parity contract");

void simapi_layout_anchor(void)
{
}
