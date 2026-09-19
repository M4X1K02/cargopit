#include <math.h>
#include <stddef.h>

#include "custom_frequency_response.h"

#ifndef M_LN10
#define M_LN10 2.30258509299404568402
#endif

#define CUSTOM_FR_ANALYSIS_POINT_COUNT 96
#define CUSTOM_FR_ANALYSIS_TARGET_TRANSFER_DB (16.731778731404)
#define CUSTOM_FR_NO_BOOST_BELOW_HZ 32.0
#define CUSTOM_FR_BOOST_BLEND_HZ 8.0
#define CUSTOM_FR_MAX_BOOST_DB 0.0
#define CUSTOM_FR_MAX_CUT_DB 14.0
#define CUSTOM_FR_RESONANCE_EXTRA_CUT_DB 8.0
#define CUSTOM_FR_UNITY_GAIN 1.0
#define CUSTOM_FR_AMPLITUDE_DB_PER_DECADE 20.0
#define CUSTOM_FR_DB_TO_LIN (M_LN10 / CUSTOM_FR_AMPLITUDE_DB_PER_DECADE)
#define CUSTOM_FR_SMOOTH_RADIUS_BINS 4
#define CUSTOM_FR_SMOOTHSTEP_CUBIC 3.0
#define CUSTOM_FR_SMOOTHSTEP_QUAD 2.0

typedef struct
{
    double frequency_hz;
    double transfer_db;
}
CustomFrequencyResponseAnalysisPoint;

