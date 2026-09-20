cmake_minimum_required(VERSION 3.18)
# detect if Cargopit is being used as a sub-project of another CMake project
if(NOT DEFINED PROJECT_NAME)
    set(CARGOPIT_SUBPROJECT OFF)
else()
    set(CARGOPIT_SUBPROJECT ON)
endif()


SET_SOURCE_FILES_PROPERTIES( src/cargopit.c PROPERTIES LANGUAGE C)
#set(CMAKE_BUILD_TYPE Debug)

option(BUILD_SHARED "Build cargopit shared library" OFF)
option(ENABLE_TESTS "Build test suite" OFF)
option(BUILD_TUI "Build cargopit-tui (ratatui)" ON)


project(cargopit)

set(CMAKE_EXE_LINKER_FLAGS "-Wl,--no-as-needed -ldl")
set(LIBUSB_INCLUDE_DIR /usr/include/libusb-1.0)
set(LIBXML_INCLUDE_DIR /usr/include/libxml2)

FIND_PACKAGE(Lua)
set(INCLUDE_DIRS ${LUA_INCLUDE_DIR})
include_directories(${LUA_INCLUDE_DIR})
if(Lua_FOUND AND NOT TARGET Lua::Lua)
  add_library(Lua::Lua INTERFACE IMPORTED)
  set_target_properties(
    Lua::Lua
    PROPERTIES
      INTERFACE_INCLUDE_DIRECTORIES "${LUA_INCLUDE_DIR}"
      INTERFACE_LINK_LIBRARIES "${LUA_LIBRARIES}"
  )

endif()
message("Lua ${Lua_VERSION} found")
if (Lua_VERSION VERSION_GREATER_EQUAL "5.5.0")
    add_compile_definitions(USE_LUA_55)
else()
    remove_definitions(USE_LUA_55)
endif()

#FIND_PATH(LIBUSB_INCLUDE_DIR libusb.h
#	HINTS $ENV{LIBUSB_ROOT}
#	PATHS ${PC_LIBUSB_INCLUDEDIR} ${PC_LIBUSB_INCLUDE_DIRS}
#	PATH_SUFFIXES include)
#
#FIND_LIBRARY(LIBUSB_LIBRARY NAMES usb-1.0
#	HINTS $ENV{LIBUSB_ROOT}
#	PATHS ${PC_LIBUSB_LIBDIR} ${PC_LIBUSB_LIBRARY_DIRS}
#	PATH_SUFFIXES lib)
#
set(HIDAPI_WITH_LIBUSB FALSE) # surely will be used only on Linux
set(BUILD_SHARED_LIBS TRUE) # HIDAPI as static library on all platforms

if(BUILD_SHARED)
    add_library(cargopit SHARED src/cargopit/cargopit.c)
else()
    add_library(cargopit STATIC src/cargopit/cargopit.c)
endif()
add_executable(cargopit-cli src/cargopit/cargopit-cli.c)
set_target_properties(cargopit-cli PROPERTIES OUTPUT_NAME cargopit)

find_package(PkgConfig REQUIRED)
pkg_check_modules(LIBPROCPS libprocps)
pkg_check_modules(LIBPROC2 libproc2)

if (LIBPROC2_FOUND)
    set(PROC_LIBS ${LIBPROC2_LIBRARIES})
    # Some distro packagings (e.g. the freedesktop Flatpak SDK) ship a
    # libproc2.pc with `Version: UNKNOWN` rather than a real version
    # string. VERSION_GREATER_EQUAL against that always evaluates false,
    # silently falling through to the old (pre-4.0.5) PIDS_VAL API and
    # failing to compile against an actually-current libproc2 whose real
    # header has already moved to the new 3-argument macro. Only take the
    # old-API branch when the version string is genuinely parseable and
    # actually old.
    if (LIBPROC2_VERSION MATCHES "^[0-9]" AND LIBPROC2_VERSION VERSION_LESS "4.0.5")
        add_compile_definitions(USE_OLD_PID_VAL=1)
    endif()
elseif (LIBPROCPS_FOUND)
    set(PROC_LIBS ${LIBPROCPS_LIBRARIES})
    add_compile_definitions(USE_OLD_PID_VAL=1)
else()
    message(FATAL_ERROR "Either libprocps or libproc2 is required")
endif()

#if(USE_PULSEAUDIO)
#else()
#endif()

message("Using pulseaudio backend...")
add_compile_definitions(USE_PULSEAUDIO=true)

add_subdirectory(src/cargopit/gameloop)
add_subdirectory(src/cargopit/simulatorapi)
add_subdirectory(src/cargopit/helper)
add_subdirectory(src/cargopit/devices)
add_subdirectory(src/cargopit/slog)

target_link_libraries(cargopit m hidapi-hidraw pulse serialport xml2 argtable2 config gameloop helper devices slog simulatorapi uv xdg-basedir Lua::Lua)
target_include_directories(cargopit PUBLIC config ${LIBXML_INCLUDE_DIR} ${LUA_INCLUDE_DIR})
target_link_libraries(cargopit-cli m hidapi-hidraw pulse serialport xml2 argtable2 config gameloop helper devices slog simulatorapi uv xdg-basedir Lua::Lua)
target_include_directories(cargopit-cli PUBLIC config ${LIBXML_INCLUDE_DIR} ${LUA_INCLUDE_DIR})




# used for enabling additional compiler options if supported
include(CheckCXXCompilerFlag)

