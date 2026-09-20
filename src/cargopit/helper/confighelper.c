#include <dirent.h>
#include <stdio.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <ctype.h>

#include <libxml/parser.h>
#include <libxml/xmlreader.h>
#include <libxml/tree.h>

#include "confighelper.h"
#include "dirhelper.h"

#include "../slog/slog.h"
#include "parameters.h"



#include "../simulatorapi/simapi/simapi/simmapper.h"

#include <pulse/pulseaudio.h>

int strcicmp(char const *a, char const *b)
{
    for (;; a++, b++) {
        int d = tolower((unsigned char)*a) - tolower((unsigned char)*b);
        if (d != 0 || !*a)
            return d;
    }
}

static uint32_t lookup_pipewire_stream_volume(const config_setting_t* device_settings)
{
    int value;

    if (device_settings == NULL)
    {
        return SOUND_STREAM_VOLUME_UNITY;
    }
    value = SOUND_STREAM_VOLUME_UNITY;
    if (config_setting_lookup_int(device_settings, "streamVolume", &value) == CONFIG_FALSE)
    {
        if (config_setting_lookup_int(device_settings, "volume", &value) == CONFIG_FALSE)
        {
            return SOUND_STREAM_VOLUME_UNITY;
        }
    }
    if (value < SOUND_STREAM_VOLUME_MIN)
    {
        return SOUND_STREAM_VOLUME_MIN;
    }
    if (value > SOUND_STREAM_VOLUME_UNITY)
    {
        return SOUND_STREAM_VOLUME_UNITY;
    }
    return (uint32_t)value;
}

int strtoeffecttype(const char* effect, DeviceSettings* ds)
{
    ds->is_valid = false;

    if (strcicmp(effect, "Engine") == 0)
    {
        ds->is_valid = true;
        ds->hapticsettings.effect_type = EFFECT_ENGINERPM;
    }
    if (strcicmp(effect, "Gear") == 0)
    {
        ds->is_valid = true;
        ds->hapticsettings.effect_type = EFFECT_GEARSHIFT;
    }
    if (strcicmp(effect, "Suspension") == 0)
    {
        ds->is_valid = true;
        ds->hapticsettings.effect_type = EFFECT_SUSPENSION;
    }
    if (strcicmp(effect, "ABS") == 0)
    {
        ds->is_valid = true;
        slogt("found abas effect set");
        ds->hapticsettings.effect_type = EFFECT_ABSBRAKES;
    }
    if ((strcicmp(effect, "SLIP") == 0) || (strcicmp(effect, "TYRESLIP") == 0) || (strcicmp(effect, "TIRESLIP") == 0))
    {
        ds->is_valid = true;
        slogt("found tyreslip effect set");
        ds->hapticsettings.effect_type = EFFECT_TYRESLIP;
    }
    if ((strcicmp(effect, "LOCK") == 0) || (strcicmp(effect, "TYRELOCK") == 0) || (strcicmp(effect, "TIRELOCK") == 0))
    {
        ds->is_valid = true;
        slogt("found tyreslock effect set");
        ds->hapticsettings.effect_type = EFFECT_TYRELOCK;
    }

    if (ds->is_valid == false)
    {
        slogw("effect %s is not a valid effect", effect);
    }

    ds->is_valid = true;
    return CARGOPIT_ERROR_NONE;
}

int strtodevsubsubtype(const char* device_subsubtype, DeviceSettings* ds)
{
    ds->dev_subsubtype = SIMDEVSUBTYPE_UNKNOWN;


    bool devfound = false;
    if (strcicmp(device_subsubtype, "CammusC5") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_CAMMUSC5;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "CammusC12") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_CAMMUSC12;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "MozaNew") == 0
        || strcicmp(device_subsubtype, "MozaR9") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_MOZA_NEW;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "MozaR5") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_MOZAR5;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "LogitechG29") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_LOGITECH_G29;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "MozaR8") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_MOZAR5;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "MozaR3") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_MOZAR5;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "MozaKSProWheel") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_MOZA_KS_PRO_WHEEL;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "CSLELITEV3PEDALS") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_CSLELITEV3PEDALS;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "SIMNETPEDALS") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_SIMNETPEDALS;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "SIMAGICP1000PEDALS") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_SIMAGICP1000PEDALS;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "SIMAGICGTNEO") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_SIMAGICGTNEO;
        devfound = true;
    }
    if (strcicmp(device_subsubtype, "REVBURNER") == 0)
    {
        ds->dev_subsubtype = SIMDEVSUBTYPE_REVBURNERTACHOMETER;
        devfound = true;
    }

    if(devfound == false)
    {
        slogw("%s does not appear to be a valid device sub sub type, but attempting to continue with other devices", device_subsubtype);
        return CARGOPIT_ERROR_INVALID_DEV;
    }
    return CARGOPIT_ERROR_NONE;
}

int strtodevsubtype(const char* device_subtype, DeviceSettings* ds, int simdev)
{
    ds->is_valid = false;
    ds->dev_subtype = SIMDEVTYPE_UNKNOWN;

    switch (simdev) {
        case SIMDEV_USB:
            if (strcicmp(device_subtype, "Tachometer") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_TACHOMETER;
                break;
            }
            if (strcicmp(device_subtype, "Wheel") == 0 || strcicmp(device_subtype, "UsbWheel") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_USBWHEEL;
                break;
            }
            if (strcicmp(device_subtype, "UsbHaptic") == 0 || strcicmp(device_subtype, "Haptic") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_USBHAPTIC;
                break;
            }
        case SIMDEV_SERIAL:
            if (strcicmp(device_subtype, "ShiftLights") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_SHIFTLIGHTS;
                break;
            }
            if (strcicmp(device_subtype, "Simleds") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_SIMLED;
                break;
            }
            if (strcicmp(device_subtype, "ArduinoCustom") == 0 || strcicmp(device_subtype, "Custom") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_ARDUINOCUSTOM;
                break;
            }
            if (strcicmp(device_subtype, "SimWind") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_SIMWIND;
                break;
            }
            if (strcicmp(device_subtype, "SerialHaptic") == 0 || strcicmp(device_subtype, "Haptic") == 0)
            {
                slogt("found serial haptic device settings");
                ds->dev_subtype = SIMDEVTYPE_SERIALHAPTIC;
                break;
            }
            if (strcicmp(device_subtype, "Wheel") == 0)
            {
                ds->dev_subtype = SIMDEVTYPE_SERIALWHEEL;
                break;
            }
        case SIMDEV_SOUND:
            ds->is_valid = true;
            break;
        default:
            ds->is_valid = false;
            slogw("%s does not appear to be a valid device sub type, but attempting to continue with other devices", device_subtype);
            return CARGOPIT_ERROR_INVALID_DEV;
    }
    ds->is_valid = true;
    return CARGOPIT_ERROR_NONE;
}