static const CustomFrequencyResponseAnalysisPoint
    custom_frequency_response_analysis_table[CUSTOM_FR_ANALYSIS_POINT_COUNT] =
{
    { 1.000000000e+01, -2.041207478e+01 },
    { 1.031479412e+01, -2.108193895e+01 },
    { 1.063949778e+01, -2.152328110e+01 },
    { 1.097442292e+01, -2.076601136e+01 },
    { 1.131989131e+01, -1.995933709e+01 },
    { 1.167623484e+01, -1.967801778e+01 },
    { 1.204379585e+01, -1.993156596e+01 },
    { 1.242292747e+01, -2.012352931e+01 },
    { 1.281399393e+01, -2.001673892e+01 },
    { 1.321737093e+01, -1.920653102e+01 },
    { 1.363344600e+01, -1.799253132e+01 },
    { 1.406261887e+01, -1.656227811e+01 },
    { 1.450530185e+01, -1.447789421e+01 },
    { 1.496192023e+01, -1.190424182e+01 },
    { 1.543291269e+01, -9.747038136e+00 },
    { 1.591873171e+01, -1.101165247e+01 },
    { 1.641984403e+01, -1.278689932e+01 },
    { 1.693673107e+01, -1.415486745e+01 },
    { 1.746988942e+01, -1.417006319e+01 },
    { 1.801983127e+01, -1.459943201e+01 },
    { 1.858708497e+01, -1.523875561e+01 },
    { 1.917219549e+01, -1.513175498e+01 },
    { 1.977572494e+01, -1.431809715e+01 },
    { 2.039825314e+01, -1.240866302e+01 },
    { 2.104037817e+01, -1.026845102e+01 },
    { 2.170271691e+01, -7.065028584e+00 },
    { 2.238590569e+01, -3.913426986e+00 },
    { 2.309060085e+01, -1.279345902e+00 },
    { 2.381747940e+01, -2.354593750e+00 },
    { 2.456723965e+01, -4.286088238e+00 },
    { 2.534060192e+01, -5.007022458e+00 },
    { 2.613830918e+01, -2.572471797e+00 },
    { 2.696112780e+01, 8.687943721e-01 },
    { 2.780984826e+01, 3.886284481e+00 },
    { 2.868528595e+01, 5.679699087e+00 },
    { 2.958828190e+01, 6.903869891e+00 },
    { 3.051970363e+01, 9.269946519e+00 },
    { 3.148044597e+01, 1.328690046e+01 },
    { 3.247143191e+01, 1.761592814e+01 },
    { 3.349361351e+01, 1.989138079e+01 },
    { 3.454797279e+01, 1.965106722e+01 },
    { 3.563552267e+01, 1.827402319e+01 },
    { 3.675730799e+01, 1.774054652e+01 },
    { 3.791440645e+01, 1.843097247e+01 },
    { 3.910792969e+01, 2.038676410e+01 },
    { 4.033902434e+01, 2.205237734e+01 },
    { 4.160887313e+01, 2.386682382e+01 },
    { 4.291869601e+01, 2.451293241e+01 },
    { 4.426975134e+01, 2.449413431e+01 },
    { 4.566333710e+01, 2.256676343e+01 },
    { 4.710079213e+01, 2.022916312e+01 },
    { 4.858349739e+01, 1.766703073e+01 },
    { 5.011287735e+01, 1.571912552e+01 },
    { 5.169040128e+01, 1.473114850e+01 },
    { 5.331758475e+01, 1.417880511e+01 },
    { 5.499599099e+01, 1.422503811e+01 },
    { 5.672723247e+01, 1.388952116e+01 },
    { 5.851297242e+01, 1.368993113e+01 },
    { 6.035492642e+01, 1.329601730e+01 },
    { 6.225486404e+01, 1.278000569e+01 },
    { 6.421461059e+01, 1.219400504e+01 },
    { 6.623604880e+01, 1.227506999e+01 },
    { 6.832112070e+01, 1.279076349e+01 },
    { 7.047182944e+01, 1.393142255e+01 },
    { 7.269024123e+01, 1.500483463e+01 },
    { 7.497848732e+01, 1.588168633e+01 },
    { 7.733876605e+01, 1.657779496e+01 },
    { 7.977334497e+01, 1.666832637e+01 },
    { 8.228456300e+01, 1.691145516e+01 },
    { 8.487483270e+01, 1.728409200e+01 },
    { 8.754664256e+01, 1.704997238e+01 },
    { 9.030255944e+01, 1.645188629e+01 },
    { 9.314523095e+01, 1.589138262e+01 },
    { 9.607738810e+01, 1.673177873e+01 },
    { 9.910184783e+01, 1.795261896e+01 },
    { 1.022215158e+02, 1.818936767e+01 },
    { 1.054393890e+02, 1.674857155e+01 },
    { 1.087585591e+02, 1.347717110e+01 },
    { 1.121822146e+02, 1.092695822e+01 },
    { 1.157136448e+02, 1.059510824e+01 },
    { 1.193562424e+02, 1.229876105e+01 },
    { 1.231135067e+02, 1.467207081e+01 },
    { 1.269890476e+02, 1.704961609e+01 },
    { 1.309865882e+02, 1.917316410e+01 },
    { 1.351099691e+02, 2.084543301e+01 },
    { 1.393631515e+02, 2.198355754e+01 },
    { 1.437502216e+02, 2.285777609e+01 },
    { 1.482753942e+02, 2.290683266e+01 },
    { 1.529430165e+02, 2.166308864e+01 },
    { 1.577575728e+02, 1.933218510e+01 },
    { 1.627236885e+02, 1.651208998e+01 },
    { 1.678461346e+02, 1.404861105e+01 },
    { 1.731298323e+02, 1.164180142e+01 },
    { 1.785798577e+02, 9.377893473e+00 },
    { 1.842014467e+02, 7.198721165e+00 },
    { 1.900000000e+02, 6.013448488e+00 }
};

static double custom_frequency_response_correction_db_table[CUSTOM_FR_ANALYSIS_POINT_COUNT];
static int custom_frequency_response_correction_table_ready = 0;

static double clamp_correction_db(double correction_db)
{
    if (correction_db > CUSTOM_FR_MAX_BOOST_DB)
    {
        return CUSTOM_FR_MAX_BOOST_DB;
    }
    if (correction_db < -CUSTOM_FR_MAX_CUT_DB)
    {
        return -CUSTOM_FR_MAX_CUT_DB;
    }
    return correction_db;
}

static double custom_frequency_response_smoothstep(double t)
{
    if (t <= 0.0)
    {
        return 0.0;
    }
    if (t >= 1.0)
    {
        return 1.0;
    }
    return (t * t) * (CUSTOM_FR_SMOOTHSTEP_CUBIC - CUSTOM_FR_SMOOTHSTEP_QUAD * t);
}

