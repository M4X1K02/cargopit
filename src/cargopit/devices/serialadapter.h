#ifndef _SERIALADAPTER_H
#define _SERIALADAPTER_H

#include <stdint.h>
#include <stdbool.h>
#include <libserialport.h>

#include "simdevice.h"

typedef struct
{
    char* portname;
    struct sp_port* port;
    uint8_t refs;
    bool open;
    bool openfail;
    bool busy;
}
cargopit_serial_device;

int cargopit_input_wait(uint8_t serialdevicenum);
int cargopit_wait_for_event(uint8_t serialdevicenum, int event);
int cargopit_serial_write(uint8_t serialdevicenum, void* data, size_t size, int timeout);
int cargopit_serial_write_block(uint8_t serialdevicenum, void* data, size_t size, int timeout);
int cargopit_serial_read_block(uint8_t serialdevicenum, void* data, size_t size, int timeout);
int cargopit_serial_open(SerialDevice* serialdevice, const char* port);
int cargopit_serial_share_port(uint8_t serialdevicenum);
int cargopit_serial_free(SerialDevice* serialdevice);

#endif
