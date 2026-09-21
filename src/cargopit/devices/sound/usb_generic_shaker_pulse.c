#include <stdio.h>
#include <string.h>
#include <math.h>
#include <stdint.h>
#include <unistd.h>


#include "usb_generic_shaker.h"
#include "custom_frequency_response.h"
#include "human_frequency_response.h"
#include "../sounddevice.h"
#include "../../helper/confighelper.h"

#define FORMAT PA_SAMPLE_S16LE
#define SAMPLE_RATE   (48000)
#define AMPLITUDE 1
#define DURATION 1.0
#define GEAR_DECAY_K 3.0
#define AMPLITUDE_UNIT ((double)HAPTIC_AMPLITUDE_UNITY)
#define AUDIO_ATTACK_S 0.008
#define AUDIO_RELEASE_S 0.150
#define AUDIO_FREQ_SMOOTH_S 0.018
#define AUDIO_AMP_SMOOTH_S 0.025
#define AUDIO_SILENCE_GAIN 0.001
#define AUDIO_LATENCY_S 0.040
#define AUDIO_GEAR_LATENCY_S 0.040
#define AUDIO_MAX_BUFFER_S 0.400
#define SHAKER_MAX_DIGITAL_DRIVE 0.40
#define SHAKER_SINK_INPUT_UNMUTED 0
#define PERCENT_SCALE 100.0

#ifndef M_PI
#define M_PI  (3.14159265)
#endif
#define TWO_PI (2.0 * M_PI)
#define HARMONIC2_ORDER 2.0
#define HARMONIC3_ORDER 3.0

static double apply_noise(double base_freq, double noise_amount) {
    if (noise_amount > 0.0) {
        double r = (double) rand() / RAND_MAX * 2.0 - 1.0;
        return base_freq + r * noise_amount;
    } else {
        return base_freq;
    }
}

static double clamp_shaker_hz(double hz)
{
    if (hz <= 0.0)
    {
        return 0.0;
    }
    if (hz > SHAKER_TONE_MAX_HZ)
    {
        return SHAKER_TONE_MAX_HZ;
    }
    return hz;
}

static double lowpass_alpha(double cutoff_hz, double rate)
{
    double a;
    if (cutoff_hz <= 0.0 || rate <= cutoff_hz)
    {
        return 1.0;
    }
    a = 1.0 - exp(-TWO_PI * cutoff_hz / rate);
    if (a < 0.0)
    {
        return 0.0;
    }
    if (a > 1.0)
    {
        return 1.0;
    }
    return a;
}

static double lowpass_pole(double x, double* z, double alpha)
{
    *z += alpha * (x - *z);
    return *z;
}

static double shaker_output_lowpass(SoundData* data, double x, double rate)
{
    double alpha = lowpass_alpha(SHAKER_OUTPUT_LP_HZ, rate);
    x = lowpass_pole(x, &data->lp1, alpha);
    x = lowpass_pole(x, &data->lp2, alpha);
    return x;
}

static void shaker_output_lowpass_reset(SoundData* data)
{
    data->lp1 = 0.0;
    data->lp2 = 0.0;
}

typedef struct
{
    double rate;
    double attack;
    double release;
    double freq;
    double amp;
}
ShakerCoeff;

static const pa_sample_spec* stream_spec(pa_stream* s)
{
    const pa_sample_spec* spec = pa_stream_get_sample_spec(s);
    return spec;
}

static int stream_channels(pa_stream* s)
{
    const pa_sample_spec* spec = stream_spec(s);
    if (spec == NULL || spec->channels < 1)
    {
        return 1;
    }
    return spec->channels;
}

static double stream_rate(pa_stream* s)
{
    const pa_sample_spec* spec = stream_spec(s);
    if (spec == NULL || spec->rate < 1)
    {
        return (double)SAMPLE_RATE;
    }
    return (double)spec->rate;
}

static ShakerCoeff coeffs_for_rate(double rate)
{
    ShakerCoeff c;
    if (rate < 1.0)
    {
        rate = (double)SAMPLE_RATE;
    }
    c.rate = rate;
    c.attack = 1.0 - exp(-1.0 / (rate * AUDIO_ATTACK_S));
    c.release = 1.0 - exp(-1.0 / (rate * AUDIO_RELEASE_S));
    c.freq = 1.0 - exp(-1.0 / (rate * AUDIO_FREQ_SMOOTH_S));
    c.amp = 1.0 - exp(-1.0 / (rate * AUDIO_AMP_SMOOTH_S));
    return c;
}

static void write_frame(int16_t* buffer, size_t frame, int channels, int16_t sample)
{
    int ch;
    for (ch = 0; ch < channels; ch++)
    {
        buffer[frame * (size_t)channels + (size_t)ch] = sample;
    }
}

