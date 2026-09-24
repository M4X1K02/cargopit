#ifndef PARITY_CAPTURE_LOG_H
#define PARITY_CAPTURE_LOG_H

#include <stddef.h>
#include <stdint.h>

#define CAPTURE_TICK_MS 16
#define CAPTURE_ORIGIN_NS 1000000000ULL
#define CAPTURE_PCM_FRAMES 48
#define CAPTURE_SIMLED_LED_COUNT 8
#define CAPTURE_FAKE_RUMBLE_PATH \
    "/sys/module/hid_fanatec/drivers/hid:fake/0003:0EB7:183B.0001/rumble"
#define CAPTURE_LEDSC_MARK "ledsc"

void capture_log_open(const char* path);
void capture_log_close(void);
void capture_set_tick(int tick);
void capture_log_op(const char* op, const char* detail);
void capture_log_bytes(const char* op, const void* data, size_t size);
void capture_clock_reset(void);
void capture_clock_advance_ms(uint64_t ms);
uint64_t capture_clock_ms(void);
void capture_render_pcm(void);

#endif
