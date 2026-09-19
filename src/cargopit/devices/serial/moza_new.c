#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#include "moza.h"
#include "moza_new.h"
#include "../serialadapter.h"
#include "../../simulatorapi/simapi/simapi/simapi.h"
#include "../../slog/slog.h"

#define MOZA_TIMEOUT_MS 1000
#define MOZA_R9_BAUD 115200
#define MOZA_MESSAGE_START 0x7E
#define MOZA_CHECKSUM_MAGIC 0x0D
#define MOZA_GROUP_WHEEL_WRITE 0x3F
#define MOZA_DEVICE_WHEEL 0x17
#define MOZA_LED_COUNT 10
#define MOZA_COLOUR_TABLE_BYTES 40
#define MOZA_COLOUR_CHUNK_BYTES 20
#define MOZA_MAX_FRAME 64
#define MOZA_BRIGHTNESS_MAX 100
#define MOZA_RPM_WINDOW_START 0.80f
#define MOZA_LED_HYSTERESIS 0.4f
#define MOZA_ALERT_FLASH_MS 110
#define MOZA_BUTTON_WRITE_MS 80
#define MOZA_BRAKE_SHOW 0.04f
#define MOZA_BRAKE_HIDE 0.02f
#define MOZA_LOCK_SLIP 0.28f
#define MOZA_CLEAR_SLIP 0.18f
#define MOZA_SPIN_SLIP 0.22f
#define MOZA_CLEAR_SPIN 0.12f
#define MOZA_MIN_BRAKE 0.20f
#define MOZA_MIN_THROTTLE 0.22f
#define MOZA_MIN_SPEED_MS 4.0f
#define MOZA_KM_H_TO_M_S 0.277778f
#define MOZA_DR2_COLD_C 120.0f
#define MOZA_DR2_HOT_C 620.0f
#define MOZA_ACR_COLD_C 320.0f
#define MOZA_ACR_HOT_C 750.0f
#define MOZA_HEAT_OFF 0.06f
#define MOZA_YELLOW_FULL 0.55f
#define MOZA_NORMALISED_TEMP_MAX 1.5f
#define MOZA_PLACEHOLDER_SPAN_C 1.5f
#define MOZA_CORNER_COUNT 4
#define MOZA_BUTTON_FL 1
#define MOZA_BUTTON_FR 8
#define MOZA_BUTTON_RL 3
#define MOZA_BUTTON_RR 6

#define MOZA_RGB_GREEN_R 0
#define MOZA_RGB_GREEN_G 255
#define MOZA_RGB_GREEN_B 0
#define MOZA_RGB_YELLOW_R 255
#define MOZA_RGB_YELLOW_G 196
#define MOZA_RGB_YELLOW_B 0
#define MOZA_RGB_RED_R 255
#define MOZA_RGB_RED_G 0
#define MOZA_RGB_RED_B 0
#define MOZA_RGB_PURPLE_R 180
#define MOZA_RGB_PURPLE_G 0
#define MOZA_RGB_PURPLE_B 255
#define MOZA_RGB_BLUE_R 0
#define MOZA_RGB_BLUE_G 120
#define MOZA_RGB_BLUE_B 255
#define MOZA_RGB_ORANGE_R 255
#define MOZA_RGB_ORANGE_G 110
#define MOZA_RGB_ORANGE_B 0
#define MOZA_RGB_BRAKE_YELLOW_R 255
#define MOZA_RGB_BRAKE_YELLOW_G 220
#define MOZA_RGB_BRAKE_YELLOW_B 0

typedef enum
{
    MOZA_BAR_RPM = 0,
    MOZA_BAR_BRAKE,
    MOZA_BAR_LOCK,
    MOZA_BAR_ABS,
    MOZA_BAR_TC
}
MozaBarMode;

typedef struct
{
    uint8_t r;
    uint8_t g;
    uint8_t b;
}
MozaRgb;