static int16_t clamp_i16(double sample)
{
    if (sample > (double)INT16_MAX)
    {
        return INT16_MAX;
    }
    if (sample < (double)INT16_MIN)
    {
        return INT16_MIN;
    }
    return (int16_t)lrint(sample);
}

static uint32_t bytes_for_duration(int channels, double seconds)
{
    if (channels < 1)
    {
        channels = 1;
    }
    if (seconds < 0.0)
    {
        seconds = 0.0;
    }
    return (uint32_t)((double)SAMPLE_RATE * seconds * (double)channels * (double)sizeof(int16_t));
}

static double wrap_cycle(double phase)
{
    if (phase < 0.0 || phase >= 1.0)
    {
        phase -= floor(phase);
    }
    return phase;
}

static void advance_cycle(double* phase, double hz, double rate)
{
    if (hz <= 0.0 || rate <= 0.0)
    {
        return;
    }
    *phase = wrap_cycle(*phase + hz / rate);
}

static double harmonic_gain_in_band(double fundamental_hz, double order, double requested_gain)
{
    if (fundamental_hz <= 0.0 || requested_gain <= 0.0 || order <= 0.0)
    {
        return 0.0;
    }
    if (fundamental_hz * order > SHAKER_TONE_MAX_HZ)
    {
        return 0.0;
    }
    return requested_gain;
}

static double harmonic_wave(const SoundData* data)
{
    double h2_gain = harmonic_gain_in_band(
        data->play_frequency, HARMONIC2_ORDER, data->harmonic2_gain);
    double h3_gain = harmonic_gain_in_band(
        data->play_frequency, HARMONIC3_ORDER, data->harmonic3_gain);
    double fund = sin(TWO_PI * data->phase);
    double h2 = h2_gain * sin(TWO_PI * HARMONIC2_ORDER * data->phase);
    double h3 = h3_gain * sin(TWO_PI * HARMONIC3_ORDER * data->phase);
    double norm = 1.0 + h2_gain + h3_gain;
    if (norm <= 0.0)
    {
        return 0.0;
    }
    return (fund + h2 + h3) / norm;
}

static double firing_envelope(const SoundData* data)
{
    double trough;
    double phase;

    if (data->pulse_hz <= 0.0 || data->pulse_depth <= 0.0)
    {
        return 1.0;
    }
    trough = 1.0 - data->pulse_depth;
    if (trough < 0.0)
    {
        trough = 0.0;
    }
    if (data->pulse_duty <= 0.0)
    {
        double lift = 0.5 * (sin(TWO_PI * data->pulse_phase) + 1.0);
        return trough + (1.0 - trough) * lift;
    }
    phase = data->pulse_phase - floor(data->pulse_phase);
    if (phase < data->pulse_duty)
    {
        return 1.0;
    }
    return trough;
}

static void smooth_play_params(SoundData* data, int audible, const ShakerCoeff* coeff)
{
    if (!audible)
    {
        return;
    }
    if (data->play_frequency <= 0.0)
    {
        data->play_frequency = data->curr_frequency;
    }
    else
    {
        data->play_frequency += (data->curr_frequency - data->play_frequency) * coeff->freq;
    }
    double target_amp = (double)data->curr_amplitude;
    if (data->play_amplitude <= 0.0)
    {
        data->play_amplitude = target_amp;
    }
    else
    {
        data->play_amplitude += (target_amp - data->play_amplitude) * coeff->amp;
    }
}

static int16_t sine_frame_then_advance(SoundData* data, double amplitude_scale, const ShakerCoeff* coeff)
{
    int audible = (data->curr_frequency > 0.0 && data->curr_amplitude > 0 && amplitude_scale > 0.0);
    double target_gain = audible ? 1.0 : 0.0;
    double gain_coeff = audible ? coeff->attack : coeff->release;
    data->play_gain += (target_gain - data->play_gain) * gain_coeff;
    if (data->play_gain < 0.0)
    {
        data->play_gain = 0.0;
    }
    if (data->play_gain > 1.0)
    {
        data->play_gain = 1.0;
    }
    smooth_play_params(data, audible, coeff);

    if (data->play_gain <= AUDIO_SILENCE_GAIN)
    {
        shaker_output_lowpass_reset(data);
        if (!audible)
        {
            data->play_frequency = 0.0;
            data->play_amplitude = 0.0;
        }
        return 0;
    }

    data->play_frequency = clamp_shaker_hz(data->play_frequency);
    if (data->play_frequency <= 0.0)
    {
        shaker_output_lowpass_reset(data);
        return 0;
    }

    double a = (data->play_amplitude / AMPLITUDE_UNIT) * amplitude_scale * data->play_gain;
    if (data->duration <= 0.0)
    {
        a *= custom_frequency_response_filter_correcting_output_gain(data->play_frequency);
    }
    if (a > SHAKER_MAX_DIGITAL_DRIVE)
    {
        a = SHAKER_MAX_DIGITAL_DRIVE;
    }
    if (a < 0.0)
    {
        a = 0.0;
    }

    double sample = a * harmonic_wave(data) * firing_envelope(data);
    sample = shaker_output_lowpass(data, sample, coeff->rate);
    double f = apply_noise(data->play_frequency, data->noise);
    f = clamp_shaker_hz(f);
    advance_cycle(&data->phase, f, coeff->rate);
    advance_cycle(&data->pulse_phase, data->pulse_hz, coeff->rate);
    return clamp_i16(sample * (double)INT16_MAX);
}

