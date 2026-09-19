#ifndef _HUMAN_FREQUENCY_RESPONSE_H
#define _HUMAN_FREQUENCY_RESPONSE_H

/*
 * Seat-feel correction from a human frequency response.
 * Disabled while the accelerometer inverse EQ is flattening the boom;
 * stacking both inverted the 3500 rpm band. Engine stream only.
 */

double human_frequency_response_correcting_amplitude_scale(double frequency_hz);

#endif
