#include "capture_log.h"

#include <hidapi/hidapi.h>

#include <stdio.h>
#include <stdlib.h>

struct hid_device_ {
    unsigned short vendor_id;
    unsigned short product_id;
    int id;
};

static int next_hid_id = 1;

int hid_init(void)
{
    capture_log_op("hid_init", "");
    return 0;
}

int hid_exit(void)
{
    capture_log_op("hid_exit", "");
    return 0;
}

hid_device* hid_open(unsigned short vendor_id, unsigned short product_id, const wchar_t* serial_number)
{
    hid_device* device;
    char detail[80];

    (void)serial_number;
    device = calloc(1, sizeof(*device));
    if (device == NULL)
    {
        return NULL;
    }
    device->vendor_id = vendor_id;
    device->product_id = product_id;
    device->id = next_hid_id++;
    snprintf(detail, sizeof(detail), "id=%d vid=0x%04x pid=0x%04x", device->id, vendor_id, product_id);
    capture_log_op("hid_open", detail);
    return device;
}

void hid_close(hid_device* device)
{
    char detail[32];

    if (device == NULL)
    {
        return;
    }
    snprintf(detail, sizeof(detail), "id=%d", device->id);
    capture_log_op("hid_close", detail);
    free(device);
}

int hid_write(hid_device* device, const unsigned char* data, size_t length)
{
    char op[32];

    if (device == NULL || data == NULL)
    {
        return -1;
    }
    snprintf(op, sizeof(op), "hid_write id=%d", device->id);
    capture_log_bytes(op, data, length);
    return (int)length;
}

int hid_send_feature_report(hid_device* device, const unsigned char* data, size_t length)
{
    char op[48];

    if (device == NULL || data == NULL)
    {
        return -1;
    }
    snprintf(op, sizeof(op), "hid_send_feature_report id=%d", device->id);
    capture_log_bytes(op, data, length);
    return (int)length;
}

const wchar_t* hid_error(hid_device* device)
{
    (void)device;
    return L"parity capture";
}