int strtodev(const char* device_type, const char* device_subtype, DeviceSettings* ds)
{
    ds->is_valid = false;
    if (strcicmp(device_type, "USB") == 0)
    {
        ds->dev_type = SIMDEV_USB;
        strtodevsubtype(device_subtype, ds, SIMDEV_USB);
    }
    else
        if (strcicmp(device_type, "Sound") == 0)
        {
            ds->dev_type = SIMDEV_SOUND;
            strtodevsubtype(device_subtype, ds, SIMDEV_SOUND);
        }
        else
            if (strcicmp(device_type, "Serial") == 0)
            {
                ds->dev_type = SIMDEV_SERIAL;
                strtodevsubtype(device_subtype, ds, SIMDEV_SERIAL);
            }
            else
            {
                ds->is_valid = false;
                slogi("%s does not appear to be a valid device type, but attempting to continue with other devices", device_type);
                return CARGOPIT_ERROR_INVALID_DEV;
            }
    ds->is_valid = true;
    return CARGOPIT_ERROR_NONE;
}

int getsimfromconfig(config_setting_t* c)
{
    int sim = 0;
    const char* simstr = NULL;
    int found = config_setting_lookup_string(c, "sim", &simstr);
    if(found == 0)
    {
        int found = config_setting_lookup_int(c, "sim", &sim);
    }
    else
    {
        sim = simapi_strtogame(simstr);
    }
    return sim;
}

int getNumberOfConfigs(const char* config_file_str)
{
    config_t cfg;
    config_init(&cfg);
    if (!config_read_file(&cfg, config_file_str))
    {
        fprintf(stderr, "%s:%d - %s\n", config_error_file(&cfg), config_error_line(&cfg), config_error_text(&cfg));
        return -1;
    }
    config_setting_t* config = NULL;
    config_setting_t* config_widgets = NULL;
    config = config_lookup(&cfg, "configs");
    int configs = config_setting_length(config);

    config_destroy(&cfg);
    return configs;
}

int getconfigtouse2(const char* config_file_str, char* car, int sim)
{
    slogt("inside first pass");
    config_t cfg;
    config_init(&cfg);
    if (!config_read_file(&cfg, config_file_str))
    {
        sloge("config read error on pass 1");
        fprintf(stderr, "%s:%d - %s\n", config_error_file(&cfg), config_error_line(&cfg), config_error_text(&cfg));
        config_destroy(&cfg);
        return -1;
    }

    slogt("config validates");
    config_setting_t* config = NULL;
    config_setting_t* config_widgets = NULL;
    config = config_lookup(&cfg, "configs");
    int configs = config_setting_length(config);

    const char* temp;
    config_setting_t* config_config = NULL;
    int j = 0;
    if ( configs == 1 )
    {
        config_destroy(&cfg);
        return -1;
    }
    int confignum = -1;
    slogt("Multiple configs found");
    for (j = 0; j < configs; j++)
    {
        config_config = config_setting_get_elem(config, j);

        int found = 0;
        int csim = 0;
        slogt("sim is %i", sim);
        csim = getsimfromconfig(config_config);
        if (csim != sim)
        {
            slogt("rejected config %i", j);
            continue;
        }

        slogt("checking if car is matched %i", j);
        temp = NULL;
        found = config_setting_lookup_string(config_config, "car", &temp);
        slogt("config car is %s found is %i", temp, found);
        if(temp != NULL && found > 0 && car > 0 && car != NULL)
        {
            slogt("checking against sim car of %s", car);
            if(strcicmp(temp, car) == 0)
            {
                confignum = j;
            }
        }
        if(confignum>=0)
        {
            break;
        }
    }

    config_destroy(&cfg);
    return confignum;
}

int getconfigtouse1(const char* config_file_str, char* car, int sim)
{
    config_t cfg;
    config_init(&cfg);
    if (!config_read_file(&cfg, config_file_str))
    {
        fprintf(stderr, "%s:%d - %s\n", config_error_file(&cfg), config_error_line(&cfg), config_error_text(&cfg));
        config_destroy(&cfg);
        return -1;
    }
    config_setting_t* config = NULL;
    config_setting_t* config_widgets = NULL;
    config = config_lookup(&cfg, "configs");
    int configs = config_setting_length(config);

    const char* temp;
    config_setting_t* config_config = NULL;
    int j = 0;
    if ( configs == 1 )
    {
        config_destroy(&cfg);
        return -1;
    }
    int confignum = -1;
    slogt("Multiple configs found");
    for (j = 0; j < configs; j++)
    {
        config_config = config_setting_get_elem(config, j);

        int found = 0;
        int csim = 0;
        slogt("sim is %i", sim);
        csim = getsimfromconfig(config_config);
        if (csim != sim)
        {
            slogt("rejected config %i", j);
            continue;
        }

        slogt("checking if car is matched %i", j);
        temp = NULL;
        found = config_setting_lookup_string(config_config, "car", &temp);
        slogt("config car is %s found is %i", temp, found);
        if(temp != NULL && found > 0 && car > 0 && car != NULL)
        {
            slogt("checking against sim car of %s", car);
            if(strcicmp(temp, car) == 0)
            {
                confignum = j;
            }
            if(strcicmp("default", temp) == 0)
            {
                slogt("matched default car");
                confignum = j;
            }
        }
        else
        {
            slogt("assuming default car");
            confignum = j;
        }
        slogt("bomb");
        if(confignum>=0)
        {
            break;
        }
    }

    config_destroy(&cfg);
    return confignum;
}

