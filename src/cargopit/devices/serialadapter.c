#include <ctype.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>

#include "serialadapter.h"
#include "../slog/slog.h"

#define SERIAL_DEVICE_CAPACITY 20

static cargopit_serial_device cargopit_serial_devices[SERIAL_DEVICE_CAPACITY];
static pthread_once_t serial_locks_once = PTHREAD_ONCE_INIT;

static void init_serial_locks(void)
{
    for (int i = 0; i < SERIAL_DEVICE_CAPACITY; i++)
    {
        pthread_mutex_init(&cargopit_serial_devices[i].lock, NULL);
    }
}

static int msastrcicmp(char const *a, char const *b)
{
    for (;; a++, b++) {
        int d = tolower((unsigned char)*a) - tolower((unsigned char)*b);
        if (d != 0 || !*a)
            return d;
    }
}

static int check(enum sp_return result)
{
    char* error_message;

    switch (result)
    {
        case SP_ERR_ARG:
            return 1;
        case SP_ERR_FAIL:
            error_message = sp_last_error_message();
            sloge("error: serial write failed: %s", error_message);
            sp_free_error_message(error_message);
            return result;
        case SP_ERR_SUPP:
            printf("Error: Not supported.\n");
            return result;
        case SP_ERR_MEM:
            printf("Error: Couldn't allocate memory.\n");
            return result;
        case SP_OK:
        default:
            return result;
    }
}

static void close_serial_slot(cargopit_serial_device* dev)
{
    slogd("freeing physical device %s", dev->portname);
    sp_close(dev->port);
    sp_free_port(dev->port);
    free(dev->portname);
    dev->port = NULL;
    dev->portname = NULL;
    dev->open = false;
    dev->openfail = false;
}

int cargopit_serial_free(SerialDevice* serialdevice)
{
    if (serialdevice == NULL || serialdevice->id < 0 ||
        serialdevice->id >= SERIAL_DEVICE_CAPACITY)
    {
        return -1;
    }

    pthread_once(&serial_locks_once, init_serial_locks);
    cargopit_serial_device* dev = &cargopit_serial_devices[serialdevice->id];
    pthread_mutex_lock(&dev->lock);
    if (dev->open == true)
    {
        dev->refs--;
        if (dev->refs == 0)
        {
            close_serial_slot(dev);
        }
    }
    pthread_mutex_unlock(&dev->lock);
    return 0;
}

int cargopit_wait_for_event(uint8_t serialdevicenum, int event)
{
    if (serialdevicenum >= SERIAL_DEVICE_CAPACITY)
    {
        return -1;
    }

    slogt("serial device id %i", serialdevicenum);
    const cargopit_serial_device* cargopit_serial_dev = &cargopit_serial_devices[serialdevicenum];

    int retval;
    struct sp_event_set* eventSet = NULL;

    retval = sp_new_event_set(&eventSet);
    if (retval == SP_OK)
    {
        retval = sp_add_port_events(eventSet, cargopit_serial_dev->port, event);
        if (retval == SP_OK)
        {
            slogd("set event on port");
            retval = sp_wait(eventSet, 5000);
        }
        else
        {
            sloge("Unable to add events to port.");
            retval = -1;
        }
    }
    else
    {
        sloge("Unable to create new event set.");
        retval = -1;
    }
    sp_free_event_set(eventSet);

    return retval;
}

int cargopit_input_wait(uint8_t serialdevicenum)
{
    if (serialdevicenum >= SERIAL_DEVICE_CAPACITY)
    {
        return -1;
    }

    return sp_input_waiting(cargopit_serial_devices[serialdevicenum].port);
}

// Helper function to get and validate device
static cargopit_serial_device* cargopit_get_serial_device(uint8_t serialdevicenum)
{
    if (serialdevicenum >= SERIAL_DEVICE_CAPACITY)
    {
        return NULL;
    }
    pthread_once(&serial_locks_once, init_serial_locks);

    cargopit_serial_device* dev = &cargopit_serial_devices[serialdevicenum];
    slogt("serial device id %i", serialdevicenum);
    slogt("port name: %s, open %i, openfail %i", dev->portname, dev->open, dev->openfail);
    
    if(dev->port == NULL)
    {
        sloge("port is null");
    }
    
    return dev;
}

typedef enum
{
    SERIAL_IO_WRITE,
    SERIAL_IO_READ
}
SerialIoDirection;

/* Caller holds dev->lock. */
static int serial_io_locked(cargopit_serial_device* dev, SerialIoDirection direction, void* data, size_t size, unsigned int timeout_ms)
{
    if (dev->open == false)
    {
        return -1;
    }
    if (direction == SERIAL_IO_READ)
    {
        return sp_blocking_read(dev->port, data, size, timeout_ms);
    }
    return sp_blocking_write(dev->port, data, size, timeout_ms);
}