static double custom_frequency_response_boost_blend(double frequency_hz)
{
    if (frequency_hz <= CUSTOM_FR_NO_BOOST_BELOW_HZ)
    {
        return 0.0;
    }
    double span = CUSTOM_FR_BOOST_BLEND_HZ;
    if (span <= 0.0)
    {
        return 1.0;
    }
    double t = (frequency_hz - CUSTOM_FR_NO_BOOST_BELOW_HZ) / span;
    return custom_frequency_response_smoothstep(t);
}

static double custom_frequency_response_resonance_extra_cut_db(double frequency_hz)
{
    if (CUSTOM_FR_RESONANCE_BAND_SIGMA_HZ <= 0.0)
    {
        return 0.0;
    }
    if (CUSTOM_FR_RESONANCE_EXTRA_CUT_DB <= 0.0)
    {
        return 0.0;
    }
    double delta = frequency_hz - CUSTOM_FR_RESONANCE_HZ;
    double x = delta / CUSTOM_FR_RESONANCE_BAND_SIGMA_HZ;
    return CUSTOM_FR_RESONANCE_EXTRA_CUT_DB * exp(-CUSTOM_FR_GAUSSIAN_EXP_SCALE * x * x);
}

double custom_frequency_response_analysis_resonance_band_amplitude_scale(double frequency_hz)
{
    if (frequency_hz <= 0.0)
    {
        return 1.0;
    }
    if (CUSTOM_FR_RESONANCE_BAND_SIGMA_HZ <= 0.0)
    {
        return 1.0;
    }
    double min_scale = CUSTOM_FR_RESONANCE_ENGINE_AMP_SCALE;
    if (min_scale < 0.0)
    {
        min_scale = 0.0;
    }
    if (min_scale > 1.0)
    {
        return 1.0;
    }
    double delta = frequency_hz - CUSTOM_FR_RESONANCE_HZ;
    double x = delta / CUSTOM_FR_RESONANCE_BAND_SIGMA_HZ;
    double window = exp(-CUSTOM_FR_GAUSSIAN_EXP_SCALE * x * x);
    return 1.0 - (1.0 - min_scale) * window;
}

static double mean_table_range(const double* values, size_t center, size_t radius)
{
    size_t n = CUSTOM_FR_ANALYSIS_POINT_COUNT;
    size_t lo = center;
    size_t hi = center;
    if (center >= radius)
    {
        lo = center - radius;
    }
    else
    {
        lo = 0;
    }
    if (center + radius < n)
    {
        hi = center + radius;
    }
    else
    {
        hi = n - 1;
    }

    double sum = 0.0;
    size_t count = 0;
    size_t k;
    for (k = lo; k <= hi; k++)
    {
        sum += values[k];
        count++;
    }
    if (count == 0)
    {
        return 0.0;
    }
    return sum / (double)count;
}