int getconfigtouse(const char* config_file_str, char* car, int sim)
{
    config_t cfg;
    config_init(&cfg);
    if (!config_read_file(&cfg, config_file_str))
    {
        fprintf(stderr, "%s:%d - %s\n", config_error_file(&cfg), config_error_line(&cfg), config_error_text(&cfg));
        config_destroy(&cfg);
        return -1;
    }

    config_setting_t* config = NULL;
    config_setting_t* config_widgets = NULL;
    config = config_lookup(&cfg, "configs");
    int configs = config_setting_length(config);

    const char* temp;
    config_setting_t* config_config = NULL;
    int j = 0;
    if ( configs == 1 )
    {
        config_destroy(&cfg);
        return 0;
    }
    int confignum = 0;
    slogt("Multiple configs found");
    for (j = 0; j < configs; j++)
    {
        config_config = config_setting_get_elem(config, j);

        int found = 0;
        int csim = 0;
        slogt("sim is %i", sim);
        csim = getsimfromconfig(config_config);
        if (csim != sim && csim != 0)
        {
            slogt("rejected config %i", j);
            continue;
        }

        slogt("checking if car is matched %i", j);
        temp = NULL;
        found = config_setting_lookup_string(config_config, "car", &temp);
        slogt("config car is %s found is %i", temp, found);
        if(temp != NULL && found > 0 && car > 0 && car != NULL)
        {
            slogt("checking against sim car of %s", car);
            if(strcicmp(temp, car) == 0)
            {
                confignum = j;
            }
            if(strcicmp("default", temp) == 0)
            {
                slogt("matched default car");
                confignum = j;
            }
        }
        else
        {
            slogt("assuming default car");
            confignum = j;
        }
        slogt("bomb");
        if(confignum<configs-1)
        {
            break;
        }
    }

    config_destroy(&cfg);
    return confignum;
}


int loadtachconfig(char* config_file, DeviceSettings* ds)
{


    xmlNode* rootnode = NULL;
    xmlNode* curnode = NULL;
    xmlNode* cursubnode = NULL;
    xmlNode* cursubsubnode = NULL;
    xmlNode* cursubsubsubnode = NULL;
    xmlDoc* doc = NULL;
    char* buf;

    doc = xmlParseFile(config_file);
    if (doc == NULL)
    {
        sloge("Could not read revburner xml config file %s", config_file);
        return 1;
    }

    rootnode = xmlDocGetRootElement(doc);
    if (rootnode == NULL)
    {
        xmlFreeDoc(doc);
        xmlCleanupParser();
        sloge("Invalid rev burner xml");
        return 1;
    }

    int arraysize = 0;
    for (curnode = rootnode; curnode; curnode = curnode->next)
    {
        for (cursubnode = curnode->children; cursubnode; cursubnode = cursubnode->next)
        {
            for (cursubsubnode = cursubnode->children; cursubsubnode; cursubsubnode = cursubsubnode->next)
            {
                if (cursubsubnode->type == XML_ELEMENT_NODE)
                {
                    slogt("Xml Element name %s", cursubsubnode->name);
                }
                if (strcicmp(cursubsubnode->name, "SettingsItem") == 0)
                {
                    arraysize++;
                }

            }
        }
    }

    uint32_t pulses_array[arraysize];
    uint32_t rpms_array[arraysize];
    slogt("rev burner settings array size %i", arraysize);
    int i = 0;
    for (curnode = rootnode; curnode; curnode = curnode->next)
    {
        if (curnode->type == XML_ELEMENT_NODE)
            for (cursubnode = curnode->children; cursubnode; cursubnode = cursubnode->next)
            {
                for (cursubsubnode = cursubnode->children; cursubsubnode; cursubsubnode = cursubsubnode->next)
                {
                    for (cursubsubsubnode = cursubsubnode->children; cursubsubsubnode; cursubsubsubnode = cursubsubsubnode->next)
                    {
                        if (strcicmp(cursubsubsubnode->name, "Value") == 0)
                        {
                            xmlChar* a = xmlNodeGetContent(cursubsubsubnode);
                            rpms_array[i] = strtol((char*) a, &buf, 10);
                            xmlFree(a);
                        }
                        if (strcicmp(cursubsubsubnode->name, "TimeValue") == 0)
                        {
                            xmlChar* a = xmlNodeGetContent(cursubsubsubnode);
                            pulses_array[i] = strtol((char*) a, &buf, 10);
                            xmlFree(a);
                            i++;
                        }
                    }
                }
            }
    }

    ds->tachsettings.pulses_array = malloc(sizeof(pulses_array));
    ds->tachsettings.rpms_array = malloc(sizeof(rpms_array));
    ds->tachsettings.size = arraysize;

    memcpy(ds->tachsettings.pulses_array, pulses_array, sizeof(pulses_array));
    memcpy(ds->tachsettings.rpms_array, rpms_array, sizeof(rpms_array));


    xmlFreeDoc(doc);
    xmlCleanupParser();

    return 0;
}

int gettyre(config_setting_t* device_settings, DeviceSettings* ds) {

    const char* temp;
    int found = config_setting_lookup_string(device_settings, "tyre", &temp);

    ds->hapticsettings.tyre = ALLFOUR;

    if (strcicmp(temp, "FRONTS") == 0)
    {
        ds->hapticsettings.tyre = FRONTS;
    }
    if (strcicmp(temp, "REARS") == 0)
    {
        ds->hapticsettings.tyre = REARS;
    }
    if (strcicmp(temp, "FRONTLEFT") == 0)
    {
        ds->hapticsettings.tyre = FRONTLEFT;
    }
    if (strcicmp(temp, "FRONTRIGHT") == 0)
    {
        ds->hapticsettings.tyre = FRONTRIGHT;
    }
    if (strcicmp(temp, "REARLEFT") == 0)
    {
        ds->hapticsettings.tyre = REARLEFT;
    }
    if (strcicmp(temp, "REARRIGHT") == 0)
    {
        ds->hapticsettings.tyre = REARRIGHT;
    }

}

