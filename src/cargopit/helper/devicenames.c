#include <strings.h>

#include "confighelper.h"
#include "devicenames.h"

#define NAME_ENTRY(name, value) { name, value },
#define NAME_TABLE(table_name, entries) \
    static const CargopitName table_name##_entries[] = { entries(NAME_ENTRY) }; \
    const CargopitNameTable table_name = { table_name##_entries, sizeof(table_name##_entries) / sizeof(CargopitName) };

NAME_TABLE(CARGOPIT_DEVICE_CLASSES, CARGOPIT_DEVICE_CLASS_NAMES)
NAME_TABLE(CARGOPIT_USB_TYPES,      CARGOPIT_USB_TYPE_NAMES)
NAME_TABLE(CARGOPIT_SERIAL_TYPES,   CARGOPIT_SERIAL_TYPE_NAMES)
NAME_TABLE(CARGOPIT_SOUND_TYPES,    CARGOPIT_SOUND_TYPE_NAMES)
NAME_TABLE(CARGOPIT_HARDWARE,       CARGOPIT_HARDWARE_NAMES)
NAME_TABLE(CARGOPIT_EFFECTS,        CARGOPIT_EFFECT_NAMES)
NAME_TABLE(CARGOPIT_TYRES,          CARGOPIT_TYRE_NAMES)
NAME_TABLE(CARGOPIT_MODULATIONS,    CARGOPIT_MODULATION_NAMES)

int cargopit_name_lookup(const CargopitNameTable* table, const char* name, int* value)
{
    if (table == NULL || name == NULL || value == NULL)
    {
        return -1;
    }
    for (size_t i = 0; i < table->count; i++)
    {
        if (strcasecmp(table->names[i].name, name) == 0)
        {
            *value = table->names[i].value;
            return 0;
        }
    }
    return -1;
}

const char* cargopit_name_for(const CargopitNameTable* table, int value)
{
    if (table == NULL)
    {
        return NULL;
    }
    for (size_t i = 0; i < table->count; i++)
    {
        if (table->names[i].value == value)
        {
            return table->names[i].name;
        }
    }
    return NULL;
}