typedef struct
{
    bool armed;
    bool braking;
    bool lock_latched;
    bool abs_latched;
    bool tc_latched;
    bool has_button_write;
    MozaBarMode bar;
    size_t last_rpm_lit;
    size_t last_brake_lit;
    MozaRgb last_corners[MOZA_CORNER_COUNT];
    struct timespec started;
    struct timespec last_button_write;
}
MozaLedState;

static MozaLedState g_moza_leds;

static const uint8_t g_brake_buttons[MOZA_CORNER_COUNT] = {
    MOZA_BUTTON_FL, MOZA_BUTTON_FR, MOZA_BUTTON_RL, MOZA_BUTTON_RR
};

static uint64_t timespec_to_ms(const struct timespec* ts)
{
    return (uint64_t)ts->tv_sec * 1000ull + (uint64_t)(ts->tv_nsec / 1000000L);
}

static uint64_t elapsed_ms(const struct timespec* start)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return timespec_to_ms(&now) - timespec_to_ms(start);
}

static uint8_t wire_checksum(const uint8_t* decoded, size_t length)
{
    uint32_t sum = MOZA_CHECKSUM_MAGIC;
    size_t i;
    for (i = 0; i < length; i++)
    {
        sum += decoded[i];
    }
    for (i = 2; i < length; i++)
    {
        if (decoded[i] == MOZA_MESSAGE_START)
        {
            sum += MOZA_MESSAGE_START;
        }
    }
    return (uint8_t)(sum & 0xFFu);
}

static size_t byte_stuff(const uint8_t* frame, size_t length, uint8_t* out, size_t out_max)
{
    if (length < 2 || out_max < length)
    {
        return 0;
    }

    size_t n = 0;
    out[n++] = frame[0];
    out[n++] = frame[1];
    size_t i;
    for (i = 2; i < length; i++)
    {
        if (n + 1 >= out_max)
        {
            return 0;
        }
        out[n++] = frame[i];
        if (frame[i] == MOZA_MESSAGE_START)
        {
            out[n++] = MOZA_MESSAGE_START;
        }
    }
    return n;
}

static int write_frame(int serial_id, const uint8_t* cmd, size_t cmd_len,
                       const uint8_t* payload, size_t payload_len)
{
    uint8_t decoded[MOZA_MAX_FRAME];
    size_t body = cmd_len + payload_len;
    if (body > 255 || 5 + body > MOZA_MAX_FRAME)
    {
        return -1;
    }

    decoded[0] = MOZA_MESSAGE_START;
    decoded[1] = (uint8_t)body;
    decoded[2] = MOZA_GROUP_WHEEL_WRITE;
    decoded[3] = MOZA_DEVICE_WHEEL;
    memcpy(decoded + 4, cmd, cmd_len);
    memcpy(decoded + 4 + cmd_len, payload, payload_len);
    size_t decoded_len = 4 + body;
    decoded[decoded_len] = wire_checksum(decoded, decoded_len);
    decoded_len++;

    uint8_t stuffed[MOZA_MAX_FRAME];
    size_t stuffed_len = byte_stuff(decoded, decoded_len, stuffed, sizeof(stuffed));
    if (stuffed_len == 0)
    {
        return -1;
    }

    int wrote = cargopit_serial_write((uint8_t)serial_id, stuffed, stuffed_len, MOZA_TIMEOUT_MS);
    if (wrote < 0 || (size_t)wrote != stuffed_len)
    {
        return -1;
    }
    return 0;
}

static int send_rpm_bitmask(int serial_id, uint32_t active)
{
    uint32_t window = (1u << MOZA_LED_COUNT) - 1u;
    uint8_t payload[8];
    memcpy(payload, &active, 4);
    memcpy(payload + 4, &window, 4);
    const uint8_t cmd[] = { 0x1A, 0x00 };
    return write_frame(serial_id, cmd, sizeof(cmd), payload, sizeof(payload));
}

static int send_buttons_bitmask(int serial_id, uint16_t mask)
{
    uint8_t payload[2];
    payload[0] = (uint8_t)(mask & 0xFFu);
    payload[1] = (uint8_t)((mask >> 8) & 0xFFu);
    const uint8_t cmd[] = { 0x1A, 0x01 };
    return write_frame(serial_id, cmd, sizeof(cmd), payload, sizeof(payload));
}

