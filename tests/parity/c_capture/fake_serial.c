#include "capture_log.h"

#include <libserialport.h>

#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct sp_port {
    char* name;
    int baud;
    int fd;
    int reply_ready;
};

static char simled_reply[8];

static const char* simled_reply_text(void)
{
    snprintf(simled_reply, sizeof(simled_reply), "%d\r", CAPTURE_SIMLED_LED_COUNT);
    return simled_reply;
}

static int write_contains_ledsc(const void* data, size_t size)
{
    const char* bytes = data;
    size_t mark_len = sizeof(CAPTURE_LEDSC_MARK) - 1;
    size_t i;

    if (size < mark_len)
    {
        return 0;
    }
    for (i = 0; i + mark_len <= size; i++)
    {
        if (memcmp(bytes + i, CAPTURE_LEDSC_MARK, mark_len) == 0)
        {
            return 1;
        }
    }
    return 0;
}

enum sp_return sp_get_port_by_name(const char* portname, struct sp_port** port_ptr)
{
    struct sp_port* port;

    if (portname == NULL || port_ptr == NULL)
    {
        return SP_ERR_ARG;
    }
    port = calloc(1, sizeof(*port));
    if (port == NULL)
    {
        return SP_ERR_MEM;
    }
    port->name = strdup(portname);
    port->fd = open("/dev/null", O_RDWR);
    if (port->name == NULL || port->fd < 0)
    {
        free(port->name);
        if (port->fd >= 0)
        {
            close(port->fd);
        }
        free(port);
        return SP_ERR_MEM;
    }
    *port_ptr = port;
    capture_log_op("sp_get_port_by_name", portname);
    return SP_OK;
}

enum sp_return sp_open(struct sp_port* port, enum sp_mode flags)
{
    char detail[128];

    (void)flags;
    if (port == NULL)
    {
        return SP_ERR_ARG;
    }
    snprintf(detail, sizeof(detail), "port=%s", port->name);
    capture_log_op("sp_open", detail);
    return SP_OK;
}

enum sp_return sp_close(struct sp_port* port)
{
    if (port == NULL)
    {
        return SP_ERR_ARG;
    }
    capture_log_op("sp_close", port->name);
    return SP_OK;
}

void sp_free_port(struct sp_port* port)
{
    if (port == NULL)
    {
        return;
    }
    capture_log_op("sp_free_port", port->name);
    free(port->name);
    if (port->fd >= 0)
    {
        close(port->fd);
    }
    free(port);
}

enum sp_return sp_set_baudrate(struct sp_port* port, int baudrate)
{
    char detail[160];

    if (port == NULL)
    {
        return SP_ERR_ARG;
    }
    port->baud = baudrate;
    snprintf(detail, sizeof(detail), "port=%s baud=%d", port->name, baudrate);
    capture_log_op("sp_set_baudrate", detail);
    return SP_OK;
}

enum sp_return sp_set_bits(struct sp_port* port, int bits)
{
    (void)port;
    (void)bits;
    return SP_OK;
}

enum sp_return sp_set_parity(struct sp_port* port, enum sp_parity parity)
{
    (void)port;
    (void)parity;
    return SP_OK;
}

enum sp_return sp_set_stopbits(struct sp_port* port, int stopbits)
{
    (void)port;
    (void)stopbits;
    return SP_OK;
}

enum sp_return sp_set_flowcontrol(struct sp_port* port, enum sp_flowcontrol flow)
{
    (void)port;
    (void)flow;
    return SP_OK;
}

enum sp_return sp_set_rts(struct sp_port* port, enum sp_rts rts)
{
    (void)port;
    (void)rts;
    return SP_OK;
}

enum sp_return sp_set_dtr(struct sp_port* port, enum sp_dtr dtr)
{
    (void)port;
    (void)dtr;
    return SP_OK;
}

enum sp_return sp_get_port_handle(const struct sp_port* port, void* result_ptr)
{
    int* fd = result_ptr;

    if (port == NULL || fd == NULL)
    {
        return SP_ERR_ARG;
    }
    *fd = port->fd;
    return SP_OK;
}

enum sp_return sp_blocking_write(struct sp_port* port, const void* buf, size_t count, unsigned int timeout_ms)
{
    char op[160];

    (void)timeout_ms;
    if (port == NULL || buf == NULL)
    {
        return SP_ERR_ARG;
    }
    snprintf(op, sizeof(op), "sp_blocking_write port=%s", port->name);
    capture_log_bytes(op, buf, count);
    if (write_contains_ledsc(buf, count))
    {
        port->reply_ready = 1;
    }
    return (enum sp_return)count;
}

enum sp_return sp_blocking_read(struct sp_port* port, void* buf, size_t count, unsigned int timeout_ms)
{
    const char* reply = simled_reply_text();
    size_t reply_len = strlen(reply);

    (void)timeout_ms;
    if (port == NULL || buf == NULL)
    {
        return SP_ERR_ARG;
    }
    if (!port->reply_ready || count < reply_len)
    {
        capture_log_op("sp_blocking_read", "empty");
        return 0;
    }
    memcpy(buf, reply, reply_len);
    port->reply_ready = 0;
    capture_log_bytes("sp_blocking_read", reply, reply_len);
    return (enum sp_return)reply_len;
}

enum sp_return sp_input_waiting(struct sp_port* port)
{
    if (port == NULL)
    {
        return SP_ERR_ARG;
    }
    if (port->reply_ready)
    {
        return (enum sp_return)strlen(simled_reply_text());
    }
    return 0;
}

enum sp_return sp_new_event_set(struct sp_event_set** result_ptr)
{
    if (result_ptr == NULL)
    {
        return SP_ERR_ARG;
    }
    *result_ptr = calloc(1, sizeof(struct sp_event_set));
    if (*result_ptr == NULL)
    {
        return SP_ERR_MEM;
    }
    return SP_OK;
}

enum sp_return sp_add_port_events(struct sp_event_set* event_set, const struct sp_port* port, enum sp_event mask)
{
    (void)event_set;
    (void)port;
    (void)mask;
    return SP_OK;
}

enum sp_return sp_wait(struct sp_event_set* event_set, unsigned int timeout_ms)
{
    char detail[64];

    (void)event_set;
    snprintf(detail, sizeof(detail), "timeout_ms=%u", timeout_ms);
    capture_log_op("sp_wait", detail);
    return SP_OK;
}

void sp_free_event_set(struct sp_event_set* event_set)
{
    free(event_set);
}

char* sp_last_error_message(void)
{
    return strdup("parity capture");
}

void sp_free_error_message(char* message)
{
    free(message);
}
