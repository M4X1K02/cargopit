#ifndef _DEVICENAMES_H
#define _DEVICENAMES_H

#include <stddef.h>

/*
 * Config-file spellings for every enum cargopit reads from cargopit.config.
 * Matching is case-insensitive. The first name listed for a value is the one
 * written back to the file; later names are accepted aliases.
 * tui/tests/config_names.rs checks that every name the TUI writes is listed.
 */

#define CARGOPIT_DEVICE_CLASS_NAMES(X) \
    X("USB",            SIMDEV_USB) \
    X("Sound",          SIMDEV_SOUND) \
    X("Serial",         SIMDEV_SERIAL)

#define CARGOPIT_USB_TYPE_NAMES(X) \
    X("Tachometer",     SIMDEVTYPE_TACHOMETER) \
    X("UsbWheel",       SIMDEVTYPE_USBWHEEL) \
    X("Wheel",          SIMDEVTYPE_USBWHEEL) \
    X("UsbHaptic",      SIMDEVTYPE_USBHAPTIC) \
    X("Haptic",         SIMDEVTYPE_USBHAPTIC)

#define CARGOPIT_SERIAL_TYPE_NAMES(X) \
    X("ShiftLights",    SIMDEVTYPE_SHIFTLIGHTS) \
    X("Simleds",        SIMDEVTYPE_SIMLED) \
    X("ArduinoCustom",  SIMDEVTYPE_ARDUINOCUSTOM) \
    X("Custom",         SIMDEVTYPE_ARDUINOCUSTOM) \
    X("SimWind",        SIMDEVTYPE_SIMWIND) \
    X("SerialHaptic",   SIMDEVTYPE_SERIALHAPTIC) \
    X("Haptic",         SIMDEVTYPE_SERIALHAPTIC) \
    X("Wheel",          SIMDEVTYPE_SERIALWHEEL)

/* Sound devices accept any type; this is only the name written back. */
#define CARGOPIT_SOUND_TYPE_NAMES(X) \
    X("Haptic",         SIMDEVTYPE_SOUNDHAPTIC)

#define CARGOPIT_HARDWARE_NAMES(X) \
    X("CammusC5",           SIMDEVSUBTYPE_CAMMUSC5) \
    X("CammusC12",          SIMDEVSUBTYPE_CAMMUSC12) \
    X("MozaR5",             SIMDEVSUBTYPE_MOZAR5) \
    X("MozaR8",             SIMDEVSUBTYPE_MOZAR5) \
    X("MozaR3",             SIMDEVSUBTYPE_MOZAR5) \
    X("MozaNew",            SIMDEVSUBTYPE_MOZA_NEW) \
    X("MozaR9",             SIMDEVSUBTYPE_MOZA_NEW) \
    X("MozaKSProWheel",     SIMDEVSUBTYPE_MOZA_KS_PRO_WHEEL) \
    X("LogitechG29",        SIMDEVSUBTYPE_LOGITECH_G29) \
    X("CSLELITEV3PEDALS",   SIMDEVSUBTYPE_CSLELITEV3PEDALS) \
    X("SIMNETPEDALS",       SIMDEVSUBTYPE_SIMNETPEDALS) \
    X("SIMAGICP1000PEDALS", SIMDEVSUBTYPE_SIMAGICP1000PEDALS) \
    X("SIMAGICGTNEO",       SIMDEVSUBTYPE_SIMAGICGTNEO) \
    X("Revburner",          SIMDEVSUBTYPE_REVBURNERTACHOMETER)

#define CARGOPIT_EFFECT_NAMES(X) \
    X("Engine",         EFFECT_ENGINERPM) \
    X("Gear",           EFFECT_GEARSHIFT) \
    X("ABS",            EFFECT_ABSBRAKES) \
    X("TyreSlip",       EFFECT_TYRESLIP) \
    X("Slip",           EFFECT_TYRESLIP) \
    X("TireSlip",       EFFECT_TYRESLIP) \
    X("TyreLock",       EFFECT_TYRELOCK) \
    X("Lock",           EFFECT_TYRELOCK) \
    X("TireLock",       EFFECT_TYRELOCK) \
    X("Suspension",     EFFECT_SUSPENSION)

#define CARGOPIT_TYRE_NAMES(X) \
    X("FrontLeft",      FRONTLEFT) \
    X("FrontRight",     FRONTRIGHT) \
    X("RearLeft",       REARLEFT) \
    X("RearRight",      REARRIGHT) \
    X("Fronts",         FRONTS) \
    X("Front",          FRONTS) \
    X("Rears",          REARS) \
    X("Rear",           REARS) \
    X("All",            ALLFOUR)

#define CARGOPIT_MODULATION_NAMES(X) \
    X("None",           EFFECT_MODULATION_NONE) \
    X("Frequency",      EFFECT_MODULATION_FREQUENCY) \
    X("Amplitude",      EFFECT_MODULATION_AMPLIFY) \
    X("Amplify",        EFFECT_MODULATION_AMPLIFY)

typedef struct
{
    const char* name;
    int value;
}
CargopitName;

typedef struct
{
    const CargopitName* names;
    size_t count;
}
CargopitNameTable;

extern const CargopitNameTable CARGOPIT_DEVICE_CLASSES;
extern const CargopitNameTable CARGOPIT_USB_TYPES;
extern const CargopitNameTable CARGOPIT_SERIAL_TYPES;
extern const CargopitNameTable CARGOPIT_SOUND_TYPES;
extern const CargopitNameTable CARGOPIT_HARDWARE;
extern const CargopitNameTable CARGOPIT_EFFECTS;
extern const CargopitNameTable CARGOPIT_TYRES;
extern const CargopitNameTable CARGOPIT_MODULATIONS;

/* Returns 0 and sets *value when `name` is listed (case-insensitive), -1 otherwise. */
int cargopit_name_lookup(const CargopitNameTable* table, const char* name, int* value);
/* The name written for `value`, or NULL when the table has none. */
const char* cargopit_name_for(const CargopitNameTable* table, int value);

#endif