static size_t custom_frequency_response_analysis_upper_index(double frequency_hz)
{
    size_t lo = 0;
    size_t hi = CUSTOM_FR_ANALYSIS_POINT_COUNT;
    while (lo < hi)
    {
        size_t mid = lo + (hi - lo) / 2;
        if (custom_frequency_response_analysis_table[mid].frequency_hz < frequency_hz)
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

static double interpolate_analysis_values(const double* values, double frequency_hz)
{
    const CustomFrequencyResponseAnalysisPoint* table =
        custom_frequency_response_analysis_table;
    if (frequency_hz <= table[0].frequency_hz)
    {
        return values[0];
    }
    if (frequency_hz >= table[CUSTOM_FR_ANALYSIS_POINT_COUNT - 1].frequency_hz)
    {
        return values[CUSTOM_FR_ANALYSIS_POINT_COUNT - 1];
    }

    size_t hi = custom_frequency_response_analysis_upper_index(frequency_hz);
    size_t lo = hi - 1;
    double f0 = table[lo].frequency_hz;
    double f1 = table[hi].frequency_hz;
    double span = f1 - f0;
    if (span <= 0.0)
    {
        return values[lo];
    }
    double t = (frequency_hz - f0) / span;
    return values[lo] + (values[hi] - values[lo]) * t;
}

void custom_frequency_response_analysis_build_correcting_output_table(void)
{
    if (custom_frequency_response_correction_table_ready)
    {
        return;
    }

    double measured[CUSTOM_FR_ANALYSIS_POINT_COUNT];
    double smoothed[CUSTOM_FR_ANALYSIS_POINT_COUNT];
    double correction[CUSTOM_FR_ANALYSIS_POINT_COUNT];
    size_t i;

    for (i = 0; i < CUSTOM_FR_ANALYSIS_POINT_COUNT; i++)
    {
        measured[i] = custom_frequency_response_analysis_table[i].transfer_db;
    }
    for (i = 0; i < CUSTOM_FR_ANALYSIS_POINT_COUNT; i++)
    {
        smoothed[i] = mean_table_range(measured, i, CUSTOM_FR_SMOOTH_RADIUS_BINS);
    }
    for (i = 0; i < CUSTOM_FR_ANALYSIS_POINT_COUNT; i++)
    {
        double hz = custom_frequency_response_analysis_table[i].frequency_hz;
        double corr = CUSTOM_FR_ANALYSIS_TARGET_TRANSFER_DB - smoothed[i];
        corr = clamp_correction_db(corr);
        if (corr > 0.0)
        {
            corr *= custom_frequency_response_boost_blend(hz);
        }
        correction[i] = corr;
    }
    for (i = 0; i < CUSTOM_FR_ANALYSIS_POINT_COUNT; i++)
    {
        correction[i] = mean_table_range(correction, i, CUSTOM_FR_SMOOTH_RADIUS_BINS);
    }
    for (i = 0; i < CUSTOM_FR_ANALYSIS_POINT_COUNT; i++)
    {
        double hz = custom_frequency_response_analysis_table[i].frequency_hz;
        double corr = correction[i] - custom_frequency_response_resonance_extra_cut_db(hz);
        custom_frequency_response_correction_db_table[i] = clamp_correction_db(corr);
    }
    custom_frequency_response_correction_table_ready = 1;
}

double custom_frequency_response_analysis_lookup_transfer_db(double frequency_hz)
{
    const CustomFrequencyResponseAnalysisPoint* table =
        custom_frequency_response_analysis_table;
    if (frequency_hz <= table[0].frequency_hz)
    {
        return table[0].transfer_db;
    }
    if (frequency_hz >= table[CUSTOM_FR_ANALYSIS_POINT_COUNT - 1].frequency_hz)
    {
        return table[CUSTOM_FR_ANALYSIS_POINT_COUNT - 1].transfer_db;
    }

    size_t hi = custom_frequency_response_analysis_upper_index(frequency_hz);
    size_t lo = hi - 1;
    double f0 = table[lo].frequency_hz;
    double f1 = table[hi].frequency_hz;
    double span = f1 - f0;
    if (span <= 0.0)
    {
        return table[lo].transfer_db;
    }
    double t = (frequency_hz - f0) / span;
    return table[lo].transfer_db + (table[hi].transfer_db - table[lo].transfer_db) * t;
}

double custom_frequency_response_analysis_correcting_output_db(double frequency_hz)
{
    if (frequency_hz <= 0.0)
    {
        return 0.0;
    }
    custom_frequency_response_analysis_build_correcting_output_table();
    return interpolate_analysis_values(
        custom_frequency_response_correction_db_table, frequency_hz);
}

double custom_frequency_response_filter_correcting_output_gain(double frequency_hz)
{
    if (frequency_hz <= 0.0)
    {
        return CUSTOM_FR_UNITY_GAIN;
    }
    double correction_db = custom_frequency_response_analysis_correcting_output_db(frequency_hz);
    double gain = exp(correction_db * CUSTOM_FR_DB_TO_LIN);
    if (gain > CUSTOM_FR_UNITY_GAIN)
    {
        return CUSTOM_FR_UNITY_GAIN;
    }
    return gain;
}
