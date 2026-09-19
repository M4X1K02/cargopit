#include <gui/gui.hxx>

#include "mainwindow.h"

Panel *test_window(Window *parent, App* app);


struct _test_device_t
{
    Layout* main_outer_layout;

    Layout* data_edit_layout;
    Layout* add_save_test_layout;
    
    Label* label_gear;
    Label* label_velocity;
    Label* label_maxrpm;
    Label* label_rpm;
    Edit* edit_gear;
    Edit* edit_velocity;
    Edit* edit_maxrpm;
    Edit* edit_rpm;

    Label* label_tyre_diameter_1;
    Label* label_tyre_diameter_2;
    Label* label_tyre_diameter_3;
    Label* label_tyre_diameter_4;
    Label* label_tyre_rps_1;
    Label* label_tyre_rps_2;
    Label* label_tyre_rps_3;
    Label* label_tyre_rps_4;
    
    Edit* edit_tyre_diameter_1;
    Edit* edit_tyre_diameter_2;
    Edit* edit_tyre_diameter_3;
    Edit* edit_tyre_diameter_4;
    Edit* edit_tyre_rps_1;
    Edit* edit_tyre_rps_2;
    Edit* edit_tyre_rps_3;
    Edit* edit_tyre_rps_4;

    Button* update_button;
    Button* done_button;
};