static int load_device_specific_config(const char* config_file, DeviceSettings* ds)
{

    ds->has_config = false;
    if(config_file == NULL)
    {
        slogt("config set to none");
    }
    else
    {
        ds->has_config = true;

        if(strcicmp(config_file, "none") == 0)
        {
            ds->has_config = false;
            ds->specific_config_file = NULL;
        }
        else
        {
            ds->specific_config_file = strdup(config_file);
            ds->specific_config_file = expand_tilde(ds->specific_config_file);
            slogt("will try to load config file at %s", ds->specific_config_file);
        }
    }

    // in the case of the revburner tachometer, we can parse once and store
    if (ds->dev_subtype == SIMDEVTYPE_TACHOMETER)
    {
        if(ds->has_config == false)
        {
            slogw("Tachometer must have a device specific config file!");
            return 1;
        }
        loadtachconfig(ds->specific_config_file, ds);
    }
    return 0;
}

int configcheck(const char* config_file_str, int confignum, int* devices)
{
    slogt("ui config check");
    config_t cfg;
    config_init(&cfg);
    if (!config_read_file(&cfg, config_file_str))
    {
        fprintf(stderr, "%s:%d - %s\n", config_error_file(&cfg), config_error_line(&cfg), config_error_text(&cfg));
    }

    config_setting_t* config = NULL;
    config = config_lookup(&cfg, "configs");
    config_setting_t* selectedconfig = config_setting_get_elem(config, confignum);
    slogt("selected num %i", confignum);
    config_setting_t* config_devices = NULL;
    config_devices = config_setting_lookup(selectedconfig, "devices");
    *devices = config_setting_length(config_devices);
    config_destroy(&cfg);
    return 0;
    //return cfg;
}
#define DEVICE_CONFIG_ENABLED_DEFAULT 1

static void device_settings_read_enabled(const config_setting_t* device_settings, DeviceSettings* ds)
{
    ds->enabled = true;
    if (device_settings == NULL)
    {
        return;
    }

    int enabled = DEVICE_CONFIG_ENABLED_DEFAULT;
    if (config_setting_lookup_bool(device_settings, "enabled", &enabled) != CONFIG_TRUE)
    {
        return;
    }
    ds->enabled = (enabled != 0);
}

static int config_get_device(const config_setting_t *entry, DeviceSettings *ds)
{
    const char *value = NULL;

    if (config_setting_lookup_string(entry, "devid", &value))
    {
        ds->dev = strdup(value);
        return ds->dev != NULL;
    }

    if (config_setting_lookup_string(entry, "devpath", &value))
    {
        ds->dev = strdup(value);
        return ds->dev != NULL;
    }

    ds->dev = NULL;
    return 0;
}

