#include <stdio.h>
#include <stdlib.h>

#include "simdevice.h"
#include "../helper/parameters.h"
#include "../helper/confighelper.h"
#include "../helper/devicenames.h"
#include "../simulatorapi/simapi/simapi/simdata.h"
#include "../slog/slog.h"


int update(SimDevice* this, SimData* simdata)
{
    if (this == NULL || this->vtable == NULL)
    {
        return -1;
    }

    return ((vtable*)this->vtable)->update(this, simdata);
}

int simdevfree(SimDevice* this)
{
    if (this == NULL || this->vtable == NULL)
    {
        return -1;
    }

    return ((vtable*)this->vtable)->free(this);
}

int devupdate(SimDevice* this, SimData* simdata)
{

    return 0;
}

static SimDevice* construct_device(DeviceSettings* ds, CargopitSettings* ms, SimInfo* siminfo)
{
    switch (ds->dev_type)
    {
        case SIMDEV_USB:
        {
            USBDevice* device = new_usb_device(ds, ms, siminfo);
            return device != NULL ? &device->m : NULL;
        }
        case SIMDEV_SOUND:
        {
            SoundDevice* device = new_sound_device(ds, ms, siminfo);
            return device != NULL ? &device->m : NULL;
        }
        case SIMDEV_SERIAL:
        {
            SerialDevice* device = new_serial_device(ds, ms, siminfo);
            return device != NULL ? &device->m : NULL;
        }
        default:
            return NULL;
    }
}

static bool device_is_skipped(const DeviceSettings* ds, const CargopitSettings* ms, int index)
{
    if (ds->enabled == false)
    {
        slogi("skipping disabled device at index %i", index);
        return true;
    }
    if (ds->dev_type == SIMDEV_SOUND && ms->disable_audio == true)
    {
        slogi("skipping configured sound device due to disable_audio being specified...");
        return true;
    }
    return false;
}

int devinit(SimDevice* simdevices, SimInfo* siminfo, int numdevices, DeviceSettings* ds, CargopitSettings* ms)
{
    slogi("initializing simdevices for simapi %i...", siminfo->simulatorapi);
    int devices = 0;

    for (int j = 0; j < numdevices; j++)
    {
        simdevices[j].initialized = false;
        if (device_is_skipped(&ds[j], ms, j))
        {
            continue;
        }

        SimDevice* device = construct_device(&ds[j], ms, siminfo);
        if (device == NULL)
        {
            slogw("Could not initialize %s device", cargopit_name_for(&CARGOPIT_DEVICE_CLASSES, ds[j].dev_type));
            continue;
        }
        simdevices[j] = *device;
        simdevices[j].initialized = true;
        simdevices[j].type = ds[j].dev_type;
        simdevices[j].fps = ds[j].fps;
        devices++;
    }

    return devices;
}
