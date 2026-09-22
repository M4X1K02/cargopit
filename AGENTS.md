# AGENTS.md

Guidance for coding agents working in this repository.

Cargopit is a hard fork of [monocoque](https://github.com/Spacefreak18/monocoque): a C device manager for driving and flight simulators (USB HID, serial/Arduino, haptic shakers, tachometers, wheels, and pedals). It reads live sim data through the [simapi](https://github.com/spacefreak18/simapi) shared-memory API.

User-facing docs: [README.md](README.md), [HOW-TO-USE.md](HOW-TO-USE.md), and [spacefreak18.github.io/simapi](https://spacefreak18.github.io/simapi/).

## Coding rules

Follow these on every change:

1. **Don't nest functions more than 3 levels.** Flatten control flow with early returns, helper functions, or named locals instead of deeper nesting.
2. **DRY: Don't repeat yourself. Reuse code whenever possible.** Extract shared helpers rather than copying device, config, or install logic.
3. **Inverse programming; exceptions and edge cases first.** Validate inputs and handle errors at the top of a function, then the happy path.
4. **No magic strings and no magic numbers.** Use named constants, enums, or `#define`s (see `src/cargopit/helper/confighelper.h` and `src/cargopit/helper/parameters.h`).

## Layout

| Path | Role |
| --- | --- |
| `src/cargopit/` | Main C sources: entry point (`cargopit.c`, built as the `cargopit` binary and library) |
| `src/cargopit/devices/` | USB, serial, sound, haptic, wheel, tachometer backends |
| `src/cargopit/gameloop/` | Play session (`gameloop.c`) and hardware test sequence (`tester.c`) |
| `src/cargopit/helper/` | Config, CLI parameters, paths, simd startup |
| `src/arduino/` | Sample sketches (shift lights, simwind, simhaptic, custom Lua serial) |
| `conf/` | Example `cargopit.config` |
| `tests/` | Automated tests (`ENABLE_TESTS=ON`). Hardware/interactive tools in `tests/manual/` |
| `tui/` | Ratatui manager: `cargopit-tui` |
| `tools/` | Installer helpers, distro packaging |
| `udev/` | `69-cargopit.rules` |
| `.github/workflows/` | PR build (`pr-build.yaml`), installer CI (`installer.yml`), manual release packages (`ci.yaml`) |

## Submodules

Initialize before building:

```bash
git submodule sync --recursive
git submodule update --init --recursive
```

| Path | Upstream | Notes |
| --- | --- | --- |
| `src/cargopit/simulatorapi/simapi` | [M4X1K02/simapi](https://github.com/M4X1K02/simapi) | Shared-memory headers (`simdata.h`). Forked until the DiRT Rally 2 local-velocity mapping is on spacefreak18/simapi. Do not vendor copies. |

Do not edit submodule trees in this repo unless the task is explicitly to bump a submodule pin.

## Build

From a source checkout:

```bash
git submodule update --init --recursive
cmake -B build -DENABLE_TESTS=ON -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

Useful CMake options: `ENABLE_TESTS`, `BUILD_SHARED`, `BUILD_TUI` (default ON; needs cargo). GCC 13+ is required (simapi uses C23 enum-with-underlying-type). Debian 12 / GCC 12 cannot compile current simapi.

End-user source install (compiles simapi, simd, and this tree; does not configure Steam, audio, or wheel firmware):

```bash
./install.sh --from-source
```

## Release

Packages are not published on merge. Push a version tag (for example `0.3.8`) or run **Make Packages** from the Actions tab. Only a tag uploads assets to a GitHub Release.

## Tests

PR CI (`.github/workflows/pr-build.yaml`) configures with `-DENABLE_TESTS=ON` and runs:

```bash
ctest --test-dir build --output-on-failure --timeout 30
```

Automated tests live in `tests/` (`confighelper_test`, `ensure_simd_test`, `startup_play.sh`). Do not add hardware-dependent or interactive tools there; those belong in `tests/manual/`.

Device-level check with real hardware (config must list only connected devices):

```bash
./cargopit test -vv
```

Installer changes should keep `.github/workflows/installer.yml` green. Local container checks:

```bash
bash tools/distro/test-install-containers.sh detect|mocks|immutable|full <distro>
```

Logs: `~/.cache/cargopit/*.log`. A running `cargopit play` answers `status`, `reload` and `stop` on `$XDG_RUNTIME_DIR/cargopit.sock` (one line in, one JSON line out; e.g. `echo status | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/cargopit.sock`). Valgrind: see README (`cd build && valgrind ... --suppressions=../.valgrindrc`).

## Conventions

- Prefer existing enums (`DeviceType`, `DeviceSubType`, `ProgramAction`, `VibrationEffectType`, …) over new stringly-typed switches.
- New USB/serial devices go under `src/cargopit/devices/`; their config names go in the tables in `src/cargopit/helper/devicenames.h` (parse and save both read them, and `tui/tests/config_names.rs` checks the TUI against them). Sample Arduino sketches stay in `src/arduino/`.
- Keep `LICENSE.rst` intact (GPL-3.0-or-later). Packaging copyright inventory: `tools/distro/debian/dpkg/copyright`.
- Do not commit build artifacts (`/build`, `*.flatpak`, `flatpak/repo/`).
- Public usage documentation for sims, bridges, and devices lives at spacefreak18.github.io/simapi, not in this tree. Keep README / HOW-TO-USE pointers accurate; do not duplicate that site here.
