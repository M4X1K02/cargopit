#include <dirent.h>
#include <stdio.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <ctype.h>
#include <limits.h>

#include <libxml/parser.h>
#include <libxml/xmlreader.h>
#include <libxml/tree.h>

#include "confighelper.h"
#include "dirhelper.h"

#include "../slog/slog.h"
#include "parameters.h"



#include "../simulatorapi/simapi/simapi/simmapper.h"
#include "devicenames.h"

#include <pulse/pulseaudio.h>

int cargopit_clamp_fps(int fps)
{
    if (fps < CARGOPIT_FPS_MIN)
    {
        slogw("fps %i is below %i, using %i", fps, CARGOPIT_FPS_MIN, CARGOPIT_FPS_MIN);
        return CARGOPIT_FPS_MIN;
    }
    if (fps > CARGOPIT_FPS_MAX)
    {
        slogw("fps %i is above %i, using %i", fps, CARGOPIT_FPS_MAX, CARGOPIT_FPS_MAX);
        return CARGOPIT_FPS_MAX;
    }
    return fps;
}

int strcicmp(char const *a, char const *b)
{
    if (a == NULL || b == NULL)
    {
        return (a == NULL) - (b == NULL);
    }

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
    int value;

    if (effect == NULL || ds == NULL)
    {
        return CARGOPIT_ERROR_INVALID_DEV;
    }
    ds->is_valid = cargopit_name_lookup(&CARGOPIT_EFFECTS, effect, &value) == 0;
    if (ds->is_valid == false)
    {
        slogw("effect %s is not a valid effect", effect);
        return CARGOPIT_ERROR_INVALID_DEV;
    }
    ds->hapticsettings.effect_type = (VibrationEffectType) value;
    return CARGOPIT_ERROR_NONE;
}

int strtodevsubsubtype(const char* device_subsubtype, DeviceSettings* ds)
{
    int value;

    ds->dev_subsubtype = SIMDEVSUBTYPE_UNKNOWN;
    if (cargopit_name_lookup(&CARGOPIT_HARDWARE, device_subsubtype, &value) != 0)
    {
        slogw("%s does not appear to be a valid device sub sub type, but attempting to continue with other devices", device_subsubtype);
        return CARGOPIT_ERROR_INVALID_DEV;
    }
    ds->dev_subsubtype = (DeviceSubSubType) value;
    return CARGOPIT_ERROR_NONE;
}

static const CargopitNameTable* device_type_names(DeviceType dev_type)
{
    switch (dev_type)
    {
        case SIMDEV_USB:
            return &CARGOPIT_USB_TYPES;
        case SIMDEV_SERIAL:
            return &CARGOPIT_SERIAL_TYPES;
        case SIMDEV_SOUND:
            return &CARGOPIT_SOUND_TYPES;
        default:
            return NULL;
    }
}

int strtodevsubtype(const char* device_subtype, DeviceSettings* ds, int simdev)
{
    int value;

    if (device_subtype == NULL || ds == NULL)
    {
        return CARGOPIT_ERROR_INVALID_DEV;
    }
    ds->is_valid = false;
    ds->dev_subtype = SIMDEVTYPE_UNKNOWN;
    if (simdev == SIMDEV_SOUND)
    {
        ds->dev_subtype = SIMDEVTYPE_SOUNDHAPTIC;
        ds->is_valid = true;
        return CARGOPIT_ERROR_NONE;
    }
    if (cargopit_name_lookup(device_type_names((DeviceType) simdev), device_subtype, &value) != 0)
    {
        slogw("%s does not appear to be a valid device sub type, but attempting to continue with other devices", device_subtype);
        return CARGOPIT_ERROR_INVALID_DEV;
    }
    ds->dev_subtype = (DeviceSubType) value;
    ds->is_valid = true;
    return CARGOPIT_ERROR_NONE;
}