int devsetup(const char* device_type, const char* device_subtype, const char* config_file, CargopitSettings* ms, DeviceSettings* ds, config_setting_t* device_settings)
{
    int error = CARGOPIT_ERROR_NONE;
    //slogt("Called device setup with %s %s %s", device_type, device_subtype, config_file);
    ds->dev_type = SIMDEV_UNKNOWN;

    error = strtodev(device_type, device_subtype, ds);

    if (error != CARGOPIT_ERROR_NONE)
    {
        return error;
    }

    if (ms->program_action == A_PLAY || ms->program_action == A_TEST)
    {
        error = load_device_specific_config(config_file, ds);
    }
    if (error != CARGOPIT_ERROR_NONE)
    {
        return error;
    }


    ds->fps = 60;
    config_setting_lookup_int(device_settings, "fps", &ds->fps);
    config_get_device(device_settings, ds);
    device_settings_read_enabled(device_settings, ds);

    if (ds->dev_subtype == SIMDEVTYPE_TACHOMETER)
    {
        if (device_settings != NULL)
        {
            config_setting_lookup_int(device_settings, "granularity", &ds->tachsettings.granularity);
            if (ds->tachsettings.granularity < 0 || ds->tachsettings.granularity > 4 || ds->tachsettings.granularity == 3)
            {
                slogd("No or invalid valid set for tachometer granularity, setting to 1");
                ds->tachsettings.granularity = 1;
            }
            slogi("Tachometer granularity set to %i", ds->tachsettings.granularity);
        }
        ds->tachsettings.use_pulses = true;
        if (ms->program_action == A_PLAY || ms->program_action == A_TEST)
        {
            ds->tachsettings.use_pulses = false;
        }
    }

    if (ds->dev_type == SIMDEV_USB)
    {
        if (device_settings != NULL)
        {
            const char* temp;
            int found = config_setting_lookup_string(device_settings, "subtype", &temp);
            if(temp != NULL && found > 0)
            {
              strtodevsubsubtype(temp, ds);
            }
        }
    }

    if (ds->dev_type == SIMDEV_SERIAL)
    {
        if (device_settings != NULL)
        {
            const char* temp = NULL;
            int found = config_setting_lookup_string(device_settings, "subtype", &temp);
            if(temp != NULL && found > 0)
            {
                strtodevsubsubtype(temp, ds);
            }

            int motorposition = 8;
            config_setting_lookup_int(device_settings, "motors", &motorposition);
            ds->serialdevsettings.motorsposition = motorposition;

            int numlights = 6;
            config_setting_lookup_int(device_settings, "numlights", &numlights);
            ds->serialdevsettings.numlights = numlights;

            int numleds = 6;
            config_setting_lookup_int(device_settings, "numleds", &numleds);
            ds->serialdevsettings.numleds = numleds;

            int startled = 1;
            config_setting_lookup_int(device_settings, "startled", &startled);
            ds->serialdevsettings.startled = startled;

            int endled = 1;
            config_setting_lookup_int(device_settings, "endled", &endled);
            ds->serialdevsettings.endled = endled;

            int baud = 9600;
            config_setting_lookup_int(device_settings, "baud", &baud);
            ds->serialdevsettings.baud = baud;

            double ampfactor = 1.0;
            ds->serialdevsettings.ampfactor = 1.0;
            found = config_setting_lookup_float(device_settings, "ampfactor", &ampfactor);
            ds->serialdevsettings.ampfactor = ampfactor;

            double fanpower = 0.6;
            config_setting_lookup_float(device_settings, "fanpower", &fanpower);
            ds->serialdevsettings.fanpower = fanpower;

            slogt("set port baud rate to %i, ampfactor %f, fanpower %f", baud, ampfactor, fanpower);

        }

    }

    ds->has_haptic_effects = false;
    if (ds->dev_subtype == SIMDEVTYPE_USBHAPTIC || ds->dev_subtype == SIMDEVTYPE_USBWHEEL || ds->dev_type == SIMDEV_SOUND || ds->dev_subtype == SIMDEVTYPE_SERIALHAPTIC)
    {
        slogt("analysing haptic effect settings");
        ds->has_haptic_effects = true;
        const char* effect;
        config_setting_lookup_string(device_settings, "effect", &effect);
        strtoeffecttype(effect, ds);
        if (ds->hapticsettings.effect_type == EFFECT_TYRESLIP || ds->hapticsettings.effect_type == EFFECT_TYRELOCK || ds->hapticsettings.effect_type == EFFECT_ABSBRAKES || ds->hapticsettings.effect_type == EFFECT_SUSPENSION )
        {
            gettyre(device_settings, ds);
            ds->hapticsettings.threshold = 0;
            int found = config_setting_lookup_float(device_settings, "threshold", &ds->hapticsettings.threshold);
        }

        slogi("reading configured haptic effect settings");
        ds->hapticsettings.frequency = 0;
        ds->hapticsettings.frequencyMax = 0;
        ds->hapticsettings.amplitude = HAPTIC_AMPLITUDE_UNITY;
        ds->hapticsettings.amplitudeMax = HAPTIC_AMPLITUDE_UNITY;
        if (ds->hapticsettings.effect_type == EFFECT_GEARSHIFT)
        {
            ds->hapticsettings.duration = .125;
        }
        if (device_settings != NULL)
        {
            config_setting_lookup_int(device_settings, "frequency", &ds->hapticsettings.frequency);
            config_setting_lookup_int(device_settings, "frequencyMax", &ds->hapticsettings.frequencyMax);
            config_setting_lookup_int(device_settings, "amplitude", &ds->hapticsettings.amplitude);
            config_setting_lookup_float(device_settings, "duration", &ds->hapticsettings.duration);
            config_setting_lookup_int(device_settings, "amplitudeMax", &ds->hapticsettings.amplitudeMax);

            const char* temp = NULL;
            int found = 0;
            found = config_setting_lookup_string(device_settings, "modulation", &temp);
            ds->hapticsettings.modulation = EFFECT_MODULATION_NONE;
            if (found == 0)
            {
                ds->hapticsettings.modulation = EFFECT_MODULATION_NONE;
                slogd("Effect modulation not found, set to none");
            }
            else
            {
                if(strcicmp(temp, "FREQUENCY") == 0)
                {
                    ds->hapticsettings.modulation = EFFECT_MODULATION_FREQUENCY;
                    if(ds->hapticsettings.frequencyMax == 0 || ds->hapticsettings.frequencyMax < ds->hapticsettings.frequency)
                    {
                        ds->hapticsettings.modulation = EFFECT_MODULATION_NONE;
                        slogw("Falling back to no frequency modulation since frequencyMax is either not set or set below target frequency");
                    }
                    else
                    {
                        slogi("Effect modulation found, set to FREQUENCY");
                    }
                }
                else if(strcicmp(temp, "AMPLIFY") == 0)
                {
                    ds->hapticsettings.modulation = EFFECT_MODULATION_AMPLIFY;
                    slogi("Effect modulation found, set to AMPLIFY");
                }
                else
                {
                    slogw("%s is not a valid modulation type, falling back to no effect modulation");
                    ds->hapticsettings.modulation = EFFECT_MODULATION_NONE;
                }
            }

            ds->hapticsettings.motorposition = 0;
            int motorposition = 1;
            config_setting_lookup_int(device_settings, "motors", &motorposition);
            ds->hapticsettings.motorposition = motorposition;
        }

        if (ds->dev_type == SIMDEV_SOUND)
        {
            slogi("reading configured sound device settings");
            ds->hapticsettings.amplitude = HAPTIC_AMPLITUDE_UNITY;
            ds->hapticsettings.amplitudeMax = HAPTIC_AMPLITUDE_UNITY;
            ds->sounddevsettings.volume = SOUND_STREAM_VOLUME_UNITY;
            ds->sounddevsettings.channelmask = sound_channel_mask_all(SOUND_CHANNEL_COUNT_MIN);
            ds->sounddevsettings.channels = SOUND_CHANNEL_COUNT_MIN;
            ds->sounddevsettings.noise = 0;
            if (device_settings != NULL)
            {
                int pan = 0;
                int channelmask = 0;
                int have_pan;
                int have_mask;

                int channels = SOUND_CHANNEL_COUNT_MIN;
                int noise = 0;

                ds->sounddevsettings.volume = lookup_pipewire_stream_volume(device_settings);
                have_pan = config_setting_lookup_int(device_settings, "pan", &pan);
                have_mask = config_setting_lookup_int(device_settings, "channelMask", &channelmask);
                config_setting_lookup_int(device_settings, "channels", &channels);
                config_setting_lookup_int(device_settings, "noise", &noise);
                if (channels < SOUND_CHANNEL_COUNT_MIN)
                {
                    channels = SOUND_CHANNEL_COUNT_MIN;
                }
                if (channels > SOUND_CHANNEL_COUNT_MAX)
                {
                    channels = SOUND_CHANNEL_COUNT_MAX;
                }
                ds->sounddevsettings.channels = (uint32_t)channels;
                ds->sounddevsettings.noise = (uint32_t)noise;
                ds->sounddevsettings.channelmask = sound_resolve_channel_mask(
                    have_pan == CONFIG_TRUE,
                    pan,
                    have_mask == CONFIG_TRUE,
                    channelmask,
                    channels);

                const char* temp = NULL;
                int found = 0;
                found = config_setting_lookup_string(device_settings, "devid", &temp);
                if (found == CONFIG_FALSE)
                {
                    ds->dev = NULL;
                }
                else
                {
                    if(temp != NULL)
                    {
                        ds->dev = strdup(temp);
                    }
                }


            }

        }

    }


    return error;
}

