#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <glob.h>

#include "cslelitev3.h"

#include "../../helper/confighelper.h"
#include "../slog/slog.h"

static const char* SYSFS_RUMBLE_PATH =
    "/sys/module/hid_fanatec/drivers/hid:f*/0003:0EB7:183B.*/rumble";


int cslelitev3_update(USBDevice* usbdevice, int effecttype, int play)
{

    int res = 0;
    int value = 0;

    if (play > 0)
    {
        switch (effecttype)
        {
            case (EFFECT_TYRESLIP):
                value = 0xff0000;
                break;
            case (EFFECT_TYRELOCK):
                value = 0xff00;
                break;
            case (EFFECT_ABSBRAKES):
                value = 0xffff00;
                break;
        }
    }

    fprintf(usbdevice->filehandle, "%i\n", value);
    fflush(usbdevice->filehandle);

    return res;
}

int cslelitev3_free(USBDevice* usbdevice)
{
    if (usbdevice == NULL)
    {
        return 1;
    }

    free(usbdevice->dev);
    usbdevice->dev = NULL;

    if (usbdevice->filehandle != NULL)
    {
        fflush(usbdevice->filehandle);
        fclose(usbdevice->filehandle);
        usbdevice->filehandle = NULL;
    }

    return 0;
}

int cslelitev3_init(USBDevice* usbdevice)
{
    slogi("initializing CSL Elite V3 Pedals...");

    if (usbdevice == NULL)
    {
        return 1;
    }

    glob_t globlist = {0};
    int glob_result = glob(SYSFS_RUMBLE_PATH, GLOB_PERIOD, NULL, &globlist);
    if (glob_result != 0)
    {
        globfree(&globlist);
        if (glob_result == GLOB_ABORTED)
        {
            sloge("Permissions issue finding Club Sport Elite V3 Pedals");
            return 2;
        }

        sloge("Could not find attached Club Sport Elite V3 Pedals");
        return 1;
    }

    if (globlist.gl_pathc == 0 || globlist.gl_pathv == NULL)
    {
        globfree(&globlist);
        sloge("Could not find attached Club Sport Elite V3 Pedals");
        return 1;
    }

    usbdevice->dev = strdup(globlist.gl_pathv[0]);
    globfree(&globlist);

    if (usbdevice->dev == NULL)
    {
        sloge("Could not allocate the pedal device path");
        return 1;
    }

    usbdevice->filehandle = fopen(usbdevice->dev, "w");

    if (!usbdevice->filehandle)
    {
        sloge("Could not open pedal device...");
        free(usbdevice->dev);
        usbdevice->dev = NULL;
        return 1;
    }

    slogd("CSL Elite V3 Pedals Successfully initialized...");

    return 0;
}