static double gear_envelope(const SoundData* data)
{
    if (data->duration <= 0.0)
    {
        return 0.0;
    }
    double t = data->curr_duration / data->duration;
    if (t < 0.0)
    {
        t = 0.0;
    }
    if (t > 1.0)
    {
        t = 1.0;
    }
    return exp(-GEAR_DECAY_K * t);
}

#define SHAKER_STREAM_ENGINE 0
#define SHAKER_STREAM_GEAR 1
#define SHAKER_ENGINE_FEEL_UNITY 1.0

static double engine_stream_amplitude_scale(int is_gear, const SoundData* data)
{
    if (is_gear || data->duration > 0.0)
    {
        if (data->curr_frequency <= 0.0)
        {
            return 0.0;
        }
        return gear_envelope(data);
    }
    return human_frequency_response_correcting_amplitude_scale(data->curr_frequency);
}

static void write_stream_frames(pa_stream* s, size_t length, SoundData* data, int is_gear)
{
    int channels = stream_channels(s);
    size_t bytes_per_frame = sizeof(int16_t) * (size_t)channels;
    size_t nsamp = length / sizeof(int16_t);
    if (nsamp == 0 || bytes_per_frame == 0)
    {
        return;
    }

    int16_t buffer[nsamp];
    memset(buffer, 0, nsamp * sizeof(int16_t));

    size_t frames = length / bytes_per_frame;
    ShakerCoeff coeff = coeffs_for_rate(stream_rate(s));
    size_t i;
    for (i = 0; i < frames; i++)
    {
        double fade = engine_stream_amplitude_scale(is_gear, data);
        write_frame(buffer, i, channels, sine_frame_then_advance(data, fade, &coeff));

        if (data->duration <= 0.0 || data->curr_frequency <= 0.0)
        {
            continue;
        }

        data->curr_duration += 1.0 / coeff.rate;
        if (data->curr_duration >= data->duration)
        {
            data->curr_duration = 0.0;
            data->curr_frequency = 0.0;
            data->curr_amplitude = 0;
        }
    }

    pa_stream_write(s, buffer, length, NULL, 0LL, PA_SEEK_RELATIVE);
}

void gear_sound_stream(pa_stream *s, size_t length, void *userdata) {
    write_stream_frames(s, length, (SoundData*)userdata, SHAKER_STREAM_GEAR);
}

void engine_sound_stream(pa_stream *s, size_t length, void *userdata) {
    write_stream_frames(s, length, (SoundData*)userdata, SHAKER_STREAM_ENGINE);
}



void stream_success_cb(pa_stream *stream, int success, void *userdata) {
    return;
}

static void unmute_shaker_sink_input(pa_context* context, pa_stream* stream)
{
    uint32_t stream_index;
    pa_operation* unmute;

    stream_index = pa_stream_get_index(stream);
    if (stream_index == PA_INVALID_INDEX)
    {
        return;
    }

    unmute = pa_context_set_sink_input_mute(
        context, stream_index, SHAKER_SINK_INPUT_UNMUTED, NULL, NULL);
    if (unmute == NULL)
    {
        return;
    }
    pa_operation_unref(unmute);
}

void stream_state_cb(pa_stream *s, void *mainloop) {
    pa_threaded_mainloop_signal(mainloop, 0);
}


int usb_generic_shaker_free(SoundDevice* sounddevice, pa_threaded_mainloop* mainloop)
{
    if(!mainloop)
    {
        // if this happens we are in trouble
        return 1;
    }

    pa_threaded_mainloop_lock(mainloop);

    int err = 0;
    if (sounddevice->stream)
    {
        pa_stream_disconnect(sounddevice->stream);
        pa_stream_unref(sounddevice->stream);
        // why is this wrong


    }

    pa_threaded_mainloop_unlock(mainloop);

    return err;
}