int strtodev(const char* device_type, const char* device_subtype, DeviceSettings* ds)
{
    int value;

    ds->is_valid = false;
    if (cargopit_name_lookup(&CARGOPIT_DEVICE_CLASSES, device_type, &value) != 0)
    {
        slogi("%s does not appear to be a valid device type, but attempting to continue with other devices", device_type);
        return CARGOPIT_ERROR_INVALID_DEV;
    }
    ds->dev_type = (DeviceType) value;
    return strtodevsubtype(device_subtype, ds, ds->dev_type);
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
    if (config_file == NULL || ds == NULL)
    {
        return 1;
    }

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

    size_t arraysize = 0;
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

    if (arraysize == 0 || arraysize > INT_MAX)
    {
        xmlFreeDoc(doc);
        xmlCleanupParser();
        sloge("Rev burner XML contains no settings");
        return 1;
    }

    uint32_t* pulses_array = calloc(arraysize, sizeof(*pulses_array));
    uint32_t* rpms_array = calloc(arraysize, sizeof(*rpms_array));
    if (pulses_array == NULL || rpms_array == NULL)
    {
        free(pulses_array);
        free(rpms_array);
        xmlFreeDoc(doc);
        xmlCleanupParser();
        sloge("Could not allocate rev burner settings");
        return 1;
    }

    slogt("rev burner settings array size %zu", arraysize);
    size_t i = 0;
    for (curnode = rootnode; curnode; curnode = curnode->next)
    {
        if (curnode->type == XML_ELEMENT_NODE)
            for (cursubnode = curnode->children; cursubnode; cursubnode = cursubnode->next)
            {
                for (cursubsubnode = cursubnode->children; cursubsubnode; cursubsubnode = cursubsubnode->next)
                {
                    for (cursubsubsubnode = cursubsubnode->children; cursubsubsubnode; cursubsubsubnode = cursubsubsubnode->next)
                    {
                        if (strcicmp(cursubsubsubnode->name, "Value") == 0 &&
                            i < arraysize)
                        {
                            xmlChar* a = xmlNodeGetContent(cursubsubsubnode);
                            if (a != NULL)
                            {
                                rpms_array[i] = strtol((char*) a, &buf, 10);
                            }
                            xmlFree(a);
                        }
                        if (strcicmp(cursubsubsubnode->name, "TimeValue") == 0 &&
                            i < arraysize)
                        {
                            xmlChar* a = xmlNodeGetContent(cursubsubsubnode);
                            if (a != NULL)
                            {
                                pulses_array[i] = strtol((char*) a, &buf, 10);
                            }
                            xmlFree(a);
                            i++;
                        }
                    }
                }
            }
    }

    ds->tachsettings.pulses_array = pulses_array;
    ds->tachsettings.rpms_array = rpms_array;
    ds->tachsettings.size = (int)arraysize;

    xmlFreeDoc(doc);
    xmlCleanupParser();

    return 0;
}

bool haptic_effect_uses_tyre(VibrationEffectType effect)
{
    return effect == EFFECT_TYRESLIP
        || effect == EFFECT_TYRELOCK
        || effect == EFFECT_ABSBRAKES
        || effect == EFFECT_SUSPENSION;
}

int gettyre(config_setting_t* device_settings, DeviceSettings* ds) {
    if (device_settings == NULL || ds == NULL)
    {
        return CONFIG_FALSE;
    }

    const char* temp = NULL;
    int found = config_setting_lookup_string(device_settings, "tyre", &temp);
    if (!found || temp == NULL)
    {
        return found;
    }

    int value;
    ds->hapticsettings.tyre = ALLFOUR;
    if (cargopit_name_lookup(&CARGOPIT_TYRES, temp, &value) == 0)
    {
        ds->hapticsettings.tyre = (CargopitTyreIdentifier) value;
    }
    return found;
}

static EffectModulationType parse_modulation(const char* name, const HapticEffectSettings* hs)
{
    int value;

    if (cargopit_name_lookup(&CARGOPIT_MODULATIONS, name, &value) != 0)
    {
        slogw("%s is not a valid modulation type, falling back to no effect modulation", name);
        return EFFECT_MODULATION_NONE;
    }
    if (value == EFFECT_MODULATION_FREQUENCY && (hs->frequencyMax == 0 || hs->frequencyMax < hs->frequency))
    {
        slogw("Falling back to no frequency modulation since frequencyMax is either not set or set below target frequency");
        return EFFECT_MODULATION_NONE;
    }
    slogi("Effect modulation found, set to %s", cargopit_name_for(&CARGOPIT_MODULATIONS, value));
    return (EffectModulationType) value;
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


    ds->fps = CARGOPIT_FPS_DEFAULT;
    config_setting_lookup_int(device_settings, "fps", &ds->fps);
    ds->fps = cargopit_clamp_fps(ds->fps);
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
        if (haptic_effect_uses_tyre(ds->hapticsettings.effect_type))
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
                ds->hapticsettings.modulation = parse_modulation(temp, &ds->hapticsettings);
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
            DeviceSettings settings = {0};

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

    set_string(device_entry, "device", cargopit_name_for(&CARGOPIT_DEVICE_CLASSES, ds->dev_type));
    set_string(device_entry, "type", cargopit_name_for(device_type_names(ds->dev_type), ds->dev_subtype));
    set_string(device_entry, "subtype", cargopit_name_for(&CARGOPIT_HARDWARE, ds->dev_subsubtype));

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
            const SerialDeviceSettings *ss = &ds->serialdevsettings;
            
            set_int(device_entry, "baud", ss->baud);

            set_float(device_entry, "ampfactor", ss->ampfactor);

            set_float(device_entry, "fanpower", ss->fanpower);

            break;
        }

        case SIMDEV_SOUND:
        {
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

        set_string(device_entry, "effect", cargopit_name_for(&CARGOPIT_EFFECTS, hs->effect_type));
         
        set_string(device_entry, "tyre", cargopit_name_for(&CARGOPIT_TYRES, hs->tyre));
         
        set_string(device_entry, "modulation", cargopit_name_for(&CARGOPIT_MODULATIONS, hs->modulation));
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
    if (ms == NULL)
    {
        return 0;
    }

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

    return 0;
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
