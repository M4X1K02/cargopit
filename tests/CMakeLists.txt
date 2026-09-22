# Automated CI test suite — only built/run when ENABLE_TESTS is on (see
# root CMakeLists.txt). Manual, interactive, and hardware-dependent scratch
# tools live in tests/manual/ instead and are never built by this file.

add_executable(confighelper_test confighelper_test.c)
# helper (confighelper.c specifically) references slog/xml2/simulatorapi
# symbols elsewhere in the same translation unit — static libs resolve per
# object file, so all of these are needed even though this test only
# exercises strtodevsubsubtype.
target_link_libraries(confighelper_test helper config slog xml2 simulatorapi m ${PROC_LIBS})
add_test(NAME confighelper_test COMMAND confighelper_test)

add_executable(ensure_simd_test ensure_simd_test.c)
target_link_libraries(ensure_simd_test helper config slog xml2 simulatorapi m ${PROC_LIBS})
add_test(NAME ensure_simd_test COMMAND ensure_simd_test)

add_executable(simd_telemetry_test simd_telemetry_test.c)
target_link_libraries(simd_telemetry_test helper config slog xml2 simulatorapi m ${PROC_LIBS})
add_test(NAME simd_telemetry_test COMMAND simd_telemetry_test)

add_executable(tester_simdata_test tester_simdata_test.c)
target_link_libraries(tester_simdata_test
    gameloop helper devices slog simulatorapi
    m hidapi-hidraw pulse serialport xml2 config uv xdg-basedir
    Lua::Lua ${PROC_LIBS})
target_include_directories(tester_simdata_test PRIVATE
    ${CMAKE_SOURCE_DIR}/src/cargopit
    ${CMAKE_SOURCE_DIR}/src/cargopit/gameloop
    ${CMAKE_SOURCE_DIR}/src/cargopit/helper
    ${CMAKE_SOURCE_DIR}/src/cargopit/devices
    ${LUA_INCLUDE_DIR})
add_test(NAME tester_simdata_test COMMAND tester_simdata_test)

add_executable(acr_udp_test acr_udp_test.c)
target_link_libraries(acr_udp_test
    gameloop helper devices slog simulatorapi
    m hidapi-hidraw pulse serialport xml2 config uv xdg-basedir
    Lua::Lua ${PROC_LIBS})
target_include_directories(acr_udp_test PRIVATE
    ${CMAKE_SOURCE_DIR}/src/cargopit
    ${CMAKE_SOURCE_DIR}/src/cargopit/gameloop
    ${CMAKE_SOURCE_DIR}/src/cargopit/helper
    ${CMAKE_SOURCE_DIR}/src/cargopit/devices
    ${LUA_INCLUDE_DIR})
add_test(NAME acr_udp_test COMMAND acr_udp_test)

add_executable(haptic_effect_test haptic_effect_test.c)
target_link_libraries(haptic_effect_test
    devices helper slog simulatorapi
    m hidapi-hidraw pulse serialport xml2 config uv xdg-basedir
    Lua::Lua ${PROC_LIBS})
target_include_directories(haptic_effect_test PRIVATE ${LUA_INCLUDE_DIR})
add_test(NAME haptic_effect_test COMMAND haptic_effect_test)

find_program(BASH_PROGRAM bash)
if (BASH_PROGRAM)
    add_test(NAME startup_play_missing_simd
        COMMAND ${BASH_PROGRAM} ${CMAKE_CURRENT_SOURCE_DIR}/startup_play.sh missing $<TARGET_FILE:cargopit-cli>)
    add_test(NAME startup_play_starts_simd
        COMMAND ${BASH_PROGRAM} ${CMAKE_CURRENT_SOURCE_DIR}/startup_play.sh starts $<TARGET_FILE:cargopit-cli>)
endif()
