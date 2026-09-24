#include "capture_log.h"

#include <pulse/pulseaudio.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct FakeMainloop {
    int locked;
};

struct FakeContext {
    int ready;
};

struct FakeStream {
    pa_sample_spec spec;
    pa_stream_request_cb_t write_cb;
    void* write_userdata;
    int ready;
    uint32_t index;
};

struct FakeProplist {
    int unused;
};

struct FakeOperation {
    int unused;
};

static struct FakeStream* active_stream;
static const uint32_t CAPTURE_STREAM_INDEX = 1;

pa_threaded_mainloop* pa_threaded_mainloop_new(void)
{
    capture_log_op("pa_threaded_mainloop_new", "");
    return (pa_threaded_mainloop*)calloc(1, sizeof(struct FakeMainloop));
}

pa_mainloop_api* pa_threaded_mainloop_get_api(pa_threaded_mainloop* loop)
{
    (void)loop;
    return (pa_mainloop_api*)(uintptr_t)1;
}

void pa_threaded_mainloop_lock(pa_threaded_mainloop* loop)
{
    struct FakeMainloop* mainloop = (struct FakeMainloop*)loop;
    if (mainloop != NULL)
    {
        mainloop->locked = 1;
    }
}

void pa_threaded_mainloop_unlock(pa_threaded_mainloop* loop)
{
    struct FakeMainloop* mainloop = (struct FakeMainloop*)loop;
    if (mainloop != NULL)
    {
        mainloop->locked = 0;
    }
}

int pa_threaded_mainloop_start(pa_threaded_mainloop* loop)
{
    (void)loop;
    capture_log_op("pa_threaded_mainloop_start", "");
    return 0;
}

void pa_threaded_mainloop_wait(pa_threaded_mainloop* loop)
{
    (void)loop;
}

void pa_threaded_mainloop_signal(pa_threaded_mainloop* loop, int wait_for_accept)
{
    (void)loop;
    (void)wait_for_accept;
}

void pa_threaded_mainloop_free(pa_threaded_mainloop* loop)
{
    capture_log_op("pa_threaded_mainloop_free", "");
    free(loop);
}

pa_context* pa_context_new(pa_mainloop_api* api, const char* name)
{
    (void)api;
    capture_log_op("pa_context_new", name != NULL ? name : "");
    return (pa_context*)calloc(1, sizeof(struct FakeContext));
}

void pa_context_set_state_callback(pa_context* context, pa_context_notify_cb_t cb, void* userdata)
{
    (void)context;
    (void)cb;
    (void)userdata;
}

int pa_context_connect(pa_context* context, const char* server, pa_context_flags_t flags, const pa_spawn_api* api)
{
    struct FakeContext* fake = (struct FakeContext*)context;

    (void)server;
    (void)flags;
    (void)api;
    if (fake == NULL)
    {
        return -1;
    }
    fake->ready = 1;
    capture_log_op("pa_context_connect", "");
    return 0;
}

pa_context_state_t pa_context_get_state(const pa_context* context)
{
    const struct FakeContext* fake = (const struct FakeContext*)context;
    if (fake != NULL && fake->ready)
    {
        return PA_CONTEXT_READY;
    }
    return PA_CONTEXT_FAILED;
}

int pa_context_errno(const pa_context* context)
{
    (void)context;
    return 0;
}

void pa_context_unref(pa_context* context)
{
    capture_log_op("pa_context_unref", "");
    free(context);
}

const char* pa_strerror(int error)
{
    (void)error;
    return "parity capture";
}

void pa_signal_done(void)
{
}

pa_proplist* pa_proplist_new(void)
{
    return (pa_proplist*)calloc(1, sizeof(struct FakeProplist));
}

void pa_proplist_free(pa_proplist* proplist)
{
    free(proplist);
}

int pa_proplist_sets(pa_proplist* proplist, const char* key, const char* value)
{
    (void)proplist;
    (void)key;
    (void)value;
    return 0;
}

pa_stream* pa_stream_new_with_proplist(pa_context* context, const char* name, const pa_sample_spec* spec,
                                       const pa_channel_map* map, pa_proplist* proplist)
{
    struct FakeStream* stream;

    (void)context;
    (void)map;
    (void)proplist;
    stream = calloc(1, sizeof(*stream));
    if (stream == NULL)
    {
        return NULL;
    }
    if (spec != NULL)
    {
        stream->spec = *spec;
    }
    stream->index = CAPTURE_STREAM_INDEX;
    active_stream = stream;
    capture_log_op("pa_stream_new", name != NULL ? name : "");
    return (pa_stream*)stream;
}

