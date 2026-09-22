#ifndef _GAMELOOP_H
#define _GAMELOOP_H

#include "../devices/simdevice.h"
#include "../helper/parameters.h"
#include "loopdata.h"

/* Play mode on the calling thread until the user quits or a signal arrives. */
int cargopit_mainloop(CargopitSettings* ms);

/* Play mode on a background thread for embedders; stop it with cargopit_mainloop_stop() from any thread. */
int start_loop(CargopitSettings* ms);
int cargopit_mainloop_stop(CargopitSettings* ms);
AppState gameloop_app_state(void);
const char* get_simd_onoff(void);
const char* get_simexe_name(void);

/* Hardware test sequence (tester.c). */
int tester(SimDevice* devices, int numdevices);
int run_hardware_test(CargopitSettings* ms, int config_index, int device_index);
int start_test(test_loop_args* test_loop_data);
int cargopit_testloop_stop(void);
SimData* get_test_simdata(void);

void set_basic_simdata(SimData* simdata);
void set_wheel_spin_simdata(SimData* simdata);
void set_wheel_lock_simdata(SimData* simdata);

#endif
