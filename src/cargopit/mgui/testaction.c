/* Listboxes */

#include "testaction.h"
#include "../gameloop/gameloop.h"
#include "mainwindow.h"
#include <gui/guiall.h>

typedef struct _modal_data_t gModalData;

struct _modal_data_t
{
    uint32_t type;
    Label *label;
    Window *parent;
};


/*---------------------------------------------------------------------------*/

static Layout *i_modal_layout(gModalData *data, Window *window, App *app);

/*---------------------------------------------------------------------------*/

static gModalData *i_modal_data(Window *parent)
{
    gModalData *data = heap_new0(gModalData);
    data->parent = parent;
    data->type = UINT32_MAX;
    return data;
}

/*---------------------------------------------------------------------------*/
static void i_OnTestDone(Window *window, Event *e)
{
    Button *button = event_sender(e, Button);
    window_stop_modal(window, button_get_tag(button));
}

/*---------------------------------------------------------------------------*/

static void i_OnTestDataUpdate(Window* window, Event* e)
{
    Button *button = event_sender(e, Button);
    //SimData* simdata = get_test_simdata();
}


/*---------------------------------------------------------------------------*/

static void i_destroy_modal_data(gModalData **data)
{
    cargopit_testloop_stop();
    heap_delete(data, gModalData);
}


/*---------------------------------------------------------------------------*/

static Layout *i_modal_layout(gModalData *data, Window* window, App *app)
{
    struct _test_device_t subwindow;
    subwindow.main_outer_layout = layout_create(1,7);
    subwindow.data_edit_layout = layout_create(8,6);
    subwindow.add_save_test_layout = layout_create(2,1);


    subwindow.edit_gear = edit_create();
    subwindow.edit_velocity = edit_create();
    subwindow.edit_maxrpm = edit_create();
    subwindow.edit_rpm = edit_create();

    subwindow.label_gear = label_create();
    subwindow.label_velocity = label_create();
    subwindow.label_maxrpm = label_create();
    subwindow.label_rpm = label_create();


    subwindow.label_tyre_diameter_1 = label_create();
    subwindow.label_tyre_diameter_2 = label_create();
    subwindow.label_tyre_diameter_3 = label_create();
    subwindow.label_tyre_diameter_4 = label_create();
    subwindow.label_tyre_rps_1 = label_create();
    subwindow.label_tyre_rps_2 = label_create();
    subwindow.label_tyre_rps_3 = label_create();
    subwindow.label_tyre_rps_4 = label_create();
    
    subwindow.edit_tyre_diameter_1 = edit_create();
    subwindow.edit_tyre_diameter_2 = edit_create();
    subwindow.edit_tyre_diameter_3 = edit_create();
    subwindow.edit_tyre_diameter_4 = edit_create();
    subwindow.edit_tyre_rps_1 = edit_create();
    subwindow.edit_tyre_rps_2 = edit_create();
    subwindow.edit_tyre_rps_3 = edit_create();
    subwindow.edit_tyre_rps_4 = edit_create();

    subwindow.update_button = button_push();
    subwindow.done_button = button_push();

    label_text(subwindow.label_gear, "gear: ");
    label_text(subwindow.label_velocity, "velocity: ");
    label_text(subwindow.label_maxrpm, "max rpm: ");
    label_text(subwindow.label_rpm, "rpm: ");


    button_text(subwindow.update_button, "Update");
    button_text(subwindow.done_button, "Done");
    button_OnClick(subwindow.done_button, listener(window, i_OnTestDone, Window));
    button_OnClick(subwindow.update_button, listener(window, i_OnTestDataUpdate, Window));

    layout_layout(subwindow.main_outer_layout, subwindow.data_edit_layout, 0, 2);
    layout_layout(subwindow.main_outer_layout, subwindow.add_save_test_layout, 0, 5);
    
    layout_label(subwindow.data_edit_layout, subwindow.label_gear, 0, 0);
    layout_label(subwindow.data_edit_layout, subwindow.label_velocity, 2, 0);
    layout_label(subwindow.data_edit_layout, subwindow.label_maxrpm, 0, 1);
    layout_label(subwindow.data_edit_layout, subwindow.label_rpm, 2, 1);

    layout_edit(subwindow.data_edit_layout, subwindow.edit_gear, 1, 0);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_velocity, 3, 0);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_maxrpm, 1, 1);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_rpm, 3, 1);
    
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_rps_1, 0, 2);
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_rps_2, 2, 2);
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_rps_3, 4, 2);
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_rps_4, 6, 2);
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_diameter_1, 0, 3);
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_diameter_2, 2, 3);
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_diameter_3, 4, 3);
    layout_label(subwindow.data_edit_layout, subwindow.label_tyre_diameter_4, 6, 3);
    label_text(subwindow.label_tyre_rps_1, "rps_lf");
    label_text(subwindow.label_tyre_rps_2, "rps_rf");
    label_text(subwindow.label_tyre_rps_3, "rps_lr");
    label_text(subwindow.label_tyre_rps_4, "rps_rr");
    label_text(subwindow.label_tyre_diameter_1, "diameter_lf");
    label_text(subwindow.label_tyre_diameter_2, "diameter_rf");
    label_text(subwindow.label_tyre_diameter_3, "diameter_lr");
    label_text(subwindow.label_tyre_diameter_4, "diameter_rr");


    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_rps_1, 1, 2);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_rps_2, 3, 2);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_rps_3, 5, 2);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_rps_4, 7, 2);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_diameter_1, 1, 3);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_diameter_2, 3, 3);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_diameter_3, 5, 3);
    layout_edit(subwindow.data_edit_layout, subwindow.edit_tyre_diameter_4, 7, 3);




    cell_dbind(layout_cell(subwindow.data_edit_layout, 1, 0), SimData, uint32_t, gear);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 3, 0), SimData, uint32_t, velocity);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 1, 1), SimData, uint32_t, maxrpm);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 3, 1), SimData, uint32_t, rpms);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 1, 2), SimData, real64_t, tyreRPS[0]);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 3, 2), SimData, real64_t, tyreRPS[1]);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 5, 2), SimData, real64_t, tyreRPS[2]);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 7, 2), SimData, real64_t, tyreRPS[3]);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 1, 3), SimData, real64_t, tyrediameter[0]);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 3, 3), SimData, real64_t, tyrediameter[1]);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 5, 3), SimData, real64_t, tyrediameter[2]);
    cell_dbind(layout_cell(subwindow.data_edit_layout, 7, 3), SimData, real64_t, tyrediameter[3]);
    

    SimData* simdata = get_test_simdata();
    layout_dbind(subwindow.data_edit_layout, NULL, SimData);
    layout_dbind_obj(subwindow.data_edit_layout, simdata, SimData);


    layout_button(subwindow.add_save_test_layout, subwindow.update_button, 0, 0);
    layout_button(subwindow.add_save_test_layout, subwindow.done_button, 1, 0);

    return subwindow.main_outer_layout;
}

/*---------------------------------------------------------------------------*/

Panel *test_window(Window *parent, App* app)
{
    Panel *panel = panel_create();
    gModalData *data = i_modal_data(parent);
    Layout *layout = i_modal_layout(data, parent, app);
    panel_layout(panel, layout);
    panel_data(panel, &data, i_destroy_modal_data, gModalData);
    return panel;
}