int load_device_configs(const char* config_file_str, int confignum, int configureddevices, CargopitSettings* ms, DeviceSettings* ds)
{
    int numdevices = 0;
    config_t cfg;
    config_init(&cfg);
    if (!config_read_file(&cfg, config_file_str))
    {
        fprintf(stderr, "%s:%d - %s\n", config_error_file(&cfg), config_error_line(&cfg), config_error_text(&cfg));
    }
    else
    {
        slogi("Parsing config file");

        config_setting_t* config = NULL;
        config = config_lookup(&cfg, "configs");
        config_setting_t* selectedconfig = config_setting_get_elem(config, confignum);
        config_setting_t* config_devices = NULL;
        config_devices = config_setting_lookup(selectedconfig, "devices");

        int i = 0;

        int error = CARGOPIT_ERROR_NONE;
        while (i<configureddevices)
        {
            error = CARGOPIT_ERROR_NONE;
            DeviceSettings settings;

            config_setting_t* config_device = config_setting_get_elem(config_devices, i);
            const char* device_type = NULL;
            const char* device_subtype = NULL;
            const char* device_config_file = NULL;
            int found = 0;
            config_setting_lookup_string(config_device, "device", &device_type);
            config_setting_lookup_string(config_device, "type", &device_subtype);
            found = config_setting_lookup_string(config_device, "config", &device_config_file);

            slogt("device type: %s", device_type);
            slogt("device sub type: %s", device_subtype);
            if(found == CONFIG_FALSE)
            {
                device_config_file = NULL;
            }
            else
            {
                slogt("device config file: %s", device_config_file);
            }
            if (error == CARGOPIT_ERROR_NONE)
            {
                error = devsetup(device_type, device_subtype, device_config_file, ms, &settings, config_device);
            }
            if (error == CARGOPIT_ERROR_NONE)
            {
                numdevices++;
            }
            ds[i] = settings;

            i++;

        }
    }


    config_destroy(&cfg);

    return numdevices;
}

int getsingledevice(const char* config_file_str, int confignum, int devicenum, CargopitSettings* ms, DeviceSettings* ds)
{
    int numdevices = 0;
    config_t cfg;
    config_init(&cfg);
    if (!config_read_file(&cfg, config_file_str))
    {
        fprintf(stderr, "%s:%d - %s\n", config_error_file(&cfg), config_error_line(&cfg), config_error_text(&cfg));
    }
    else
    {
        slogi("Parsing config file");

        config_setting_t* config = NULL;
        config = config_lookup(&cfg, "configs");
        config_setting_t* selectedconfig = config_setting_get_elem(config, confignum);
        config_setting_t* config_devices = NULL;
        config_devices = config_setting_lookup(selectedconfig, "devices");
        int num_devices = config_setting_length(config_devices);

        int i = 0;

        int error = CARGOPIT_ERROR_NONE;
        while (i<num_devices)
        {
            if(i!=devicenum)
            {
                i++;
                continue;
            }
            error = CARGOPIT_ERROR_NONE;

            config_setting_t* config_device = config_setting_get_elem(config_devices, i);
            const char* device_type = NULL;
            const char* device_subtype = NULL;
            const char* device_config_file = NULL;
            int found = 0;
            config_setting_lookup_string(config_device, "device", &device_type);
            config_setting_lookup_string(config_device, "type", &device_subtype);
            found = config_setting_lookup_string(config_device, "config", &device_config_file);

            slogt("device type: %s", device_type);
            slogt("device sub type: %s", device_subtype);
            if(found == CONFIG_FALSE)
            {
                device_config_file = NULL;
            }
            else
            {
                slogt("device config file: %s", device_config_file);
            }
            if (error == CARGOPIT_ERROR_NONE)
            {
                error = devsetup(device_type, device_subtype, device_config_file, ms, ds, config_device);
            }
            if (error == CARGOPIT_ERROR_NONE)
            {
                numdevices++;
            }


            i++;

        }
    }


    config_destroy(&cfg);
    return numdevices;
}

int resolve_config_index(const char* config_file_str, int requested_index)
{
    int configs = getNumberOfConfigs(config_file_str);
    if (configs <= 0)
    {
        sloge("no device profiles in %s", config_file_str);
        return CONFIG_INDEX_UNSET;
    }
    if (requested_index < 0)
    {
        return CONFIG_INDEX_FIRST;
    }
    if (requested_index >= configs)
    {
        sloge("config-index %i is out of range (%i configs)", requested_index, configs);
        return CONFIG_INDEX_UNSET;
    }
    return requested_index;
}

static int load_single_test_device(
    const char* config_file_str,
    int confignum,
    int device_index,
    CargopitSettings* ms,
    DeviceSettings** ds,
    int* configureddevices)
{
    DeviceSettings* settings;
    int loaded;

    settings = calloc(1, sizeof(DeviceSettings));
    if (settings == NULL)
    {
        return 0;
    }
    loaded = getsingledevice(config_file_str, confignum, device_index, ms, settings);
    if (loaded <= 0)
    {
        free(settings);
        return 0;
    }
    settings->enabled = true;
    *ds = settings;
    *configureddevices = 1;
    slogi("testing device index %i", device_index);
    return loaded;
}

int load_devices_for_test(
    const char* config_file_str,
    int confignum,
    int device_index,
    CargopitSettings* ms,
    DeviceSettings** ds,
    int* configureddevices)
{
    if (config_file_str == NULL || ds == NULL || configureddevices == NULL)
    {
        return 0;
    }
    *ds = NULL;
    *configureddevices = 0;
    if (device_index >= 0)
    {
        return load_single_test_device(
            config_file_str, confignum, device_index, ms, ds, configureddevices);
    }
    configcheck(config_file_str, confignum, configureddevices);
    if (*configureddevices <= 0)
    {
        return 0;
    }
    *ds = calloc(*configureddevices, sizeof(DeviceSettings));
    if (*ds == NULL)
    {
        *configureddevices = 0;
        return 0;
    }
    return load_device_configs(config_file_str, confignum, *configureddevices, ms, *ds);
}

static const char *haptic_effect_type_to_string(VibrationEffectType effect)
{
    switch (effect)
    {
        case EFFECT_ENGINERPM:
            return "EngineRPM";

        case EFFECT_GEARSHIFT:
            return "GearShift";

        case EFFECT_ABSBRAKES:
            return "ABS";

        case EFFECT_TYRESLIP:
            return "TyreSlip";

        case EFFECT_TYRELOCK:
            return "TyreLock";

        case EFFECT_SUSPENSION:
            return "Suspension";

        default:
            return NULL;
    }
}

