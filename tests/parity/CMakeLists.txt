add_executable(parity_c_capture
    c_capture/capture_main.c
    c_capture/capture_log.c
    c_capture/fake_hid.c
    c_capture/fake_serial.c
    c_capture/fake_pulse.c
    c_capture/wrap_clock.c
    c_capture/wrap_fs.c)

target_include_directories(parity_c_capture PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/c_capture
    ${CMAKE_SOURCE_DIR}/src/cargopit
    ${CMAKE_SOURCE_DIR}/src/cargopit/devices
    ${CMAKE_SOURCE_DIR}/src/cargopit/helper
    ${CMAKE_SOURCE_DIR}/src/cargopit/slog
    ${LUA_INCLUDE_DIR})

target_link_libraries(parity_c_capture
    devices helper slog simulatorapi
    m pthread xml2 config Lua::Lua ${PROC_LIBS})

target_link_options(parity_c_capture PRIVATE
    -Wl,--wrap=clock_gettime
    -Wl,--wrap=gettimeofday
    -Wl,--wrap=usleep
    -Wl,--wrap=fopen
    -Wl,--wrap=glob)

add_executable(uhid_probe c_capture/uhid_probe.c)
add_test(NAME parity_self_check
    COMMAND ${CMAKE_SOURCE_DIR}/tests/parity/self-check.sh
        $<TARGET_FILE:parity_c_capture>
        ${CMAKE_SOURCE_DIR})
set_tests_properties(parity_self_check PROPERTIES TIMEOUT 120)
add_test(NAME uhid_probe COMMAND $<TARGET_FILE:uhid_probe>)
