#include "human_frequency_response.h"

#include <stddef.h>

#define HUMAN_FR_TARGET_FEEL 5.0
#define HUMAN_FR_MIN_FEEL 0.5
#define HUMAN_FR_UNITY_SCALE 1.0
#define HUMAN_FR_MIN_SCALE 0.50
#define HUMAN_FR_MAX_SCALE 4.00
#define HUMAN_FR_POINT_COUNT 9
#define HUMAN_FR_APPLY_SEAT_CURVE 0

typedef struct
{
    double frequency_hz;
    double feel;
}
HumanFrequencyResponsePoint;

/*
 * Hz from this car's idle 1257 / redline 7330 mapped to 32–100 Hz.
 * Feel is 0–10 from the seat, current filter still in the loop.
 */
static const HumanFrequencyResponsePoint
    human_frequency_response_table[HUMAN_FR_POINT_COUNT] =
{
    { 34.7, 5.0 },
    { 40.3, 8.0 },
    { 43.7, 10.0 },
    { 45.9, 8.0 },
    { 57.1, 1.0 },
    { 62.7, 2.0 },
    { 73.9, 3.0 },
    { 85.1, 3.0 },
    { 96.3, 3.0 }
};

static double clamp_scale(double scale)
{
    if (scale < HUMAN_FR_MIN_SCALE)
    {
        return HUMAN_FR_MIN_SCALE;
    }
    if (scale > HUMAN_FR_MAX_SCALE)
    {
        return HUMAN_FR_MAX_SCALE;
    }
    return scale;
}

static double feel_to_scale(double feel)
{
    if (feel < HUMAN_FR_MIN_FEEL)
    {
        feel = HUMAN_FR_MIN_FEEL;
    }
    return clamp_scale(HUMAN_FR_TARGET_FEEL / feel);
}

static size_t upper_index(double frequency_hz)
{
    size_t lo = 0;
    size_t hi = HUMAN_FR_POINT_COUNT;
    while (lo < hi)
    {
        size_t mid = lo + (hi - lo) / 2;
        if (human_frequency_response_table[mid].frequency_hz < frequency_hz)
        {
            lo = mid + 1;
        }
        else
        {
            hi = mid;
        }
    }
    return lo;
}

static double interpolate_feel(double frequency_hz)
{
    const HumanFrequencyResponsePoint* table = human_frequency_response_table;
    if (frequency_hz <= table[0].frequency_hz)
    {
        return table[0].feel;
    }
    if (frequency_hz >= table[HUMAN_FR_POINT_COUNT - 1].frequency_hz)
    {
        return table[HUMAN_FR_POINT_COUNT - 1].feel;
    }

    size_t hi = upper_index(frequency_hz);
    size_t lo = hi - 1;
    double f0 = table[lo].frequency_hz;
    double f1 = table[hi].frequency_hz;
    double span = f1 - f0;
    if (span <= 0.0)
    {
        return table[lo].feel;
    }
    double t = (frequency_hz - f0) / span;
    return table[lo].feel + (table[hi].feel - table[lo].feel) * t;
}

double human_frequency_response_correcting_amplitude_scale(double frequency_hz)
{
    if (frequency_hz <= 0.0)
    {
        return HUMAN_FR_UNITY_SCALE;
    }
    if (HUMAN_FR_APPLY_SEAT_CURVE == 0)
    {
        return HUMAN_FR_UNITY_SCALE;
    }
    return feel_to_scale(interpolate_feel(frequency_hz));
}
