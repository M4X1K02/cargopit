cmake_minimum_required(VERSION 3.18)
# detect if Cargopit is being used as a sub-project of another CMake project
if(NOT DEFINED PROJECT_NAME)
    set(CARGOPIT_SUBPROJECT OFF)
else()
    set(CARGOPIT_SUBPROJECT ON)
endif()

set(CARGOPIT_VERSION_FILE "${CMAKE_CURRENT_LIST_DIR}/version.txt")
if(NOT EXISTS "${CARGOPIT_VERSION_FILE}")
    message(FATAL_ERROR "Missing Cargopit version file: ${CARGOPIT_VERSION_FILE}")
endif()

file(STRINGS "${CARGOPIT_VERSION_FILE}" CARGOPIT_VERSION LIMIT_COUNT 1)
string(STRIP "${CARGOPIT_VERSION}" CARGOPIT_VERSION)
if(NOT CARGOPIT_VERSION MATCHES "^[0-9]+\\.[0-9]+\\.[0-9]+$")
    message(FATAL_ERROR
        "Cargopit version must use MAJOR.MINOR.PATCH: '${CARGOPIT_VERSION}'")
endif()

option(ENABLE_TESTS "Build test suite" OFF)
option(BUILD_TUI "Build cargopit-tui (ratatui)" ON)

project(cargopit VERSION "${CARGOPIT_VERSION}")

find_program(CARGO_EXECUTABLE cargo)
if(NOT CARGO_EXECUTABLE)
    message(FATAL_ERROR "cargo is required. Install rustc/cargo or rustup.")
endif()

set(CARGO_TARGET_DIR "${CMAKE_BINARY_DIR}/cargo")
set(CARGOPIT_STAGED "${CMAKE_BINARY_DIR}/cargopit")
set(CARGOPIT_TUI_STAGED "${CMAKE_BINARY_DIR}/cargopit-tui")

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(CARGO_PROFILE_DIR debug)
    set(CARGO_RELEASE_ARG)
else()
    set(CARGO_PROFILE_DIR release)
    set(CARGO_RELEASE_ARG --release)
endif()

set(CARGOPIT_BIN "${CARGO_TARGET_DIR}/${CARGO_PROFILE_DIR}/cargopit")
set(CARGOPIT_TUI_BIN "${CARGO_TARGET_DIR}/${CARGO_PROFILE_DIR}/cargopit-tui")

set(CARGO_BUILD_CMD
    ${CARGO_EXECUTABLE} build ${CARGO_RELEASE_ARG}
    --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
    --target-dir "${CARGO_TARGET_DIR}"
    -p cargopit)
set(CARGO_OUTPUTS "${CARGOPIT_BIN}")
if(BUILD_TUI)
    list(APPEND CARGO_BUILD_CMD -p cargopit-tui)
    list(APPEND CARGO_OUTPUTS "${CARGOPIT_TUI_BIN}")
endif()

file(GLOB_RECURSE CARGOPIT_RUST_SOURCES CONFIGURE_DEPENDS
    "${CMAKE_SOURCE_DIR}/crates/*.rs"
    "${CMAKE_SOURCE_DIR}/crates/*.toml"
    "${CMAKE_SOURCE_DIR}/tui/*.rs"
    "${CMAKE_SOURCE_DIR}/src/cargopit/simulatorapi/simapi/simapi/*.c"
    "${CMAKE_SOURCE_DIR}/src/cargopit/simulatorapi/simapi/simapi/*.h"
    "${CMAKE_SOURCE_DIR}/src/cargopit/simulatorapi/simapi/simmap/*.c"
    "${CMAKE_SOURCE_DIR}/src/cargopit/simulatorapi/simapi/simmap/*.h"
    "${CMAKE_SOURCE_DIR}/crates/simapi-sys/csrc/*.c"
    "${CMAKE_SOURCE_DIR}/crates/simapi-sys/csrc/*.h")

add_custom_command(
    OUTPUT ${CARGO_OUTPUTS}
    COMMAND ${CARGO_BUILD_CMD}
    DEPENDS
        ${CARGOPIT_RUST_SOURCES}
        "${CMAKE_SOURCE_DIR}/Cargo.toml"
        "${CMAKE_SOURCE_DIR}/Cargo.lock"
        "${CMAKE_SOURCE_DIR}/version.txt"
        "${CMAKE_SOURCE_DIR}/tui/Cargo.toml"
        "${CMAKE_SOURCE_DIR}/tui/build.rs"
        "${CMAKE_SOURCE_DIR}/crates/simapi-sys/build.rs"
        "${CMAKE_SOURCE_DIR}/crates/cargopit/Cargo.toml"
        "${CMAKE_SOURCE_DIR}/crates/cargopit-config/Cargo.toml"
        "${CMAKE_SOURCE_DIR}/crates/cargopit-devices/Cargo.toml"
    WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
    COMMENT "Building cargopit with cargo"
    VERBATIM)

add_custom_command(
    OUTPUT "${CARGOPIT_STAGED}"
    COMMAND ${CMAKE_COMMAND} -E copy "${CARGOPIT_BIN}" "${CARGOPIT_STAGED}"
    DEPENDS "${CARGOPIT_BIN}"
    COMMENT "Staging cargopit"
    VERBATIM)

set(CARGOPIT_STAGE_TARGETS "${CARGOPIT_STAGED}")
if(BUILD_TUI)
    add_custom_command(
        OUTPUT "${CARGOPIT_TUI_STAGED}"
        COMMAND ${CMAKE_COMMAND} -E copy "${CARGOPIT_TUI_BIN}" "${CARGOPIT_TUI_STAGED}"
        DEPENDS "${CARGOPIT_TUI_BIN}"
        COMMENT "Staging cargopit-tui"
        VERBATIM)
    list(APPEND CARGOPIT_STAGE_TARGETS "${CARGOPIT_TUI_STAGED}")
endif()

add_custom_target(cargopit-host ALL DEPENDS ${CARGOPIT_STAGE_TARGETS})

install(PROGRAMS "${CARGOPIT_STAGED}" DESTINATION bin)
if(BUILD_TUI)
    install(PROGRAMS "${CARGOPIT_TUI_STAGED}" DESTINATION bin)
endif()
install(FILES tools/cargopit.desktop DESTINATION share/applications)
install(FILES tools/cargopit.svg
    DESTINATION share/icons/hicolor/scalable/apps)
install(DIRECTORY conf/ DESTINATION share/cargopit/conf)

if(ENABLE_TESTS AND NOT CARGOPIT_SUBPROJECT)
    message(STATUS "[cargopit] Unit Tests Enabled")
    enable_testing()
    add_subdirectory(tests)
endif()
