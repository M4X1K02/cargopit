#ifndef _CUSTOM_FREQUENCY_RESPONSE_H
#define _CUSTOM_FREQUENCY_RESPONSE_H

/*
 * Custom frequency-response analysis filter for this rig.
 * Corrects shaker output from a phyphox Acceleration-with-g transfer sweep
 * (Sinuslive BassPUMP III + Nobsound NS-01G Pro, 19 V full power,
 *  phyphox seat-wedge Acceleration-with-g sweep, 2026-09-19).
 */

#define CUSTOM_FR_RESONANCE_HZ 42.918696
#define CUSTOM_FR_RESONANCE_BAND_SIGMA_HZ 5.0
#define CUSTOM_FR_RESONANCE_ENGINE_AMP_SCALE 1.0
#define CUSTOM_FR_GAUSSIAN_EXP_SCALE 0.5

void custom_frequency_response_analysis_build_correcting_output_table(void);
double custom_frequency_response_analysis_lookup_transfer_db(double frequency_hz);
double custom_frequency_response_analysis_correcting_output_db(double frequency_hz);
double custom_frequency_response_filter_correcting_output_gain(double frequency_hz);
double custom_frequency_response_analysis_resonance_band_amplitude_scale(double frequency_hz);

#endif