static int serial_io(uint8_t serialdevicenum, SerialIoDirection direction, void* data, size_t size, int timeout, bool wait_for_port)
{
    cargopit_serial_device* dev = cargopit_get_serial_device(serialdevicenum);
    int result;

    if (dev == NULL || timeout < 0)
    {
        return -1;
    }
    if (wait_for_port)
    {
        pthread_mutex_lock(&dev->lock);
    }
    else if (pthread_mutex_trylock(&dev->lock) != 0)
    {
        slogw("serial device data update ignored due to busy port");
        return -1;
    }
    result = serial_io_locked(dev, direction, data, size, (unsigned int) timeout);
    pthread_mutex_unlock(&dev->lock);
    slogt("serial io result is %i", result);
    return result;
}

/* Drops the frame if another device on the same port is mid-transfer. */
int cargopit_serial_write(uint8_t serialdevicenum, void* data, size_t size, int timeout)
{
    return serial_io(serialdevicenum, SERIAL_IO_WRITE, data, size, timeout, false);
}

int cargopit_serial_write_block(uint8_t serialdevicenum, void* data, size_t size, int timeout)
{
    return serial_io(serialdevicenum, SERIAL_IO_WRITE, data, size, timeout, true);
}

int cargopit_serial_read_block(uint8_t serialdevicenum, void* data, size_t size, int timeout)
{
    return serial_io(serialdevicenum, SERIAL_IO_READ, data, size, timeout, true);
}

#ifndef TIOCNXCL
#define TIOCNXCL 0x5429
#endif

int cargopit_serial_share_port(uint8_t serialdevicenum)
{
    cargopit_serial_device* dev = cargopit_get_serial_device(serialdevicenum);
    if (dev == NULL)
    {
        return -1;
    }

    if (dev->port == NULL || dev->open == false)
    {
        return -1;
    }

    int fd = -1;
    if (sp_get_port_handle(dev->port, &fd) != SP_OK || fd < 0)
    {
        slogw("could not get native serial handle to share port");
        return -1;
    }

    if (ioctl(fd, TIOCNXCL) != 0)
    {
        slogw("TIOCNXCL failed; Boxflat may still contend for the wheel");
    }

    struct termios term;
    if (tcgetattr(fd, &term) == 0)
    {
        term.c_cflag &= (tcflag_t)~HUPCL;
        if (tcsetattr(fd, TCSANOW, &term) != 0)
        {
            slogw("could not clear HUPCL on Moza serial");
        }
    }

    return 0;
}

int cargopit_serial_open(SerialDevice* serialdevice, const char* portdev)
{
    if (serialdevice == NULL || portdev == NULL)
    {
        return -1;
    }

    int serial_device_num = -1;
    bool notfound = true;
    slogi("looking to open physical serialdevice %s", portdev);
    for (int i = 0; i < SERIAL_DEVICE_CAPACITY; i++)
    {
        if(cargopit_serial_devices[i].open == true && cargopit_serial_devices[i].openfail == false)
        {
            if(msastrcicmp(cargopit_serial_devices[i].portname, portdev) == 0)
            {
                notfound = false;
                serial_device_num = i;
                cargopit_serial_devices[i].refs++;
                slogd("found exisiting handle to serial device %s", portdev);
            }
        }
    }

    if(notfound == true)
    {
        slogd("no existing connections found, looking to create new");
        int i = -1;
        for (int candidate = 0; candidate < SERIAL_DEVICE_CAPACITY; candidate++)
        {
            if (cargopit_serial_devices[candidate].open == false &&
                cargopit_serial_devices[candidate].openfail == false)
            {
                i = candidate;
                break;
            }
        }

        if (i < 0)
        {
            sloge("No serial device slots are available");
            return -1;
        }


        slogi("opening physical serial device...");
        int error = 0;
        char* port_name = strdup(portdev);
        if (port_name == NULL)
        {
            return -1;
        }

        cargopit_serial_devices[i].portname = port_name;

        struct sp_port* sp;
        slogd("Looking for port %s", port_name);
        error = check(sp_get_port_by_name(port_name, &sp));
        if (error != 0)
        {
            sloge("Error opening serial port");
            free(port_name);
            cargopit_serial_devices[i].portname = NULL;
            cargopit_serial_devices[i].open = false;
            cargopit_serial_devices[i].openfail = false;
            return -1;
        }

        slogd("Opening port");
        check(sp_open(sp, SP_MODE_READ | SP_MODE_WRITE));

        slogd("Setting port to %i 8N1, no flow control", serialdevice->baudrate);
        check(sp_set_baudrate(sp, serialdevice->baudrate));
        check(sp_set_bits(sp, 8));
        check(sp_set_parity(sp, SP_PARITY_NONE));
        check(sp_set_stopbits(sp, 1));
        check(sp_set_flowcontrol(sp, SP_FLOWCONTROL_NONE));

        check(sp_set_rts(sp, 1));
        check(sp_set_dtr(sp, 1));

        cargopit_serial_devices[i].port = sp;

        cargopit_serial_devices[i].open = true;
        cargopit_serial_devices[i].openfail = false;
        cargopit_serial_devices[i].refs++;

        serial_device_num = i;

        slogd("Successfully setup cargopit serial device...");
    }

    return serial_device_num;
}