static int send_colour_table(int serial_id, uint8_t group, const uint8_t table[MOZA_COLOUR_TABLE_BYTES])
{
    const uint8_t cmd[] = { 0x19, group };
    if (write_frame(serial_id, cmd, sizeof(cmd), table, MOZA_COLOUR_CHUNK_BYTES) < 0)
    {
        return -1;
    }
    return write_frame(serial_id, cmd, sizeof(cmd), table + MOZA_COLOUR_CHUNK_BYTES,
                       MOZA_COLOUR_CHUNK_BYTES);
}

static void fill_solid_table(uint8_t table[MOZA_COLOUR_TABLE_BYTES], uint8_t r, uint8_t g, uint8_t b)
{
    size_t i;
    for (i = 0; i < MOZA_LED_COUNT; i++)
    {
        size_t base = i * 4;
        table[base] = (uint8_t)i;
        table[base + 1] = r;
        table[base + 2] = g;
        table[base + 3] = b;
    }
}

static void fill_rpm_table(uint8_t table[MOZA_COLOUR_TABLE_BYTES])
{
    static const uint8_t rgb[MOZA_LED_COUNT][3] = {
        { MOZA_RGB_GREEN_R, MOZA_RGB_GREEN_G, MOZA_RGB_GREEN_B },
        { MOZA_RGB_GREEN_R, MOZA_RGB_GREEN_G, MOZA_RGB_GREEN_B },
        { MOZA_RGB_GREEN_R, MOZA_RGB_GREEN_G, MOZA_RGB_GREEN_B },
        { MOZA_RGB_GREEN_R, MOZA_RGB_GREEN_G, MOZA_RGB_GREEN_B },
        { MOZA_RGB_YELLOW_R, MOZA_RGB_YELLOW_G, MOZA_RGB_YELLOW_B },
        { MOZA_RGB_YELLOW_R, MOZA_RGB_YELLOW_G, MOZA_RGB_YELLOW_B },
        { MOZA_RGB_YELLOW_R, MOZA_RGB_YELLOW_G, MOZA_RGB_YELLOW_B },
        { MOZA_RGB_RED_R, MOZA_RGB_RED_G, MOZA_RGB_RED_B },
        { MOZA_RGB_RED_R, MOZA_RGB_RED_G, MOZA_RGB_RED_B },
        { MOZA_RGB_RED_R, MOZA_RGB_RED_G, MOZA_RGB_RED_B }
    };
    size_t i;
    for (i = 0; i < MOZA_LED_COUNT; i++)
    {
        size_t base = i * 4;
        table[base] = (uint8_t)i;
        table[base + 1] = rgb[i][0];
        table[base + 2] = rgb[i][1];
        table[base + 3] = rgb[i][2];
    }
}

static int arm_telemetry(int serial_id)
{
    const uint8_t mode_cmd[] = { 0x1C, 0x00 };
    const uint8_t mode_on[] = { 1 };
    const uint8_t idle_cmd[] = { 0x1D, 0x00 };
    const uint8_t idle_off[] = { 0 };
    const uint8_t rpm_bright_cmd[] = { 0x1B, 0x00, 0xFF };
    const uint8_t btn_bright_cmd[] = { 0x1B, 0x01, 0xFF };
    const uint8_t bright[] = { MOZA_BRIGHTNESS_MAX };
    uint8_t rpm_table[MOZA_COLOUR_TABLE_BYTES];

    if (write_frame(serial_id, mode_cmd, sizeof(mode_cmd), mode_on, sizeof(mode_on)) < 0)
    {
        return -1;
    }
    if (write_frame(serial_id, rpm_bright_cmd, sizeof(rpm_bright_cmd), bright, sizeof(bright)) < 0)
    {
        return -1;
    }
    fill_rpm_table(rpm_table);
    if (send_colour_table(serial_id, 0x00, rpm_table) < 0)
    {
        return -1;
    }
    if (write_frame(serial_id, btn_bright_cmd, sizeof(btn_bright_cmd), bright, sizeof(bright)) < 0)
    {
        return -1;
    }
    if (write_frame(serial_id, idle_cmd, sizeof(idle_cmd), idle_off, sizeof(idle_off)) < 0)
    {
        return -1;
    }
    return 0;
}

