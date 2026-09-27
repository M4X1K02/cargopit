# Automated CI test suite — only built/run when ENABLE_TESTS is on (see
# root CMakeLists.txt). Manual, interactive, and hardware-dependent scratch
# tools live in tests/manual/ instead and are never built by this file.

add_test(NAME cargo_workspace
    COMMAND ${CARGO_EXECUTABLE} test --workspace --exclude cargopit-tui
        --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
        --target-dir "${CARGO_TARGET_DIR}"
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}")
set_tests_properties(cargo_workspace PROPERTIES TIMEOUT 300)

if(BUILD_TUI)
    add_test(NAME cargo_tui
        COMMAND ${CARGO_EXECUTABLE} test -p cargopit-tui
            --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
            --target-dir "${CARGO_TARGET_DIR}"
        WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}")
    set_tests_properties(cargo_tui PROPERTIES TIMEOUT 180)
endif()

find_package(Python3 COMPONENTS Interpreter)
if (Python3_Interpreter_FOUND)
    add_test(NAME fr_to_filterchain_test
        COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/fr_to_filterchain_test.py)
endif()

find_program(BASH_PROGRAM bash)
if (BASH_PROGRAM)
    add_test(NAME version_reporting
        COMMAND ${BASH_PROGRAM} ${CMAKE_CURRENT_SOURCE_DIR}/version_reporting.sh
            ${CARGOPIT_STAGED} ${PROJECT_VERSION})
    set_tests_properties(version_reporting PROPERTIES
        REQUIRED_FILES ${CARGOPIT_STAGED})
    add_test(NAME startup_play_missing_simd
        COMMAND ${BASH_PROGRAM} ${CMAKE_CURRENT_SOURCE_DIR}/startup_play.sh missing ${CARGOPIT_STAGED})
    set_tests_properties(startup_play_missing_simd PROPERTIES
        REQUIRED_FILES ${CARGOPIT_STAGED})
    add_test(NAME startup_play_starts_simd
        COMMAND ${BASH_PROGRAM} ${CMAKE_CURRENT_SOURCE_DIR}/startup_play.sh starts ${CARGOPIT_STAGED})
    set_tests_properties(startup_play_starts_simd PROPERTIES
        REQUIRED_FILES ${CARGOPIT_STAGED})
    add_test(NAME check_clock
        COMMAND ${BASH_PROGRAM} ${CMAKE_CURRENT_SOURCE_DIR}/parity/check-clock.sh)
    add_test(NAME shadow_virtual_jobs
        COMMAND ${BASH_PROGRAM} ${CMAKE_CURRENT_SOURCE_DIR}/shadow/virtual-jobs.sh)
    set_tests_properties(shadow_virtual_jobs PROPERTIES
        ENVIRONMENT "CARGOPIT_BIN=${CARGOPIT_STAGED}")
endif()

add_executable(uhid_probe parity/uhid_probe.c)
add_test(NAME uhid_probe COMMAND uhid_probe)
