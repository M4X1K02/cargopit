# Cargopit

Cargopit is a hard fork of [monocoque](https://github.com/Spacefreak18/monocoque).
This source was modified in 2026. Original copyright 2022 Paul Jones.
The program remains GNU GPL v3 or later; see [License](#license).

```
___   |/  /____________________________________ ____  ______ 
__  /|_/ /_  __ \_  __ \  __ \  ___/  __ \  __ `/  / / /  _ \
_  /  / / / /_/ /  / / / /_/ / /__ / /_/ / /_/ // /_/ //  __/
/_/  /_/  \____//_/ /_/\____/\___/ \____/\__, / \__,_/ \___/ 
                                           /_/
```
Cross Platform device manager for driving and flight simulators, for use with common simulator software titles.

📚 **Usage Documentation:** [spacefreak18.github.io/simapi/](https://spacefreak18.github.io/simapi/)

## Features
- Updates at 60 frames per seconds.
- Modular design for support with various titles and devices.
- Supports bass shakers, tachometers, several wheels and pedals, simlights, simwind etc, through usb and arduino serial.
- Tachometer support is currently  limited to the Revburner model. Supports existing revburner xml configuration files.
- Includes utility to configure revburner tachometer
- Can send data to any serial device. So far only tested with arduino. and ESP32. Includes sample arduino sketch for sim lights, simwind, and simhaptic effects for motors.
- Support for custom arduino (or any serial) device, with a [custom lua format](https://spacefreak18.github.io/simapi/serial_custom) for sending data
- Convincing shaker effects for noise tranducers for wheel slip, wheel lock, and abs, as well as engine rpm and gear shifts.
- Support for many [wheels and pedals](https://spacefreak18.github.io/simapi/thirdpartydevices) including Clubsport Elite V3, [Logitech G29](https://spacefreak18.github.io/simapi/logitechg29), and Moza R5.

## Adding More Devices
- If a device isn't already supported, feel free to assist in the reverse engineering process by submitting a pcap or even better working code in a pull request!
https://santeri.pikarinen.com/pages/usb_hid_reverse_engineering/

## Quick Install

Prefer a packaged build of upstream monocoque when one exists. This fork is installed from source (`./install.sh --from-source`). The source installer compiles simapi, simd, and cargopit; it does **not** configure Steam, audio devices, or wheel firmware.

**Arch Linux (AUR)** — install simapi first, then simd, then monocoque:
```bash
yay -S simapi-git
yay -S simd-git
yay -S monocoque-git
```
simshmbridge is not in AUR; use the [prebuilt compatibility EXEs](https://github.com/spacefreak18/simshmbridge/releases).

**Fedora / Nobara** — use the RPM for your Fedora version from [Releases](https://github.com/Spacefreak18/monocoque/releases), plus matching [simapi/simd packages](https://github.com/Spacefreak18/simapi/releases). Nobara is Fedora-based; do not expect a separate installer flavour.

**Debian / Ubuntu / Mint** — use the `.deb` that matches your release from [Releases](https://github.com/Spacefreak18/monocoque/releases). Linux Mint often still needs the `libconfig9` (older SONAME) package; if `dpkg` complains about `libconfig`, try the other `.deb` on the same release page.

**Bazzite / Silverblue / Steam Deck (immutable)** — do not layer this with `rpm-ostree`. Use the distrobox helper:
```bash
bash tools/distro/distrobox/install-distrobox.sh
```
This creates an Arch Linux container, installs packages via AUR, and sets up wrapper scripts (`start-simd`, `start-cargopit`, `test-cargopit`) in `~/.local/bin/`. Uninstall with `bash tools/distro/distrobox/uninstall-distrobox.sh`.

**Build from source** (any supported distro). Download the script and run it in a terminal so prompts work (`curl | bash` cannot answer the AUR question and cannot find `cargopit-manager` next to itself):
```bash
git clone https://github.com/M4X1K02/cargopit.git
cd cargopit
git submodule update --init --recursive
./install.sh --from-source
```

Options: `--aur`, `--skip-bridges`, `--build-bridges`, `--deps-only`. See `./install.sh --help`.

Installer CI (`.github/workflows/installer.yml`) runs these checks in containers: `bash tools/distro/test-install-containers.sh detect|mocks|immutable|full <distro>`.

After install, run `start-cargopit` or `cargopit-manager`. simd is started automatically if it is not already running; you will only be asked to act if simd is not installed. Game and bridge setup: [simd usage](https://spacefreak18.github.io/simapi/simd_usage). Full docs: [spacefreak18.github.io/simapi](https://spacefreak18.github.io/simapi/).

For a manual walkthrough, see [HOW-TO-USE.md](HOW-TO-USE.md).

**Supported Games**
[Supported Sims](https://spacefreak18.github.io/simapi/supportedsims)
On Linux some titles need a compatibility exe from simshmbridge. Follow the linked documentation for setup.

## Building

### Dependencies (static dependencies are linked as to show their respective copyright and licenses)
- libserialport - arduino serial devices
- hidapi - usb hid devices (hidraw)
- portaudio - sound devices (haptic bass shakers)
- libpulse - sound devices (haptic bass shakers)
- libuv base event loop
- libxml2
- argtable2
- libconfig
- xdg-basedir
- lua
- libproc2
- libcurl4
- libgtk3
- libglu1-mesa
- [simapi](https://github.com/spacefreak18/simapi)
- [slog](https://github.com/kala13x/slog) (static)
- [nappgui](https://github.com/frang75/nappgui_src) (static) (for GUI)
(sorta optional)
- [simshmbridge](https://github.com/spacefreak18/simshmbridge) - for sims that need shared memory mapping like AC and Project Cars related.

**Arch**
```
pacman -S --needed git cmake base-devel pulse-native-provider libxdg-basedir libserialport libconfig libuv argtable hidapi lua54 libxml2 pkgconf procps-ng
```

**Fedora / Nobara**
```
dnf install git cmake gcc gcc-c++ make libuv-devel argtable-devel libserialport-devel libconfig-devel hidapi-devel lua-devel libxdg-basedir-devel libxml2-devel pulseaudio-libs-devel pkgconf-pkg-config procps-ng-devel
```
`yder-devel` (needed to build simd) is often missing from Fedora repos. `install.sh` builds yder from source when the package is absent. Extra packages: https://repo.spacefreak18.xyz/Packages/Fedora/43/

**Debian / Ubuntu / Mint**
```
apt install build-essential git cmake libuv1-dev libargtable2-dev libserialport-dev libconfig-dev libhidapi-dev liblua5.4-dev libxdg-basedir-dev libxml2-dev libpulse-dev pkg-config libproc2-dev
```
Use `liblua5.3-dev` if 5.4 is not in the repo. `libyder-dev` is similarly optional; the installer can build yder.

Debian 12 (bookworm) ships GCC 12, which cannot compile current simapi. Use the [release .deb](https://github.com/Spacefreak18/monocoque/releases), Ubuntu 24.04, or a newer GCC.

This code depends on the shared memory data headers in the simapi [repo](https://github.com/spacefreak18/simapi). When pulling lastest if the submodule does not download run:
```
git submodule sync --recursive
git submodule update --init --recursive
```

Then to compile simply:
```
mkdir build; cd build
cmake ..
make
```

## User Setup Guide
See the dedicated [How To](HOW-TO-USE.md) for detailed instructions to set up and run 'cargopit`

## Testing
```
./cargopit test -vv # Make sure that ~/.config/cargopit/cargopit.config only contains the devices you have connected.
```

### Logs file location
`~/.cache/cargopit/*.log`

### Static Analysis
```
    mkdir build; cd build
    make clean
    cmake -Danalyze=on ..
    make
```

### Valgrind
```
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
| NAppGUI | MIT | `src/cargopit/mgui/nappgui_src/LICENSE`, `packaging/licenses/nappgui-LICENSE.txt` |
| simapi (submodule) | LGPL-3.0 | https://github.com/Spacefreak18/simapi |
| Lua 5.4 | MIT | `packaging/licenses/lua-5.4-LICENSE.txt` |
| GLU | SGI Free B 2.0 | `packaging/licenses/glu-LICENSE.txt` |

## ToDo
 - add frequency cap (low pass filter) to sound haptic effects
 - add road and kerb sound haptic effects
 - windows port
 - more memory testing
 - cleanup tests which are basically just copies of the example from their respective projects
 - much, much more