static uint32_t mask_from_lit(size_t lit)
{
    if (lit == 0)
    {
        return 0;
    }
    if (lit >= 32)
    {
        return UINT32_MAX;
    }
    return (1u << lit) - 1u;
}

static size_t apply_hysteresis(float continuous, size_t raw, size_t last_lit)
{
    if (raw == 0 || raw == last_lit)
    {
        return raw;
    }
    float last = (float)last_lit;
    if (raw > last_lit && continuous < last + 0.5f + MOZA_LED_HYSTERESIS)
    {
        return last_lit;
    }
    if (raw < last_lit && continuous > last - 0.5f - MOZA_LED_HYSTERESIS)
    {
        return last_lit;
    }
    return raw;
}

static void rpm_to_bitmask(float rpm, float redline, float idle, size_t last_lit,
                           uint32_t* mask, size_t* lit_out)
{
    if (redline <= idle)
    {
        *mask = 0;
        *lit_out = 0;
        return;
    }
    float span = redline - idle;
    float window_start = idle + MOZA_RPM_WINDOW_START * span;
    if (rpm < window_start || redline <= window_start)
    {
        *mask = 0;
        *lit_out = 0;
        return;
    }
    float frac = (rpm - window_start) / (redline - window_start);
    if (frac < 0.0f)
    {
        frac = 0.0f;
    }
    if (frac > 1.0f)
    {
        frac = 1.0f;
    }
    float continuous = frac * (float)MOZA_LED_COUNT;
    size_t raw = (size_t)roundf(continuous);
    if (raw > MOZA_LED_COUNT)
    {
        raw = MOZA_LED_COUNT;
    }
    size_t lit = apply_hysteresis(continuous, raw, last_lit);
    *mask = mask_from_lit(lit);
    *lit_out = lit;
}

static void frac_to_bitmask(float frac, size_t last_lit, uint32_t* mask, size_t* lit_out)
{
    if (!(frac > 0.0f) || !isfinite(frac))
    {
        *mask = 0;
        *lit_out = 0;
        return;
    }
    if (frac > 1.0f)
    {
        frac = 1.0f;
    }
    float continuous = frac * (float)MOZA_LED_COUNT;
    size_t raw = (size_t)roundf(continuous);
    if (raw < 1)
    {
        raw = 1;
    }
    if (raw > MOZA_LED_COUNT)
    {
        raw = MOZA_LED_COUNT;
    }
    size_t lit = apply_hysteresis(continuous, raw, last_lit);
    if (lit < 1)
    {
        lit = 1;
    }
    *mask = mask_from_lit(lit);
    *lit_out = lit;
}

static float clamp01(float v)
{
    if (v < 0.0f)
    {
        return 0.0f;
    }
    if (v > 1.0f)
    {
        return 1.0f;
    }
    return v;
}

static float peak_positive_slip(const SimData* simdata)
{
    float peak = 0.0f;
    int i;
    for (i = 0; i < MOZA_CORNER_COUNT; i++)
    {
        float slip = (float)simdata->tyreslipratio[i];
        if (slip > peak)
        {
            peak = slip;
        }
    }
    return peak;
}

static MozaBarMode update_alerts(MozaLedState* state, const SimData* simdata)
{
    float speed_ms = (float)simdata->velocity * MOZA_KM_H_TO_M_S;
    float lock_slip = peak_positive_slip(simdata);
    bool locking = (float)simdata->brake >= MOZA_MIN_BRAKE
        && speed_ms >= MOZA_MIN_SPEED_MS
        && lock_slip >= MOZA_LOCK_SLIP;
    bool abs_on = simdata->abs > 0.5;

    if (state->lock_latched)
    {
        if (!locking || lock_slip < MOZA_CLEAR_SLIP)
        {
            state->lock_latched = false;
        }
    }
    else if (locking)
    {
        state->lock_latched = true;
    }

    if (state->abs_latched)
    {
        if (!abs_on)
        {
            state->abs_latched = false;
        }
    }
    else if (abs_on)
    {
        state->abs_latched = true;
    }

    state->tc_latched = false;

    if (state->lock_latched)
    {
        return MOZA_BAR_LOCK;
    }
    if (state->abs_latched)
    {
        return MOZA_BAR_ABS;
    }
    return MOZA_BAR_RPM;
}

