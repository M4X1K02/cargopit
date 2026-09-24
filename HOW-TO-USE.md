# Cargopit User Setup Guide

Game/bridge details live in the [simapi docs](https://spacefreak18.github.io/simapi/simd_usage). This page is the short path after the binaries exist.

## Install the stack

Prefer the method for your distro in the [README](README.md#quick-install). A packaged or `./install.sh` install should give you:

* `start-simd`, `start-cargopit`, `test-cargopit`, `cargopit-tui` in `~/.local/bin`
* configs in `~/.config/simd/` and `~/.config/cargopit/`
* optional user unit `~/.config/systemd/user/simd.service`

To compile by hand instead:

* build [cargopit](https://github.com/M4X1K02/cargopit) — `git submodule update --init --recursive`, then `cmake` / `make` (needs cargo for `cargopit-tui`)
* build [simd](https://github.com/Spacefreak18/simapi/tree/master/simd) (needs simapi installed first, including `simdata.h`)
* get [simshmbridge](https://github.com/spacefreak18/simshmbridge) compatibility EXEs ([releases](https://github.com/spacefreak18/simshmbridge/releases)) unless you only use UDP titles

## Configure SIMD & Cargopit

Use `cargopit-tui` for device lists, play/test flags, simd.config, Lua scripts, tachometer calibration, and tyre diameters. A read-only raw view of `cargopit.config` is on the Settings tab.

* `~/.config/simd/simd.config` — [example](https://github.com/Spacefreak18/simapi/blob/master/simd/conf/simd.config) (editable in the TUI)
* `~/.config/cargopit/cargopit.config` — start from an empty profile in the TUI, or the installer stub / [conf/cargopit.config](https://github.com/M4X1K02/cargopit/blob/master/conf/cargopit.config)
    * Keep only devices you have plugged in (or disable unused rows in the TUI)
    * [Bass shaker config](https://spacefreak18.github.io/simapi/shakers)
    * Test with `test-cargopit`, `cargopit test -vv`, or **t** in the TUI. Press **t** again (or **q** / Ctrl+C in the CLI) to stop a running test.

Serial/HID devices often need your user in `input`, `dialout`, and/or `uucp`, plus the udev rules from `udev/69-cargopit.rules`. The TUI Diagnostics page reports groups, udev, binaries, and `SIMAPI.DAT`.

### External processing (EQ, limiter, rig correction)

Cargopit generates the haptic signal. Room and rig correction, EQ, compression, and limiting belong to a PipeWire processor that you choose. In the TUI, set a Sound device's **Device id** to that processor's sink. Processor sinks are listed after hardware sinks and tagged `[processor]`.

* **Easy Effects** or **Carla**: start the tool, then pick its sink.
* **Shipped preset**: `conf/pipewire/cargopit-tactile.conf` (installed under `share/cargopit/conf/pipewire/`) is a filter-chain sink named `cargopit_tactile`. It carries the seat correction that used to be built into cargopit, plus a 10 Hz high-pass, a 120 Hz low-pass, and a sample clamp. To use it, copy it to `~/.config/pipewire/pipewire.conf.d/`, set `target.object` to your amplifier's sink (`pactl list short sinks`), and run `systemctl --user restart pipewire`. It needs PipeWire 1.0 or newer.
* **Your own rig**: measure a seat sweep as `frequency_hz,transfer_db` CSV, then run `tools/haptics/fr_to_filterchain.py sweep.csv --target-sink <amp sink> > cargopit-tactile.conf` from a source checkout. `--help` lists the target, cut, filter, and channel options.

Each stream is a PipeWire node named `cargopit.<Effect>` or `cargopit.<Effect>.<Tyre>` (for example `cargopit.TyreSlip.FrontLeft`), with `cargopit.effect` and `cargopit.tyre` properties for qpwgraph, Carla, or WirePlumber rules.

If the chosen sink is missing, cargopit logs `could not connect sound stream` and skips that device. If the sink disappears during play, the stream goes silent rather than moving to your speakers. Once the processor is back, use **Apply** in the TUI or `echo reload | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/cargopit.sock` to reconnect. External tools can add gain, so keep a limiter or gain cap on the amplifier as well.

## Steam & Game Config

### Steam

Shared-memory titles (Assetto Corsa, ACC, AMS2, PCars2, …) need a simshmbridge EXE in the **same Proton prefix** as the game. Set a launch option such as:

```bash
SIMD_BRIDGE_EXE=/home/YOU/.local/share/cargopit/simshmbridge/assets/acbridge.exe %command%
```

Exact EXE names and more examples: [simd usage](https://spacefreak18.github.io/simapi/simd_usage) and [simshmbridge](https://github.com/spacefreak18/simshmbridge?tab=readme-ov-file#basic-mapping-examples).

### Game specific settings

#### Automobilista 2 (AMS2)

Activate Shared Memory and set the protocol to Project CARS 2, then restart the game.

![System Settings in AMS2](https://static.wixstatic.com/media/910f3b_adabfa94a57944cca33e488972534fdd~mv2.png/v1/fill/w_964,h_374,al_c,q_90,usm_0.66_1.00_0.01,enc_avif,quality_auto/game_setup_ams2_1.png)
<img src="https://docs.simucube.com/Tuner/games/assets/automobilista2_telemetry_2.png" alt="Shared Memory Settings in AMS2" width="65%">

#### Assetto Corsa & Assetto Corsa Competizione (ACC)

No extra in-game telemetry toggle. You still need the AC/ACC bridge EXE in the Steam launch command; cargopit starts simd.

## Run

Start a session with `start-cargopit`, `cargopit play`, or `cargopit-tui`. Cargopit starts simd itself when it is not already running. Launch the game from Steam as usual.

If simd is not installed, that is the one case that needs a human: install simd and try again.

Shared-memory titles still need the bridge EXE in the Steam launch command (`SIMD_BRIDGE_EXE=... %command%`). The installer can also enable `simd.service` so mapping is ready at login. If the simd binary was installed under `/usr/local`, the launcher already sets `LD_LIBRARY_PATH`.

## Troubleshooting

* simd should log that it found the sim after you are in session
* `hexdump /dev/shm/SIMAPI.DAT | head` should not be all zeros once mapping works
* AC/ACC: `hexdump /dev/shm/acpmf_physics | head` — non-zero while on track
* AMS2: look for `/dev/shm/$pcars2$`
* confirm the bridge EXE is actually running (`ps aux | grep -i bridge`)
* if simd sees the game but cargopit shows no RPM/gear, re-check [game settings](#steam--game-config)
* `cargopit-tui` can start/stop the two processes, edit devices, and show logs if `~/.local/bin` is on `PATH`
