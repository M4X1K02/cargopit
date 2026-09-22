#include <math.h>

#include "revlights.h"

int revlights_lit_count(int rpm, int maxrpm, int steps)
{
    if (rpm <= 0 || maxrpm <= 0 || steps <= 0)
    {
        return 0;
    }
    int margin = (int) ceil(REVLIGHTS_REDLINE_MARGIN_FRAC * maxrpm);
    int interval = (maxrpm - margin) / steps;
    if (interval <= 0)
    {
        return steps;
    }
    int lit = rpm / interval;
    return lit > steps ? steps : lit;
}