static MozaRgb heat_to_rgb(float heat)
{
    MozaRgb off = { 0, 0, 0 };
    if (!isfinite(heat) || heat < MOZA_HEAT_OFF)
    {
        return off;
    }

    MozaRgb yellow = {
        MOZA_RGB_BRAKE_YELLOW_R, MOZA_RGB_BRAKE_YELLOW_G, MOZA_RGB_BRAKE_YELLOW_B
    };
    MozaRgb red = { MOZA_RGB_RED_R, MOZA_RGB_RED_G, MOZA_RGB_RED_B };

    if (heat <= MOZA_YELLOW_FULL)
    {
        float t = clamp01((heat - MOZA_HEAT_OFF) / (MOZA_YELLOW_FULL - MOZA_HEAT_OFF));
        MozaRgb out;
        out.r = (uint8_t)roundf((float)yellow.r * t);
        out.g = (uint8_t)roundf((float)yellow.g * t);
        out.b = (uint8_t)roundf((float)yellow.b * t);
        return out;
    }

    float t = clamp01((heat - MOZA_YELLOW_FULL) / (1.0f - MOZA_YELLOW_FULL));
    MozaRgb out;
    out.r = (uint8_t)roundf((float)yellow.r + ((float)red.r - (float)yellow.r) * t);
    out.g = (uint8_t)roundf((float)yellow.g + ((float)red.g - (float)yellow.g) * t);
    out.b = (uint8_t)roundf((float)yellow.b + ((float)red.b - (float)yellow.b) * t);
    return out;
}

static float heat_scaled(float temp_c, bool acr)
{
    if (!isfinite(temp_c) || temp_c <= 0.0f)
    {
        return 0.0f;
    }
    if (temp_c <= MOZA_NORMALISED_TEMP_MAX)
    {
        return clamp01(temp_c);
    }
    float cold = acr ? MOZA_ACR_COLD_C : MOZA_DR2_COLD_C;
    float hot = acr ? MOZA_ACR_HOT_C : MOZA_DR2_HOT_C;
    return clamp01((temp_c - cold) / (hot - cold));
}

static bool temps_are_placeholder(const double temps[MOZA_CORNER_COUNT])
{
    double min = temps[0];
    double max = temps[0];
    int i;
    for (i = 1; i < MOZA_CORNER_COUNT; i++)
    {
        if (temps[i] < min)
        {
            min = temps[i];
        }
        if (temps[i] > max)
        {
            max = temps[i];
        }
    }
    return (max - min) < MOZA_PLACEHOLDER_SPAN_C;
}

static void corners_from_temps(const SimData* simdata, MozaRgb out[MOZA_CORNER_COUNT])
{
    bool acr = (simdata->simexe == SIMULATOREXE_ASSETTO_CORSA_RALLY);
    int i;
    if (acr && temps_are_placeholder(simdata->braketemp))
    {
        memset(out, 0, sizeof(MozaRgb) * MOZA_CORNER_COUNT);
        return;
    }
    for (i = 0; i < MOZA_CORNER_COUNT; i++)
    {
        out[i] = heat_to_rgb(heat_scaled((float)simdata->braketemp[i], acr));
    }
}

static bool corner_on(MozaRgb c)
{
    return c.r > 0 || c.g > 0 || c.b > 0;
}

static uint16_t corners_to_bitmask(const MozaRgb corners[MOZA_CORNER_COUNT])
{
    uint16_t mask = 0;
    int i;
    for (i = 0; i < MOZA_CORNER_COUNT; i++)
    {
        if (corner_on(corners[i]))
        {
            mask |= (uint16_t)(1u << g_brake_buttons[i]);
        }
    }
    return mask;
}

