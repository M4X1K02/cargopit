# Cargopit

Cargopit is a hard fork of [monocoque](https://github.com/Spacefreak18/monocoque).
This source was modified in 2026. Original copyright 2022 Paul Jones.
The program remains GNU GPL v3 or later; see [License](#license).

```
   _________    ____  __________  ____  __________
  / ____/   |  / __ \/ ____/ __ \/ __ \/  _/_  __/
 / /   / /| | / /_/ / / __/ / / / /_/ // /  / /
/ /___/ ___ |/ _, _/ /_/ / /_/ / ____// /  / /
\____/_/  |_/_/ |_|\____/\____/_/   /___/ /_/
```

Linux device manager for driving and flight simulators. It reads live telemetry through the [simapi](https://github.com/spacefreak18/simapi) shared-memory API and drives USB HID, serial/Arduino, and PulseAudio (PipeWire) devices.

Usage docs for sims, bridges, and hardware: [spacefreak18.github.io/simapi](https://spacefreak18.github.io/simapi/). After the binaries exist, see [HOW-TO-USE.md](HOW-TO-USE.md).

## Features

- 60 fps update loop with a modular USB, serial, and sound backend.
- Bass shakers over PulseAudio (including PipeWire's Pulse server): engine rumble, gear shifts, ABS, tyre slip/lock, and suspension. Per-device `enabled` and `streamVolume` keys in `cargopit.config`.
- USB haptic shakers with engine rumble mapped across the shaker band, plus chassis/tyre gating so effects stay off when the car is not rolling.
- Tachometers: Revburner only, including existing Revburner XML and `cargopit config tachometer` to write a calibration file.
- Serial output to Arduino and ESP32. Sample sketches for shift lights, simwind, and motor haptics live in `src/arduino/`. Custom serial devices use a [Lua payload format](https://spacefreak18.github.io/simapi/serial_custom).
- Wheels and pedals including Clubsport Elite V3, [Logitech G29](https://spacefreak18.github.io/simapi/logitechg29), Moza R3/R5/R8/R9/KS Pro, Cammus C5/C12, Simagic GT Neo / P1000, and Simnet. Full list: [third-party devices](https://spacefreak18.github.io/simapi/thirdpartydevices).
- `cargopit-tui`: ratatui TUI to start, test, restart, stop, and edit devices/config.
- Starts [simd](https://spacefreak18.github.io/simapi/simd_usage) automatically when it is installed and not already running.

## Adding More Devices

If a device is not already supported, a USB HID pcap or a pull request with working code is the fastest path.

https://santeri.pikarinen.com/pages/usb_hid_reverse_engineering/

## Quick Install

Prefer a packaged build of cargopit when one exists. This fork is also installed from source (`./install.sh --from-source`). The source installer compiles simapi, simd, and cargopit; it does **not** configure Steam, audio devices, or wheel firmware.

**Fedora / Nobara** — use the RPM for your Fedora version from [Releases](https://github.com/M4X1K02/cargopit/releases), plus matching [simapi/simd packages](https://github.com/Spacefreak18/simapi/releases). Nobara is Fedora-based; do not expect a separate installer flavour.

**Debian / Ubuntu / Mint** — use the `.deb` that matches your release from [Releases](https://github.com/M4X1K02/cargopit/releases). Linux Mint often still needs the `libconfig9` (older SONAME) package; if `dpkg` complains about `libconfig`, try the other `.deb` on the same release page.

simshmbridge is not packaged here; use the [prebuilt compatibility EXEs](https://github.com/spacefreak18/simshmbridge/releases).

**Bazzite / Silverblue / Steam Deck (immutable)** — do not layer this with `rpm-ostree`. Use the distrobox helper:

```bash
bash tools/distro/distrobox/install-distrobox.sh
```

This creates an Arch Linux container, installs simapi and simd, builds cargopit from source, and sets up wrapper scripts (`start-simd`, `start-cargopit`, `test-cargopit`, `cargopit-tui`) in `~/.local/bin/`. Uninstall with `bash tools/distro/distrobox/uninstall-distrobox.sh`.

**Build from source** (Arch, Fedora, Debian/Ubuntu, openSUSE, and immutable distros via distrobox). Run the script in a terminal so prompts work:

```bash
git clone https://github.com/M4X1K02/cargopit.git
cd cargopit
git submodule update --init --recursive
./install.sh --from-source
```

Installer options include `--skip-bridges`, `--build-bridges`, `--deps-only`, `--detect-only`, `--force-native`, `--distrobox`, and `--allow-root`. See `./install.sh --help`.

Installer CI (`.github/workflows/installer.yml`) runs these checks in containers: `bash tools/distro/test-install-containers.sh detect|mocks|immutable|full <distro>`.

After install, run `start-cargopit` or `cargopit-tui`. simd is started automatically if it is not already running; you will only be asked to act if simd is not installed. Game and bridge setup: [simd usage](https://spacefreak18.github.io/simapi/simd_usage).

**Supported Games**
[Supported Sims](https://spacefreak18.github.io/simapi/supportedsims).
On Linux some titles need a compatibility exe from simshmbridge. Follow the linked documentation for setup.

## Building

GCC 13+ is required (simapi uses C23 enum-with-underlying-type). Debian 12 ships GCC 12 and cannot compile current simapi; use Ubuntu 24.04, a newer GCC, or a [release `.deb`](https://github.com/M4X1K02/cargopit/releases).

This tree depends on the simapi shared-memory headers as a submodule. If they are missing after clone or pull:

```bash
git submodule sync --recursive
git submodule update --init --recursive
```

Then:

```bash
cmake -B build -DENABLE_TESTS=ON -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

Useful CMake options: `ENABLE_TESTS`, `BUILD_SHARED`, `BUILD_TUI` (default ON; needs cargo). Static analysis: `-Danalyze=on`.

### Dependencies

Vendored/static copies are listed so their licenses stay visible. PulseAudio is the sound backend.

- libserialport — Arduino / serial devices
- hidapi (hidraw) — USB HID
- libpulse — bass shakers and USB shaker streams
- libuv — event loop
- libxml2 — Revburner XML
- argtable2, libconfig, xdg-basedir, lua, libproc2 (or libprocps)
- python3 — some tests
- rustc / cargo — `cargopit-tui`
- [simapi](https://github.com/spacefreak18/simapi) (submodule)
- [slog](https://github.com/kala13x/slog) (static, in-tree)
- [simshmbridge](https://github.com/spacefreak18/simshmbridge) — optional; shared-memory titles such as Assetto Corsa and Project CARS–related sims

**Arch**

```bash
pacman -S --needed git cmake base-devel python curl libuv argtable libserialport libconfig hidapi lua54 libpulse pkgconf libxdg-basedir libxml2 yder procps-ng rust
```

**Fedora / Nobara**

```bash
dnf install git cmake gcc gcc-c++ make pkgconf-pkg-config python3 curl libuv-devel argtable-devel libserialport-devel libconfig-devel hidapi-devel lua-devel libxdg-basedir-devel libxml2-devel pulseaudio-libs-devel procps-ng-devel cargo
```

`yder-devel` (needed to build simd) is often missing from Fedora repos. `install.sh` builds yder from source when the package is absent. Extra packages: https://repo.spacefreak18.xyz/Packages/Fedora/43/

**Debian / Ubuntu / Mint**

```bash
apt install build-essential git cmake pkg-config python3 cargo rustc libuv1-dev libargtable2-dev libserialport-dev libconfig-dev libhidapi-dev liblua5.4-dev libxdg-basedir-dev libxml2-dev libpulse-dev libproc2-dev
```

Use `liblua5.3-dev` if 5.4 is not in the repo, and `libprocps-dev` if `libproc2-dev` is absent. `libyder-dev` is similarly optional; the installer can build yder.

**openSUSE**

```bash
zypper install git cmake gcc gcc-c++ make pkg-config python3 cargo rust libuv-devel argtable-devel libserialport-devel libconfig-devel hidapi-devel lua-devel libxdg-basedir-devel libxml2-devel libpulse-devel procps-devel
```

End-user source install (compiles simapi, simd, and this tree):

```bash
./install.sh --from-source
```

## User Setup Guide

See [HOW-TO-USE.md](HOW-TO-USE.md) to configure devices with `cargopit-tui`, udev/groups, Steam launch options, and simd. Keep only connected devices in the config, or set `enabled` to `false` on unused entries.

## Testing

Automated suite (same as PR CI in `.github/workflows/pr-build.yaml`):

```bash
cmake -B build -DENABLE_TESTS=ON -DCMAKE_BUILD_TYPE=Debug
cmake --build build
ctest --test-dir build --output-on-failure --timeout 30
```

Hardware check (config must list only connected devices):

```bash
./cargopit test -vv
```

Logs: `~/.cache/cargopit/*.log`.

### Static Analysis

```bash
cmake -B build -Danalyze=on
cmake --build build
```

### Valgrind

```bash
cd build
valgrind -v --leak-check=full --show-leak-kinds=all --suppressions=../.valgrindrc ./cargopit play
```

## Join the Discussion

[Sim Racing Matrix Space](https://matrix.to/#/#simracing:matrix.org)

## License

The program is GNU GPL v3 or later. Keep `LICENSE.rst` intact; that file is
the GPL text. Debian-format inventory of this tree and bundled works:
`tools/distro/debian/dpkg/copyright`.

| Component | License | Where |
| --- | --- | --- |
| cargopit (this fork) | GPL-3.0-or-later | `LICENSE.rst` |
| slog | MIT | `src/cargopit/slog/slog.h` |
| simapi (submodule) | LGPL-3.0 | https://github.com/Spacefreak18/simapi |
| Lua 5.4 | MIT | `packaging/licenses/lua-5.4-LICENSE.txt` |
| ratatui / crossterm (TUI) | MIT | Cargo crates linked into `cargopit-tui` |

## ToDo

- frequency cap (low-pass filter) for sound haptic effects
- road and kerb sound haptic effects
- Windows port
- more memory testing
- cleanup tests which are copies of upstream examples
- much, much more
