#include "capture_log.h"

#include <stdio.h>
#include <string.h>

static FILE* capture_out;
static int capture_tick;
static uint64_t capture_ns = CAPTURE_ORIGIN_NS;

void capture_log_open(const char* path)
{
    capture_out = fopen(path, "w");
    if (capture_out == NULL)
    {
        perror(path);
    }
}

void capture_log_close(void)
{
    if (capture_out != NULL)
    {
        fclose(capture_out);
        capture_out = NULL;
    }
}

void capture_set_tick(int tick)
{
    capture_tick = tick;
}

void capture_clock_reset(void)
{
    capture_ns = CAPTURE_ORIGIN_NS;
}

void capture_clock_advance_ms(uint64_t ms)
{
    capture_ns += ms * 1000000ULL;
}

uint64_t capture_clock_ms(void)
{
    return capture_ns / 1000000ULL;
}

static void capture_prefix(void)
{
    if (capture_out == NULL)
    {
        return;
    }
    fprintf(capture_out, "tick %d t_ms %llu ", capture_tick,
            (unsigned long long)capture_clock_ms());
}

void capture_log_op(const char* op, const char* detail)
{
    if (capture_out == NULL || op == NULL)
    {
        return;
    }
    capture_prefix();
    fprintf(capture_out, "%s %s\n", op, detail != NULL ? detail : "");
}

void capture_log_bytes(const char* op, const void* data, size_t size)
{
    const unsigned char* bytes = data;
    size_t i;

    if (capture_out == NULL || op == NULL)
    {
        return;
    }
    capture_prefix();
    fprintf(capture_out, "%s ", op);
    for (i = 0; i < size; i++)
    {
        fprintf(capture_out, "%02x", bytes[i]);
    }
    fprintf(capture_out, "\n");
}