static void corners_to_table(const MozaRgb corners[MOZA_CORNER_COUNT],
                             uint8_t table[MOZA_COLOUR_TABLE_BYTES])
{
    size_t i;
    memset(table, 0, MOZA_COLOUR_TABLE_BYTES);
    for (i = 0; i < MOZA_LED_COUNT; i++)
    {
        table[i * 4] = (uint8_t)i;
    }
    for (i = 0; i < MOZA_CORNER_COUNT; i++)
    {
        size_t idx = g_brake_buttons[i];
        if (idx >= MOZA_LED_COUNT)
        {
            continue;
        }
        size_t base = idx * 4;
        table[base] = (uint8_t)idx;
        table[base + 1] = corners[i].r;
        table[base + 2] = corners[i].g;
        table[base + 3] = corners[i].b;
    }
}

static bool corners_equal(const MozaRgb a[MOZA_CORNER_COUNT], const MozaRgb b[MOZA_CORNER_COUNT])
{
    return memcmp(a, b, sizeof(MozaRgb) * MOZA_CORNER_COUNT) == 0;
}

static int apply_bar_colours(int serial_id, MozaBarMode mode)
{
    uint8_t table[MOZA_COLOUR_TABLE_BYTES];
    switch (mode)
    {
        case MOZA_BAR_BRAKE:
        case MOZA_BAR_LOCK:
            fill_solid_table(table, MOZA_RGB_PURPLE_R, MOZA_RGB_PURPLE_G, MOZA_RGB_PURPLE_B);
            break;
        case MOZA_BAR_ABS:
            fill_solid_table(table, MOZA_RGB_BLUE_R, MOZA_RGB_BLUE_G, MOZA_RGB_BLUE_B);
            break;
        case MOZA_BAR_TC:
        case MOZA_BAR_RPM:
        default:
            fill_rpm_table(table);
            break;
    }
    return send_colour_table(serial_id, 0x00, table);
}

static bool alert_blink_on(const MozaLedState* state)
{
    return ((elapsed_ms(&state->started) / MOZA_ALERT_FLASH_MS) % 2ull) == 0;
}

static int write_brake_buttons(int serial_id, const MozaRgb corners[MOZA_CORNER_COUNT])
{
    uint8_t table[MOZA_COLOUR_TABLE_BYTES];
    corners_to_table(corners, table);
    if (send_colour_table(serial_id, 0x01, table) < 0)
    {
        return -1;
    }
    return send_buttons_bitmask(serial_id, corners_to_bitmask(corners));
}

static void reset_led_state(MozaLedState* state)
{
    memset(state, 0, sizeof(*state));
    clock_gettime(CLOCK_MONOTONIC, &state->started);
}

static int blank_wheel(int serial_id, MozaLedState* state)
{
    MozaRgb off[MOZA_CORNER_COUNT];
    memset(off, 0, sizeof(off));
    write_brake_buttons(serial_id, off);
    send_rpm_bitmask(serial_id, 0);
    if (state->bar != MOZA_BAR_RPM)
    {
        apply_bar_colours(serial_id, MOZA_BAR_RPM);
    }
    reset_led_state(state);
    state->armed = true;
    return 0;
}

static MozaBarMode desired_bar(MozaLedState* state, const SimData* simdata, MozaBarMode alert)
{
    if ((float)simdata->brake >= MOZA_BRAKE_SHOW)
    {
        state->braking = true;
    }
    else if ((float)simdata->brake < MOZA_BRAKE_HIDE)
    {
        state->braking = false;
    }

    if (alert != MOZA_BAR_RPM)
    {
        return alert;
    }
    if (state->braking)
    {
        return MOZA_BAR_BRAKE;
    }
    return MOZA_BAR_RPM;
}