void pa_stream_set_state_callback(pa_stream* stream, pa_stream_notify_cb_t cb, void* userdata)
{
    (void)stream;
    (void)cb;
    (void)userdata;
}

void pa_stream_set_write_callback(pa_stream* stream, pa_stream_request_cb_t cb, void* userdata)
{
    struct FakeStream* fake = (struct FakeStream*)stream;
    if (fake == NULL)
    {
        return;
    }
    fake->write_cb = cb;
    fake->write_userdata = userdata;
}

pa_stream_state_t pa_stream_get_state(const pa_stream* stream)
{
    const struct FakeStream* fake = (const struct FakeStream*)stream;
    if (fake != NULL && fake->ready)
    {
        return PA_STREAM_READY;
    }
    return PA_STREAM_CREATING;
}

const pa_sample_spec* pa_stream_get_sample_spec(pa_stream* stream)
{
    struct FakeStream* fake = (struct FakeStream*)stream;
    if (fake == NULL)
    {
        return NULL;
    }
    return &fake->spec;
}

uint32_t pa_stream_get_index(const pa_stream* stream)
{
    const struct FakeStream* fake = (const struct FakeStream*)stream;
    if (fake == NULL)
    {
        return PA_INVALID_INDEX;
    }
    return fake->index;
}

int pa_stream_connect_playback(pa_stream* stream, const char* dev, const pa_buffer_attr* attr,
                               pa_stream_flags_t flags, const pa_cvolume* volume, pa_stream* sync_stream)
{
    struct FakeStream* fake = (struct FakeStream*)stream;

    (void)attr;
    (void)flags;
    (void)volume;
    (void)sync_stream;
    if (fake == NULL)
    {
        return -1;
    }
    fake->ready = 1;
    capture_log_op("pa_stream_connect_playback", dev != NULL ? dev : "default");
    return 0;
}

int pa_stream_disconnect(pa_stream* stream)
{
    (void)stream;
    capture_log_op("pa_stream_disconnect", "");
    return 0;
}

void pa_stream_unref(pa_stream* stream)
{
    if ((struct FakeStream*)stream == active_stream)
    {
        active_stream = NULL;
    }
    free(stream);
}

int pa_stream_write(pa_stream* stream, const void* data, size_t nbytes, pa_free_cb_t free_cb,
                    int64_t offset, pa_seek_mode_t seek)
{
    (void)stream;
    (void)free_cb;
    (void)offset;
    (void)seek;
    capture_log_bytes("pa_stream_write", data, nbytes);
    return 0;
}

pa_operation* pa_context_set_sink_input_mute(pa_context* context, uint32_t idx, int mute,
                                             pa_context_success_cb_t cb, void* userdata)
{
    char detail[64];

    (void)context;
    (void)cb;
    (void)userdata;
    snprintf(detail, sizeof(detail), "index=%u mute=%d", idx, mute);
    capture_log_op("pa_context_set_sink_input_mute", detail);
    return (pa_operation*)calloc(1, sizeof(struct FakeOperation));
}

void pa_operation_unref(pa_operation* operation)
{
    free(operation);
}

pa_channel_map* pa_channel_map_init_auto(pa_channel_map* map, unsigned channels, pa_channel_map_def_t def)
{
    (void)def;
    if (map == NULL)
    {
        return NULL;
    }
    memset(map, 0, sizeof(*map));
    map->channels = (uint8_t)channels;
    return map;
}

pa_cvolume* pa_cvolume_set(pa_cvolume* volume, unsigned channels, pa_volume_t value)
{
    unsigned i;

    if (volume == NULL)
    {
        return NULL;
    }
    volume->channels = (uint8_t)channels;
    for (i = 0; i < channels && i < PA_CHANNELS_MAX; i++)
    {
        volume->values[i] = value;
    }
    return volume;
}

pa_channel_map* pa_channel_map_parse(pa_channel_map* map, const char* s)
{
    (void)s;
    return map;
}

void capture_render_pcm(void)
{
    size_t channels;
    size_t bytes;

    if (active_stream == NULL || active_stream->write_cb == NULL)
    {
        return;
    }
    channels = active_stream->spec.channels > 0 ? active_stream->spec.channels : 1;
    bytes = CAPTURE_PCM_FRAMES * channels * sizeof(int16_t);
    active_stream->write_cb((pa_stream*)active_stream, bytes, active_stream->write_userdata);
}
