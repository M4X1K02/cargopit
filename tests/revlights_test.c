#include <stdio.h>

#include "../src/cargopit/devices/revlights.h"

#define MAXRPM      8000
#define STEPS       6
/* Lights span 0..7600 rpm (5% margin), so each of 6 steps is 1266 rpm. */
#define STEP_RPM    1266

static int failures;

static void check(const char* name, int got, int expected)
{
    if (got != expected)
    {
        fprintf(stderr, "FAIL: %s = %d, expected %d\n", name, got, expected);
        failures++;
    }
}

int main(void)
{
    check("idle below first step", revlights_lit_count(STEP_RPM - 1, MAXRPM, STEPS), 0);
    check("first step", revlights_lit_count(STEP_RPM, MAXRPM, STEPS), 1);
    check("third step", revlights_lit_count(STEP_RPM * 3, MAXRPM, STEPS), 3);
    check("all lit before redline", revlights_lit_count(MAXRPM - 1, MAXRPM, STEPS), STEPS);
    check("capped past redline", revlights_lit_count(MAXRPM * 2, MAXRPM, STEPS), STEPS);
    check("no rpm", revlights_lit_count(0, MAXRPM, STEPS), 0);
    check("no maxrpm", revlights_lit_count(STEP_RPM, 0, STEPS), 0);
    check("no steps", revlights_lit_count(STEP_RPM, MAXRPM, 0), 0);
    if (failures > 0)
    {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    return 0;
}
