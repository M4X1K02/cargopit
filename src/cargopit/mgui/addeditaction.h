#include <gui/gui.hxx>
#include "mainwindow.h"

Panel *adddevice_window(Window *parent, App* app, CargopitSettings* ms, enum device_action dev_action, int confignum, int devicenum);

typedef struct _add_device_t AddDeviceData;

struct _add_device_t
{
    Layout* main_outer_layout;

    Layout* top_selection_layout;
    Label* label_select_device_class;
    Label* label_select_device_subtype;
    Label* label_select_device;
    Combo* combo_select_device_class;
    Combo* combo_select_device_subtype;
    Combo* combo_select_device;

    Edit* edit_select_device;

    Layout* general_settings_layout;
    Label* label_fps;
    Edit* edit_fps;
    Button* check_use_lua;

    Layout* empty_device_sublayout;

    Panel* device_specific_panel;

    // sound sublayout
    Layout* sound_device_sublayout;
    Label* label_channel;
    Label* label_channels;
    Label* label_volume;
    Edit* edit_channel;
    Edit* edit_channels;
    Edit* edit_volume;

    // serial sublayout
    Layout* serial_device_sublayout;
    Label* label_baud;
    Edit* edit_baud;

    // led sublayout
    Layout* led_device_sublayout;
    Label* label_num_leds;
    Label* label_start_led;
    Label* label_end_led;
    Edit* edit_num_leds;
    Edit* edit_start_led;
    Edit* edit_end_led;

    Layout* haptic_effect_sublayout;
    Label* label_select_effect;
    Combo* combo_select_effect;
    Label* label_select_modulation;
    Combo* combo_select_modulation;
    Label* label_select_tyre;
    Combo* combo_select_tyre;

    Label* label_motorposition;
    Edit* edit_motorposition;
    Label* label_frequency;
    Edit* edit_frequency;
    Label* label_amplitude;
    Edit* edit_amplitude;

    Label* label_frequencyMax;
    Edit* edit_frequencyMax;
    Label* label_amplitudeMax;
    Edit* edit_amplitudeMax;
    Label* label_threshold;
    Edit* edit_threshold;
    Label* label_duration;
    Edit* edit_duration;



    Layout* add_save_test_layout;
    Button* test_button;
    Button* save_button;

    Cell* cell_top_selection_layout;
    Cell* cell_led_device_layout;
    Cell* cell_haptic_effect_layout;

    uint32_t type;
    Label *label;
    CargopitSettings* ms;
    DeviceSettings* ds;
    Panel* panel;
    App* apppointer;
    int confignum;
    int devicenum;
    Window *parent;
};
