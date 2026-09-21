#include "../devices/simdevice.h"
#include "../helper/parameters.h"
#include "loopdata.h"

extern int appstate;

int tester(SimDevice* devices, int numdevices);
int run_hardware_test(CargopitSettings* ms, int config_index, int device_index);
int looper(SimDevice* devices, int numdevices, Parameters* p);

int cargopit_mainloop(CargopitSettings* ms);

int start_loop(CargopitSettings* ms);
int start_test(test_loop_args* test_loop_data);
int cargopit_mainloop_stop(CargopitSettings* ms);
int cargopit_testloop_stop(void);

const char* get_simd_onoff(void);
const char* get_simexe_name(void);
SimData* get_test_simdata(void);


void set_basic_simdata(SimData* simdata);
void set_wheel_spin_simdata(SimData* simdata);
void set_wheel_lock_simdata(SimData* simdata);