static uint32_t bar_bitmask(MozaLedState* state, const SimData* simdata, MozaBarMode bar)
{
    uint32_t mask = 0;
    size_t lit = 0;
    switch (bar)
    {
        case MOZA_BAR_BRAKE:
            frac_to_bitmask((float)simdata->brake, state->last_brake_lit, &mask, &lit);
            state->last_brake_lit = lit;
            return mask;
        case MOZA_BAR_LOCK:
        case MOZA_BAR_ABS:
            if ((float)simdata->brake >= MOZA_BRAKE_HIDE)
            {
                frac_to_bitmask((float)simdata->brake, state->last_brake_lit, &mask, &lit);
                state->last_brake_lit = lit;
            }
            else
            {
                mask = mask_from_lit(MOZA_LED_COUNT);
                state->last_brake_lit = MOZA_LED_COUNT;
            }
            return alert_blink_on(state) ? mask : 0;
        case MOZA_BAR_TC:
        case MOZA_BAR_RPM:
        default:
            state->last_brake_lit = 0;
            rpm_to_bitmask((float)simdata->rpms, (float)simdata->maxrpm,
                           (float)simdata->idlerpm, state->last_rpm_lit, &mask, &lit);
            state->last_rpm_lit = lit;
            return mask;
    }
}

int moza_new_update(SerialDevice* serialdevice, SimData* simdata)
{
    if (serialdevice->id < 0)
    {
        return -1;
    }

    int serial_id = serialdevice->id;
    if (!g_moza_leds.armed)
    {
        if (arm_telemetry(serial_id) < 0)
        {
            slogw("Moza R9 telemetry arm failed");
            return -1;
        }
        reset_led_state(&g_moza_leds);
        g_moza_leds.armed = true;
        slogi("Moza R9 telemetry mode armed");
    }

    if (simdata->simstatus != SIMAPI_STATUS_ACTIVEPLAY)
    {
        return blank_wheel(serial_id, &g_moza_leds);
    }

    MozaBarMode alert = update_alerts(&g_moza_leds, simdata);
    MozaBarMode bar = desired_bar(&g_moza_leds, simdata, alert);
    if (bar != g_moza_leds.bar)
    {
        if (apply_bar_colours(serial_id, bar) == 0)
        {
            if (bar == MOZA_BAR_RPM)
            {
                g_moza_leds.last_brake_lit = 0;
            }
            g_moza_leds.bar = bar;
        }
    }

    MozaRgb corners[MOZA_CORNER_COUNT];
    corners_from_temps(simdata, corners);
    bool corners_changed = !corners_equal(corners, g_moza_leds.last_corners);
    bool on_off_changed = corners_to_bitmask(corners) != corners_to_bitmask(g_moza_leds.last_corners);
    bool button_due = !g_moza_leds.has_button_write
        || elapsed_ms(&g_moza_leds.last_button_write) >= MOZA_BUTTON_WRITE_MS;
    if (corners_changed && (on_off_changed || button_due))
    {
        if (write_brake_buttons(serial_id, corners) == 0)
        {
            memcpy(g_moza_leds.last_corners, corners, sizeof(corners));
            clock_gettime(CLOCK_MONOTONIC, &g_moza_leds.last_button_write);
            g_moza_leds.has_button_write = true;
        }
    }

    uint32_t rpm_bits = bar_bitmask(&g_moza_leds, simdata, g_moza_leds.bar);
    if (send_rpm_bitmask(serial_id, rpm_bits) < 0)
    {
        slogw("Moza R9 RPM bitmask write failed");
        g_moza_leds.armed = false;
        return -1;
    }
    return 0;
}

int moza_new_init(SerialDevice* serialdevice, const char* portdev)
{
    if (serialdevice->baudrate < MOZA_R9_BAUD)
    {
        serialdevice->baudrate = MOZA_R9_BAUD;
    }

    int id = cargopit_serial_open(serialdevice, portdev);
    if (id < 0)
    {
        return id;
    }
    serialdevice->id = id;
    cargopit_serial_share_port((uint8_t)id);
    reset_led_state(&g_moza_leds);
    if (arm_telemetry(id) == 0)
    {
        g_moza_leds.armed = true;
        slogi("Moza R9 wheel opened on %s", portdev);
    }
    else
    {
        slogw("Moza R9 opened on %s but telemetry arm will retry", portdev);
    }
    return 0;
}