static const char *tyre_identifier_to_string(CargopitTyreIdentifier tyre)
{
    switch (tyre)
    {
        case FRONTLEFT:
            return "FrontLeft";

        case FRONTRIGHT:
            return "FrontRight";

        case REARLEFT:
            return "RearLeft";

        case REARRIGHT:
            return "RearRight";

        case FRONTS:
            return "Front";

        case REARS:
            return "Rear";

        case ALLFOUR:
            return "ALL";

        default:
            return NULL;
    }
}

static const char *modulation_type_to_string(EffectModulationType modulation)
{
    switch (modulation)
    {
        case EFFECT_MODULATION_NONE:
            return "none";

        case EFFECT_MODULATION_FREQUENCY:
            return "frequency";

        case EFFECT_MODULATION_AMPLIFY:
            return "amplify";

        default:
            return NULL;
    }
}


static int set_string(config_setting_t *parent, const char *name, const char *value)
{
    config_setting_t *setting;

    if (value == NULL)
        return 1;

    setting = config_setting_lookup(parent, name);

    if (setting == NULL)
        setting = config_setting_add(parent, name, CONFIG_TYPE_STRING);

    if (setting == NULL)
        return 0;

    return config_setting_set_string(setting, value);
}

static int set_int(config_setting_t *parent, const char *name, int value)
{
    config_setting_t *setting;

    setting = config_setting_lookup(parent, name);

    if (setting == NULL)
        setting = config_setting_add(parent, name, CONFIG_TYPE_INT);

    if (setting == NULL)
        return 0;

    return config_setting_set_int(setting, value);
}

static int set_bool(config_setting_t *parent, const char *name, int value)
{
    config_setting_t *setting;

    setting = config_setting_lookup(parent, name);

    if (setting == NULL)
        setting = config_setting_add(parent, name, CONFIG_TYPE_BOOL);

    if (setting == NULL)
        return 0;

    return config_setting_set_bool(setting, value);
}

int set_float(config_setting_t *parent, const char *name, double value)
{
    config_setting_t *setting;

    setting = config_setting_lookup(parent, name);

    if (setting == NULL)
        setting = config_setting_add(parent, name, CONFIG_TYPE_FLOAT);

    if (setting == NULL)
        return 0;

    return config_setting_set_float(setting, value);
}

int delete_device_config(config_t *cfg, const char *configfile, int confignum, int devicenum)
{
    config_setting_t *configs;
    config_setting_t *config;
    config_setting_t *devices;

    configs = config_lookup(cfg, "configs");
    if (configs == NULL)
        return -1;

    config = config_setting_get_elem(configs, confignum);
    if (config == NULL)
        return -1;

    devices = config_setting_lookup(config, "devices");
    if (devices == NULL)
        return -1;

    if (devicenum < 0 || devicenum >= config_setting_length(devices))
        return -1;

    if (!config_setting_remove_elem(devices, devicenum))
        return -1;

    if (!config_write_file(cfg, configfile))
        return -1;

    return 0;
}

static const char *device_type_to_string(DeviceType device_type)
{
    switch (device_type)
    {
        case SIMDEV_USB:
            return "USB";
        case SIMDEV_SOUND:
            return "Sound";
        case SIMDEV_SERIAL:
            return "Serial";
        case SIMDEV_UNKNOWN:
            return "Unknown";
        default:
            return "Unknown";
    }
}

static const char *device_subtype_to_string(DeviceSubType device_subtype)
{
    switch (device_subtype)
    {
        case SIMDEVTYPE_SOUNDHAPTIC:
            return "SoundHaptic";
        case SIMDEVTYPE_TACHOMETER:
            return "Tachometer";
        case SIMDEVTYPE_USBHAPTIC:
            return "UsbHaptic";
        case SIMDEVTYPE_USBWHEEL:
            return "UsbWheel";
        case SIMDEVTYPE_SHIFTLIGHTS:
            return "ShiftLights";
        case SIMDEVTYPE_SIMWIND:
            return "SimWind";
        case SIMDEVTYPE_SERIALHAPTIC:
            return "SerialHaptic";
        case SIMDEVTYPE_SERIALWHEEL:
            return "Wheel";
        case SIMDEVTYPE_SIMLED:
            return "Simleds";
        case SIMDEVTYPE_ARDUINOCUSTOM:
            return "ArduinoCustom";
        case SIMDEVTYPE_UNKNOWN:
        default:
            return "Unknown";
    }
}

static const char *haptic_effect_to_string(VibrationEffectType effect)
{
    switch (effect)
    {
        case EFFECT_ENGINERPM:
            return "Engine";
        case EFFECT_GEARSHIFT:
            return "Gear";
        case EFFECT_ABSBRAKES:
            return "ABS";
        case EFFECT_TYRESLIP:
            return "TyreSlip";
        case EFFECT_TYRELOCK:
            return "TyreLock";
        case EFFECT_SUSPENSION:
            return "Suspension";
        default:
            return "Unknown";
    }
}

static const char *modulation_to_string(EffectModulationType modulation)
{
    switch (modulation)
    {
        case EFFECT_MODULATION_NONE:
            return "None";
        case EFFECT_MODULATION_FREQUENCY:
            return "Frequency";
        case EFFECT_MODULATION_AMPLIFY:
            return "Amplitude";
        default:
            return "Unknown";
    }
}

static const char *tyre_to_string(CargopitTyreIdentifier tyre)
{
    switch (tyre)
    {
        case FRONTLEFT:
            return "FrontLeft";
        case FRONTRIGHT:
            return "FrontRight";
        case REARLEFT:
            return "RearLeft";
        case REARRIGHT:
            return "RearRight";
        case FRONTS:
            return "Fronts";
        case REARS:
            return "Rears";
        case ALLFOUR:
            return "All";
        default:
            return "Unknown";
    }
}