int usb_generic_shaker_init(SoundDevice* sounddevice, pa_threaded_mainloop* mainloop, pa_context* context, const char* devname, int volume, uint32_t channelmask, int channels, const char* streamname)
{
    custom_frequency_response_analysis_build_correcting_output_table();
    pa_threaded_mainloop_lock(mainloop);
    pa_stream *stream;

    if (channels < SOUND_CHANNEL_COUNT_MIN)
    {
        channels = SOUND_CHANNEL_COUNT_MIN;
    }
    if (channels > SOUND_CHANNEL_COUNT_MAX)
    {
        channels = SOUND_CHANNEL_COUNT_MAX;
    }

    // Create a playback stream
    pa_sample_spec sample_specifications;
    sample_specifications.format = FORMAT;
    sample_specifications.rate = SAMPLE_RATE;
    sample_specifications.channels = channels;


    pa_channel_map channel_map;
    pa_channel_map_init_auto(&channel_map, channels, PA_CHANNEL_MAP_DEFAULT);
    //pa_channel_map_init_stereo(&channel_map);

    // not sure about what to do here
    if(channels == SOUND_CHANNELS_STEREO)
    {
        pa_channel_map_parse(&channel_map, "front-left,front-right");
    }
    if(channels == SOUND_CHANNELS_QUAD)
    {
        pa_channel_map_parse(&channel_map, "front-left,front-right,rear-left,rear-right");
    }
    if(channels == SOUND_CHANNELS_SURROUND_51)
    {
        pa_channel_map_parse(&channel_map, "front-left,front-right,front-center,lfe,rear-left,rear-right");
    }
    if(channels == SOUND_CHANNELS_SURROUND_71)
    {
        pa_channel_map_parse(&channel_map, "front-left,front-right,front-center,lfe,rear-left,rear-right,side-left,side-right");
    }

    stream = pa_stream_new(context, streamname, &sample_specifications, &channel_map);
    pa_stream_set_state_callback(stream, stream_state_cb, mainloop);

    if (sounddevice->m.hapticeffect.effecttype == EFFECT_GEARSHIFT)
    {
        pa_stream_set_write_callback(stream, gear_sound_stream, &sounddevice->sounddata);
    }
    else
    {
        pa_stream_set_write_callback(stream, engine_sound_stream, &sounddevice->sounddata);
    }

    // recommended settings, i.e. server uses sensible values
    pa_buffer_attr buffer_attr;
    double latency_s = AUDIO_LATENCY_S;
    if (sounddevice->m.hapticeffect.effecttype == EFFECT_GEARSHIFT)
    {
        latency_s = AUDIO_GEAR_LATENCY_S;
    }
    buffer_attr.tlength = bytes_for_duration(channels, latency_s);
    buffer_attr.maxlength = (uint32_t) -1;
    buffer_attr.prebuf = (uint32_t) -1;
    buffer_attr.minreq = (uint32_t) -1;
    buffer_attr.fragsize = (uint32_t) -1;

    pa_cvolume cv;
    pa_cvolume_mute(&cv, channels);

    pa_volume_t channel_volume = PA_CLAMP_VOLUME((pa_volume_t)((volume / PERCENT_SCALE) * PA_VOLUME_NORM));

    pa_stream_flags_t stream_flags;
    stream_flags = PA_STREAM_INTERPOLATE_TIMING
        | PA_STREAM_AUTO_TIMING_UPDATE
        | PA_STREAM_ADJUST_LATENCY
        | PA_STREAM_START_UNMUTED;

    uint32_t active_channels = channelmask & sound_channel_mask_all(channels);
    if (active_channels == 0)
    {
        active_channels = sound_channel_mask_all(channels);
    }
    for (int ch = 0; ch < channels; ch++)
    {
        if ((active_channels & SOUND_CHANNEL_BIT(ch)) != 0)
        {
            cv.values[ch] = channel_volume;
        }
    }

    pa_stream_connect_playback(stream, devname, &buffer_attr, stream_flags, &cv, NULL);
    //pa_stream_connect_playback(stream, devname, &buffer_attr, stream_flags, &cv, NULL);

    // Wait for the stream to be ready
    for(;;)
    {
        pa_stream_state_t stream_state = pa_stream_get_state(stream);
        PA_STREAM_IS_GOOD(stream_state);
        //PA_STREAM_IS_GOOD(stream_state);
        if (stream_state == PA_STREAM_READY) break;
        pa_threaded_mainloop_wait(mainloop);
    }

    unmute_shaker_sink_input(context, stream);
    sounddevice->stream = stream;
    pa_threaded_mainloop_unlock(mainloop);
    return 0;
}