function(enable_cxx_compiler_flag_if_supported flag)
    message(STATUS "[cargopit] Checking if compiler supports warning flag '${flag}'")
    check_cxx_compiler_flag("${flag}" flag_supported)
    if(flag_supported)
        message(STATUS "[cargopit] Enabling warning flag '${flag}'")
        target_compile_options(cargopit INTERFACE "${flag}")
    endif()
    unset(flag_supported CACHE)
endfunction()

# enable a large amount of extra warnings, regardless of build mode
if (MSVC) # MSVC supports different warning options to GCC/Clang
    enable_cxx_compiler_flag_if_supported("/W3")  # set warning level 3
    # if tests are enabled, enable converting all warnings to errors too
    if (ENABLE_TESTS)
        # add_compile_options(/WX)
        enable_cxx_compiler_flag_if_supported("/WX")
    endif()
else() # GCC/Clang warning option
    # NOTE: GCC and Clang support most of the same options, but neither supports all
    # of the others'. By only enabling them if supported, we get graceful failure
    # when trying to enable unsupported flags
    # e.g. at the time of writing, GCC does not support -Wdocumentation
    #
    # enable all warnings about 'questionable constructs'

    if(analyze)
        message("-- Analyzer is on")
        target_compile_options(cargopit PRIVATE -fanalyzer -Wno-analyzer-possible-null-argument -Wno-analyzer-possible-null-dereference -fsanitize=address -g -fno-omit-frame-pointer)
    endif()

    enable_cxx_compiler_flag_if_supported("-Wall")
    # issue 'pedantic' warnings for strict ISO compliance
    enable_cxx_compiler_flag_if_supported("-pedantic")
    # enable 'extra' strict warnings
    enable_cxx_compiler_flag_if_supported("-Wextra")
    # enable sign conversion warnings
    enable_cxx_compiler_flag_if_supported("-Wsign-conversion")
    # enable warnings about mistakes in Doxygen documentation
    enable_cxx_compiler_flag_if_supported("-Wdocumentation")
    # if tests are enabled, enable converting all warnings to errors too
    if (ENABLE_TESTS)
        enable_cxx_compiler_flag_if_supported("-Werror")
        # exclude the following kinds of warnings from being converted into errors
        # unknown-pragma is useful to have as a warning but not as an error, if you have
        # pragmas which are for the consumption of one compiler only
        enable_cxx_compiler_flag_if_supported("-Wno-error=unknown-pragmas")
        # unused variable and function warnings are helpful but we don't need them as errors
        enable_cxx_compiler_flag_if_supported("-Wno-error=unused-function")
        enable_cxx_compiler_flag_if_supported("-Wno-error=unused-variable")
        enable_cxx_compiler_flag_if_supported("-Wno-error=unused-parameter")
        enable_cxx_compiler_flag_if_supported("-Wno-error=unused-private-field")
        enable_cxx_compiler_flag_if_supported("-Wno-error=unused-but-set-variable")
    endif()
endif()

# library
# unit tests --only enable if requested AND we're not building as a sub-project
if(ENABLE_TESTS AND NOT CARGOPIT_SUBPROJECT)
    message(STATUS "[cargopit] Unit Tests Enabled")
    enable_testing()
    add_subdirectory(tests)
endif()

if(BUILD_TUI)
    find_program(CARGO_EXECUTABLE cargo)
    if(NOT CARGO_EXECUTABLE)
        message(FATAL_ERROR "cargo is required when BUILD_TUI is ON. Install rustc/cargo or pass -DBUILD_TUI=OFF")
    endif()
    set(TUI_TARGET_DIR "${CMAKE_BINARY_DIR}/tui")
    set(CARGO_COMMAND ${CARGO_EXECUTABLE} build)
    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        set(TUI_BIN "${TUI_TARGET_DIR}/debug/cargopit-tui")
    else()
        list(APPEND CARGO_COMMAND --release)
        set(TUI_BIN "${TUI_TARGET_DIR}/release/cargopit-tui")
    endif()
    list(APPEND CARGO_COMMAND
        --manifest-path "${CMAKE_SOURCE_DIR}/tui/Cargo.toml"
        --target-dir "${TUI_TARGET_DIR}")
    file(GLOB_RECURSE TUI_RUST_SOURCES CONFIGURE_DEPENDS
        "${CMAKE_SOURCE_DIR}/tui/src/*.rs")
    add_custom_command(
        OUTPUT "${TUI_BIN}"
        COMMAND ${CARGO_COMMAND}
        DEPENDS
            ${TUI_RUST_SOURCES}
            "${CMAKE_SOURCE_DIR}/tui/Cargo.toml"
            "${CMAKE_SOURCE_DIR}/tui/Cargo.lock"
            "${CMAKE_SOURCE_DIR}/tui/build.rs"
            "${CMAKE_SOURCE_DIR}/tui/native/simapi_view.c"
            "${CMAKE_SOURCE_DIR}/tui/native/simapi_view.h"
            "${CMAKE_SOURCE_DIR}/tui/native/simapi_view_stub.c"
        WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}/tui"
        COMMENT "Building cargopit-tui with cargo"
        VERBATIM)
    add_custom_target(cargopit-tui ALL DEPENDS "${TUI_BIN}")
    install(PROGRAMS "${TUI_BIN}" DESTINATION bin)
endif()

install(TARGETS cargopit-cli RUNTIME DESTINATION bin)
install(FILES tools/cargopit.desktop DESTINATION share/applications)
install(DIRECTORY conf/ DESTINATION share/cargopit/conf)