int save_device_config(config_t *cfg, const char* configfile, int confignum, int devicenum, const DeviceSettings *ds)
{
    config_setting_t *configs;
    config_setting_t *config_entry;
    config_setting_t *devices;
    config_setting_t *device_entry;

    if (cfg == NULL || ds == NULL)
        return 0;

    slogd("Saving device with device num %i and confignum %i", devicenum, confignum);
    configs = config_lookup(cfg, "configs");
    if (configs == NULL)
        return 0;

    config_entry = config_setting_get_elem(configs, confignum);
    if (config_entry == NULL)
        return 0;

    devices = config_setting_lookup(config_entry, "devices");
    if (devices == NULL)
        return 0;

    device_entry = config_setting_get_elem(devices, devicenum);
    if (device_entry == NULL)
    {
        device_entry = config_setting_add(devices, NULL, CONFIG_TYPE_GROUP);

        if (device_entry == NULL)
            return 0;

        if (config_setting_index(device_entry) != devicenum)
            return 0;
    }
    if (device_entry == NULL)
        return 0;

    set_string(device_entry, "device", device_type_to_string(ds->dev_type));

    set_bool(device_entry, "enabled", ds->enabled ? 1 : 0);
    
    set_int(device_entry, "fps", ds->fps);

    if (ds->specific_config_file != NULL)
    {
        set_string(device_entry, "config", ds->specific_config_file);
    }
    if (ds->dev != NULL)
    {
        set_string(device_entry, "devid", ds->dev);
    }

    switch (ds->dev_type)
    {

        case SIMDEV_SERIAL:
        {
            set_string(device_entry, "type", device_subtype_to_string(ds->dev_type + SerialDevicesOffset));

            const SerialDeviceSettings *ss = &ds->serialdevsettings;
            
            set_int(device_entry, "baud", ss->baud);

            set_float(device_entry, "ampfactor", ss->ampfactor);

            set_float(device_entry, "fanpower", ss->fanpower);

            break;
        }

        case SIMDEV_SOUND:
        {
            set_string(device_entry, "type", device_subtype_to_string(ds->dev_type));

            const SoundDeviceSettings *ss = &ds->sounddevsettings;

            set_int(device_entry, "streamVolume", ss->volume);
            set_int(device_entry, "channelMask", (int)ss->channelmask);
            set_int(device_entry, "pan", sound_first_channel(ss->channelmask));
            set_int(device_entry, "channels", ss->channels);
            set_int(device_entry, "noise", ss->noise);

            break;
        }

        case SIMDEV_USB:
        {
            set_string(device_entry, "type", device_subtype_to_string(ds->dev_type + USBDevicesOffset));

            const USBDeviceSettings *us = &ds->usbdevsettings;

            break;
        }

        default:
            break;
    }

    if(ds->has_haptic_effects == true)
    {
        slogt("Saving haptic effect settings");
        const HapticEffectSettings *hs = &ds->hapticsettings;

        set_int(device_entry, "frequency", hs->frequency);

        set_int(device_entry, "amplitude", hs->amplitude);

        set_int(device_entry, "frequencyMax", hs->frequencyMax);

        set_int(device_entry, "amplitudeMax", hs->amplitudeMax);

        set_float(device_entry, "threshold", hs->threshold);

        set_float(device_entry, "duration", hs->duration);

        set_string(device_entry, "effect", haptic_effect_to_string(hs->effect_type));
         
        set_string(device_entry, "tyre", tyre_to_string(hs->tyre));
         
        set_string(device_entry, "modulation", modulation_to_string(hs->modulation));
    }

    if(ds->has_led_effects == true)
    {
        slogt("Saving led effect settings");
        const SerialDeviceSettings *ss = &ds->serialdevsettings;

        set_int(device_entry, "numleds", ss->numleds);
        set_int(device_entry, "startled", ss->startled);
        set_int(device_entry, "endled", ss->endled);
    }

    if(!config_write_file(cfg, configfile))
    {
        return 1;
    }
    return 0;
}

config_t *open_cargopit_config(const char *filename)
{
    config_t *cfg;

    if (filename == NULL)
        return NULL;

    cfg = malloc(sizeof(*cfg));
    if (cfg == NULL)
        return NULL;

    config_init(cfg);

    if (!config_read_file(cfg, filename))
    {
        fprintf(stderr,
                "Failed to read config file '%s' (line %d): %s\n",
                filename,
                config_error_line(cfg),
                config_error_text(cfg));

        config_destroy(cfg);
        free(cfg);

        return NULL;
    }

    return cfg;
}

void close_cargopit_config(config_t* cfg)
{
    if (cfg == NULL)
        return;

    config_destroy(cfg);
}

int settingsfree(DeviceSettings ds)
{
    if (ds.dev != NULL)
    {
        free(ds.dev);
    }

    if(ds.has_config && ds.specific_config_file != NULL)
    {
        free(ds.specific_config_file);
    }

    return 0;
}

int cargopitsettingsfree(CargopitSettings* ms)
{
    if(ms->tyre_diameter_config != NULL)
    {
        free(ms->tyre_diameter_config);
        ms->tyre_diameter_config = NULL;
    }
    if(ms->config_str != NULL)
    {
        free(ms->config_str);
        ms->config_str = NULL;
    }
    if(ms->log_filename_str != NULL)
    {
        free(ms->log_filename_str);
        ms->log_filename_str = NULL;
    }
    if(ms->log_dirname_str != NULL)
    {
        free(ms->log_dirname_str);
        ms->log_dirname_str = NULL;
    }
}

uint32_t sound_channel_mask_all(int channels)
{
    int count = channels;
    if (count < SOUND_CHANNEL_COUNT_MIN)
    {
        count = SOUND_CHANNEL_COUNT_MIN;
    }
    if (count > SOUND_CHANNEL_COUNT_MAX)
    {
        count = SOUND_CHANNEL_COUNT_MAX;
    }
    return (1u << (unsigned)count) - 1u;
}

uint32_t sound_resolve_channel_mask(int have_pan, int pan, int have_mask, int mask, int channels)
{
    uint32_t all = sound_channel_mask_all(channels);
    uint32_t clipped;
    if (have_mask)
    {
        clipped = (uint32_t)mask & all;
        if (clipped != 0)
        {
            return clipped;
        }
        return all;
    }
    if (!have_pan)
    {
        return all;
    }
    if (pan == SOUND_PAN_ALL_CHANNELS || pan < 0 || pan >= channels)
    {
        return all;
    }
    return SOUND_CHANNEL_BIT(pan);
}

int sound_first_channel(uint32_t mask)
{
    int index;
    for (index = 0; index < SOUND_CHANNEL_COUNT_MAX; index++)
    {
        if ((mask & SOUND_CHANNEL_BIT(index)) != 0)
        {
            return index;
        }
    }
    return 0;
}
