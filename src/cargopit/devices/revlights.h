#ifndef _REVLIGHTS_H
#define _REVLIGHTS_H

/* Lights are spread over [0, maxrpm - margin] so the last one comes on just before redline. */
#define REVLIGHTS_REDLINE_MARGIN_FRAC 0.05

/* How many of `steps` rev lights are lit at `rpm` (0..steps). */
int revlights_lit_count(int rpm, int maxrpm, int steps);

#endif
