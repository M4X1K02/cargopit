# TUI device management plan

Implemented by `cargopit-tui` (see `tui/`). This write-up remains as the schema and gap list the TUI is built against.

Goal: **every setting a user currently has to edit outside the TUI becomes editable inside `cargopit-tui`**, with device management that goes beyond a single add/edit form.

## Current TUI

`cargopit-tui` is the Ratatui 0.29 app in `tui/`. The NAppGUI app and the Python curses `cargopit-manager` are gone. Four tabs plus overlay screens:

| Screen | What it does |
| --- | --- |
| Dashboard | simd / cargopit / `SIMAPI.DAT` / configured-vs-present; Start / Test / Restart / Stop |
| Devices | Switch / add / duplicate / delete `configs[]`; list devices; add / edit / delete / enable / reorder; templates; Enter opens offline tune |
| Settings | Play flags, simd.config, Lua templates, tach XML, tyre diameters, diagnostics, raw config view |
| Logs | Tail `~/.cache/cargopit/*.log` plus child stdout/stderr, filter play/test/file |
| Device editor / tune | Schema-driven field list (`tui/src/form.rs`); save rewrites `cargopit.config` through the Rust libconfig subset |

Almost every screen is a `List` + `ListState` rebuilt each frame. That is enough to ship, but it is the main place official Ratatui examples still have something to teach (see [Ratatui examples](#ratatui-examples)).

The remaining product gaps are schema completeness, live discovery refresh, live tune IPC, and widget fit — not “open `$EDITOR` for simd / Lua / flags”.

## Configuration universe

Anything in this list that remains “open `$EDITOR`” is a gap.

### 1. `cargopit.config`

Path: `$XDG_CONFIG_HOME/cargopit/cargopit.config` (today hardcoded in `tui/src/paths.rs`).

Top-level `configs` array. Each entry:

| Key | Role | TUI today |
| --- | --- | --- |
| `sim` | Title (`default`, `ac`, `acc`, `ace`, `ams2`, `et`, `at`, `rf2`, …) | Display / switch only |
| `car` | Car filter (`default` / `all` / name) | Display / switch only |
| `api` | Optional API tag | Display only |
| `devices` | Device list | Add / edit / delete |
| unknown keys | Preserved in `SimConfig.extra` | Not shown, not editable |

C selects the matching `configs[]` entry at play time (`getconfigtouse*` in `src/cargopit/helper/confighelper.c`). Multiple profiles are the intended way to have different device sets per sim/car. The TUI can **switch** them but cannot **create, rename, copy, or delete** them.

### 2. Device keys the C loader reads

`devsetup()` / `save_device_config()` in `confighelper.c` plus `DeviceSettings` in `confighelper.h`. The TUI form does **not** cover the full set.

| Key | Used by | TUI field? |
| --- | --- | --- |
| `device` | Class: USB / Sound / Serial | Yes |
| `type` | Tachometer, Haptic, Wheel, ShiftLights, SimWind, Simleds, Custom, … | Yes |
| `subtype` | Hardware: Revburner, CSLELITEV3PEDALS, LogitechG29, MozaR5, … | USB only |
| `devid` | Pulse sink name or USB `vid:pid` | Via “Device” identity |
| `devpath` | Serial node or sysfs rumble path | Via “Device” identity |
| `enabled` | Skip device at load | Yes |
| `fps` | Per-device refresh | Yes |
| `config` | Lua script or Revburner XML | Tachometer only |
| `effect` / `tyre` / `modulation` | Haptic mapping | Yes, if haptic |
| `frequency` / `frequencyMax` / `amplitude` / `amplitudeMax` | Haptic | Yes, if haptic |
| `threshold` / `duration` | Haptic | Yes, if haptic |
| `motors` | Serial haptic motor mask (`MotorPosition`) | Serial haptic only |
| `baud` | Serial | Serial |
| `fanpower` | SimWind | SimWind only |
| `ampfactor` | Serial gain | **No** (defaulted on new Serial, never shown) |
| `numleds` / `startled` / `endled` | Simleds | Simleds only |
| `numlights` | ShiftLights | **No** |
| `granularity` | Tachometer (`1`, `2`, `4` — not `3`) | Tachometer |
| `volume` / `streamVolume` | Sound; C prefers `streamVolume`, falls back to `volume`, clamp `0..100` | Volume (nudge writes both) |
| `pan` / `channels` / `noise` | Sound | Yes |
| `value0` / `value1` | Present in `conf/cargopit.config` CSL Elite examples | **No**, and **C never reads them** (colours are hardcoded in `cslelitev3.c`) |

USB hardware list in `USB_HARDWARE_SUBTYPES` is also incomplete versus `strtodevsubsubtype()`: missing `SIMAGICGTNEO` and the `MozaR9` alias (`MozaNew`).

### 3. Sidecar files referenced by devices

| File | Who consumes it | TUI today |
| --- | --- | --- |
| Lua (`conf/*.lua`, user copies under `~/.config/cargopit/`) | Serial Simleds / Custom (`config = "..."`) | Path field missing except on tach |
| Revburner XML | USB Tachometer | Path string only; no calibration wizard |
| Tyre-diameter libconfig (`cars` list: `car`, `sim`, `tyre0..3`) | Haptic slip when the sim does not supply diameters (`loadtyreconfig` / `savetyreconfig`) | None |

### 4. simd

`~/.config/simd/simd.config` is created by the installer and is **required** for play. HOW-TO-USE currently says “usually fine as-is”, which is exactly the kind of file that must still be inspectable and editable in the TUI.

Optional: user unit `~/.config/systemd/user/simd.service` (enable / disable / status). Do not invent a second process supervisor; Dashboard already starts simd via `cargopit play`.

### 5. Runtime flags the C CLI already has

`cargopit play` / `test` accept verbosity, `--disable_audio`, `--udp`, `--fps`, `--config-file`, `--log`. The TUI always spawns bare `play` / `test`. Those flags are configuration. They belong in a Settings page and should be passed through on spawn.

### 6. Calibration utilities

`cargopit config tachometer -m <max_revs> -g <granularity> -s <xml>` is a first-class C action. The TUI has no path to it.

## Design principles

1. **TUI is the only config UI.** No `$EDITOR` fallback, no “edit file” action as the primary path. Raw file view is allowed as a *read-only* diagnostics pane.
2. **C remains runtime truth.** The TUI must not invent keys the loader ignores, and must not drop keys the loader needs. `value0` / `value1` are the cautionary example: they look configurable in the sample file and are dead.
3. **Schema-driven forms.** One field catalog (Rust constants + a `DeviceSchema`) drives visibility, labels, combo choices, validation, and tests. Do not grow `visible_fields()` with more ad-hoc `if class && type` trees.
4. **Exceptions first.** Refuse save on parse failure, missing required sidecar, invalid enum, or `frequencyMax < frequency` with Frequency modulation (C already falls back and warns).
5. **Preserve unknown keys** on a device when editing known fields. Replacing the whole `DeviceEntry` on class change (`form.rs` `cycle_class`) already wipes them; keep identity *and* any keys the new class still understands.
6. **No magic strings.** Extend `tui/src/consts.rs` (and C enums) rather than scattering `"Simleds"` / `115200`.
7. **Do not nest UI handlers more than three levels.** Split screens into modules (`dashboard`, `devices`, `settings`, `tune`) instead of growing `app.rs`.

## Ratatui examples

Catalog: [ratatui.rs/examples/apps](https://ratatui.rs/examples/apps/) (Ratatui `0.30.2` sources). This crate pins `ratatui = "0.29"` (`tui/Cargo.toml`). Steal **layout, state, and widget choice**; do not paste 0.30 APIs (`ratatui::run`, `area.layout(&…)`) without a version bump.

`cargopit-tui` already matches the skeleton of **Demo2** + **Hello World** + **Todo List**: header, `Tabs`, body, footer hints, `List` selection, `Clear` confirm popup, poll timeout. The useful leftovers are **Table**, **Scrollbar**, **User Input**, **Panic**, **Async GitHub**, and a **Gauge** on the tune page.

### App examples → our screens

| Example | What it demonstrates | Apply to cargopit |
| --- | --- | --- |
| [Demo2](https://ratatui.rs/examples/apps/demo2/) | `Tab` enum, one module per tab, `Widget for &App`, title bar + tabs + footer key chips, `Clear` overlay | **Already the IA.** Next: split `tui/src/ui.rs` / `app.rs` per screen (`dashboard`, `devices`, `settings`, `logs`, `form`) so handlers stay ≤3 nest levels. Do **not** copy RGB swatch, traceroute map, or destroy animation. |
| [Demo](https://ratatui.rs/examples/apps/demo/) | Kitchen-sink widgets on one screen; backend feature flags | Keep **crossterm only**. Do not put charts, gauges, and lists on Dashboard. |
| [Table](https://ratatui.rs/examples/apps/table/) | `Table` + `TableState` + column constraints + `Scrollbar` + footer | **Devices** rows: enabled, `class / type / subtype`, identity, presence. **Tyres** rows: sim, car, four diameters. Store `TableState` on `App`, do not rebuild selection only as an index into a `List`. |
| [Todo List](https://ratatui.rs/examples/apps/todo-list/) | Master–detail: list + selected-item pane; Enter toggles status; `☐`/`✓` | **Devices**: list (or table) on top, detail pane with summary + actions. Space already toggles `enabled` — keep that as the todo-status analogue. **Settings → Lua / templates**: name list + description pane. |
| [User Input](https://ratatui.rs/examples/apps/user_input/) | `InputMode::{Normal, Editing}`, cursor, backspace; docs point at `tui-input` / `ratatui-textarea` | **Device editor** today appends `█` onto `edit_buffer`. Use Normal/Editing (vim-style: Enter to type, Esc to leave) and `tui-input` so unicode paths (`/dev/simdev*`, Pulse names) do not break. Multi-line Lua stays a **copy-from-template** action, not a full editor; if we grow one, `ratatui-textarea`. |
| [Async GitHub](https://ratatui.rs/examples/apps/async-github/) | `tokio` draw/event `select!`, `Arc<RwLock<State>>`, `LoadingState::{Idle,Loading,Loaded,Error}` | **Hardware discovery** (`pactl`, hidraw, serial) and **process poll** must not stall the 200 ms UI tick. A background task + `LoadingState` on the Devices presence column is the pattern. Do not take a tokio dependency just for GitHub; `std::thread` + the existing child `mpsc` is enough. |
| [Gauge](https://ratatui.rs/examples/apps/gauge/) | `Gauge` / `LineGauge` with percent vs ratio labels | **Tune** (offline): `streamVolume` 0..100, `fanpower`, amplitude as gauges while h/l nudges. **Dashboard** stays binary RUNNING/STOPPED, not a fake progress bar. |
| [Chart](https://ratatui.rs/examples/apps/chart/) | Animated line / bar / scatter | **Skip for v1.** Matches the non-goal “live haptic graphs or audio meters”. Revisit only after live-tune IPC exists. |
| [Inline](https://ratatui.rs/examples/apps/inline/) | `Viewport::Inline`, worker threads, `LineGauge`, `insert_before` finished lines | Keep the **full-screen** manager. Steal the **event enum** (`Input` / `Tick` / `ChildUpdate` / `DiscoveryDone`) instead of mixing child stdout into `tick()`. Do not drop into an inline viewport to spawn `cargopit test`. |
| [Tracing](https://ratatui.rs/examples/apps/tracing/) | `tracing` to a file; mentions `tui-logger` | TUI debug (`handle_key`, save, spawn) → `~/.cache/cargopit/tui.log`. The Logs **tab** stays a file tail of play/test, not an in-app tracer. `tui-logger` is optional later, not a replacement for `LogState`. |
| [Panic](https://ratatui.rs/examples/apps/panic/) | `ratatui::init` panic hook + `color_eyre` so raw mode cannot stick | **`tui/src/main.rs` today** only `restore()` on a normal `Result`. A panic leaves the terminal raw. Install a hook (or bump to `ratatui::run`) and `color_eyre` before adding more screens. |
| [Advanced Widget](https://ratatui.rs/examples/apps/advanced-widget-impl/) | `Widget` on value vs `&T` vs `&mut T`; `WidgetRef` for boxed children | Schema fields as small widgets (`Combo`, `Numeric`, `Identity`, `Toggle`) driven by `DeviceSchema`, not more `if class && type` in `draw_form`. |
| [Hyperlink](https://ratatui.rs/examples/apps/hyperlink/) | OSC 8 clickable URLs | Diagnostics / footer: simapi docs (`simd_usage`, serial Lua, third-party devices). Best-effort; terminals without OSC 8 still show the URL text. |
| [Calendar Explorer](https://ratatui.rs/examples/apps/calendar-explorer/) | `Monthly` calendar | **Skip.** No date-based config. |
| [Hello World](https://ratatui.rs/examples/apps/hello_world/) / [Minimal](https://ratatui.rs/examples/apps/minimal/) | `init`/`restore`, poll timeout | Already the event loop. Prefer their panic-safe teardown over more custom `setup()`. |

Widget-level pages that match the same gaps: [Tabs](https://ratatui.rs/examples/widgets/tabs/), [List](https://ratatui.rs/examples/widgets/list/), [Table](https://ratatui.rs/examples/widgets/table/), [Scrollbar](https://ratatui.rs/examples/widgets/scrollbar/), [Gauge](https://ratatui.rs/examples/widgets/gauge/).

### Third-party crates (only if a screen needs them)

From [awesome-ratatui](https://github.com/ratatui/awesome-ratatui): `tui-input` for one-line identity/path fields; `ratatui-textarea` only if we edit Lua in-place; `ratatui-explorer` if the tach XML / Lua picker outgrows cycling `Discovery` choices. Do not pull `rat-widget` as a second UI framework.

### What to do next (UI only)

Order that leaves the TUI usable after each step (implemented in `tui/`):

1. Panic hook via `ratatui::init` / `restore` in `main.rs` (Panic / Hello World) — **done**. `color_eyre` is omitted because it pulls crates that need a newer rustc than distro `apt` cargo.
2. Devices (and tyres) as `Table` + `Scrollbar`; keep `device_index` as the selected row (Table Demo + Todo List detail pane) — **done**.
3. Form `InputMode` + `tui-input` for free-text fields (User Input) — **done**.
4. Background discovery with `LoadingState` on the presence column (Async GitHub, without tokio unless we already want it) — **done**.
5. Offline tune gauges for volume / amplitude / fanpower (Gauge). Charts stay out — **done**.

## Target information architecture

These four tabs are implemented. Remaining work is widget fit ([Ratatui examples](#ratatui-examples)) and the schema/discovery gaps above, not a new information architecture.

```
[ Dashboard ] [ Devices ] [ Settings ] [ Logs ]
```

### Dashboard (keep, then extend)

Keep process control. Add a compact “configured vs present” line (N connected / M missing) once discovery exists. Settings that affect spawn (verbosity, disable audio) live on Settings, not here.

### Devices

Split the page:

1. **Profile bar** — current `sim / car / api`; actions: previous/next, add, duplicate, edit metadata, delete (with confirm).
2. **Device list** — one row per device with:
   - enabled marker
   - summary (`class - type - subtype/effect`)
   - identity (`devid` / `devpath`)
   - presence (`connected` / `missing` / `unknown`)
3. **Actions** — add, edit, duplicate, delete, toggle enabled, move up/down, apply template, Enter for live tune.

Do not require opening the editor to disable a device. `enabled` is already in the file format.

### Settings (new)

Everything that is not a device:

- Path to `cargopit.config` (default XDG; override persisted in TUI state)
- Path to `simd.config`; structured editor for that file
- Play/test flags: verbosity (`-v` / `-vv`), `disable_audio`, `udp`, global fps, log file
- Lua library: list scripts under the config dir + bundled `conf/*.lua`; open a constrained editor or copy-from-template
- Tachometer calibration: max revs, granularity, output XML path; spawn `cargopit config tachometer`
- Tyre-diameter store: list/add/edit `cars[]` entries
- Diagnostics: groups (`input`, `dialout`, `uucp`), udev rule present, simd binary found, `SIMAPI.DAT` size/non-zero

### Logs (keep)

Filter by source (`play` / `test` / file). Optional: jump from a device-init warning to that device’s editor.

## Device editor

Replace the flat field list with a **wizard that still fits one screen**: class → type → hardware → identity → type-specific pane.

### Field catalog (source of truth)

Implement as data, not as a second copy of `visible_fields()`. Sketch:

```text
class USB:
  types: Tachometer | Haptic | Wheel
  identity: hid vid:pid and/or sysfs rumble path (not serial tty nodes)
  Tachometer: subtype Revburner, granularity in {1,2,4}, config XML required
  Haptic/Wheel: subtype from USB_HARDWARE_SUBTYPES + SIMAGICGTNEO + MozaR9,
                haptic block, optional rumble path glob

class Sound:
  types: Haptic
  identity: Pulse/PipeWire sink (name + description)
  volume via streamVolume (0..100), pan, channels, noise, haptic block

class Serial:
  types: ShiftLights | SimWind | Haptic | Wheel | Simleds | Custom
  identity: serial port (ttyUSB/ttyACM/ttyS plus udev symlink /dev/simdev*)
  always: baud, ampfactor
  SimWind: fanpower
  Haptic: motors (MotorPosition enum, not a raw unexplained int), haptic block
  ShiftLights: numlights
  Simleds: numleds, startled, endled, config Lua
  Custom: config Lua required
```

Haptic block (USB haptic, USB wheel, Sound, Serial haptic): effect, modulation, tyre, frequency, frequencyMax, amplitude, amplitudeMax, threshold, duration.

Identity cycling today is wrong for USB: `device_choices_for_class` returns **serial ports** for both Serial and USB (`tui/src/hardware.rs`). USB must enumerate HID (`hidapi` / sysfs) and known rumble globs (see `cslelitev3.c` `SYSFSRUMBLEPATH`). Serial must also include installer udev names (`/dev/simdev*`), not only `ttyUSB` / `ttyACM` / `ttyS`.

Sound discovery should show Pulse **descriptions**, not only sink names (`pactl list short sinks` is opaque). Prefer `pactl list sinks` or a small Pulse helper; keep `pactl` as the no-new-crate path.

### Validation before write

Fail closed:

- libconfig parse of the *would-be* file
- required identity non-empty
- tach XML / Lua path exists after tilde expand
- granularity in `{1, 2, 4}`
- volume in `[0, 100]`
- Frequency modulation requires `frequencyMax > frequency` (match C)
- `type` legal for `device` class (the C `strtodevsubtype` switch has a fall-through bug USB→Serial; do not copy that)

Show the C-side error text when `cargopit test` is run from the form (`t` already exists; wire it to test **after save**, and later to a single-device test if the C CLI gains `--device-index`).

### Save behaviour

- Atomic write (temp file + rename) so a crash does not truncate `cargopit.config`.
- Keep unknown device keys.
- Comments will still be lost (the Rust renderer does not round-trip comments). Accept that for v1; do not try to be a comment-preserving libconfig pretty-printer until the schema is complete. If users rely on comments, the Settings “raw view” shows the last on-disk file *before* save.
- After save, if play is running, prompt “Restart to apply?” rather than silently diverging from the live process.

### Templates

The sample `conf/cargopit.config` is a kitchen sink. New users should not copy it. Offer named templates that insert *sets* of devices:

- Sound: Engine + Gear on one sink
- Sound: four-corner TyreSlip and/or TyreLock
- Serial haptic: slip / lock / ABS motor map
- USB CSL Elite V3: ABS / Slip / Lock (after presence check)
- Serial Simleds with a bundled Lua (`basic_rpms.lua`, `rpms_and_flags.lua`, …)

Templates only insert; they never wipe the profile without confirm.

## Live tuning (the stub)

`Screen::DeviceTune` exists as a slot (`TUNING_STUB_MESSAGE`). Fill it in two steps so the page is useful before IPC exists.

**Step A — offline tune (no new C API):** the tune page is the same schema as the editor, focused on haptic/sound numbers, with larger nudges and a live preview of the *saved* values. Save still goes to disk. “Apply” restarts play if it was running.

**Step B — live apply:** only after the C loop can reload one `DeviceSettings` without tearing down every device. Do **not** parse `cargopit.config` from the 60 fps thread. Preferred shape:

- Unix socket or a documented command pipe owned by the play process
- TUI sends a single device replacement (`confignum`, `devicenum`, fields)
- Loop applies on the next tick for that device only

Until that socket exists, do not fake live tuning by killing and restarting on every keystroke.

Per-device **test** from the old NAppGUI window (synthetic `SimData`, one device) is worth restoring as `t` on the editor. That needs a C CLI flag (e.g. `cargopit test --config-index N --device-index M`) rather than the TUI reimplementing the game loop.

## simd.config

Treat it as a second libconfig document with its own schema module (`tui/src/simd_config.rs`), not a generic key dump.

- Load/save with the same parser.
- Unknown keys preserved.
- If the example from simapi is the usual “leave it”, still show every key with a one-line description sourced from comments in that example (copy the comment text into constants; do not scrape GitHub at runtime).
- Missing file: offer “create from installer stub / simapi example” instead of an empty document.

Do not start a second simd from Settings if Dashboard/play already owns that.

## Implementation slices

Ship in PRs that each leave the TUI usable. Do not wait for live IPC.

1. **Schema + missing fields + USB discovery fix**
   - Catalog in `consts` / new `schema.rs`
   - `ampfactor`, `numlights`, Lua `config` on Simleds/Custom, `SIMAGICGTNEO`, `MozaR9`
   - USB identity ≠ serial ports
   - Validation + atomic save
   - Tests: parse `conf/cargopit.config`; round-trip every catalog key; reject bad granularity / volume

2. **Device list operations**
   - Toggle enabled, duplicate, reorder
   - Presence column
   - Templates
   - Profile add / edit / delete / duplicate

3. **Settings tab**
   - Play flags persisted (XDG state file, not stuffed into `cargopit.config`)
   - simd.config editor
   - Lua picker + copy bundled templates into the user config dir
   - Tachometer calibration form that shells out to the existing C action
   - Tyre-diameter file editor

4. **Diagnostics**
   - Groups, udev, binary paths, `SIMAPI.DAT`
   - Configured-vs-present on Dashboard

5. **Tune page step A**, then **C CLI single-device test**, then **live reload IPC** if still wanted

Slice 1 is the minimum that makes “I can configure every device key without nano” true. Slice 3 is the minimum that makes “all configuration” true (simd, lua, tach, diameters, flags).

## Testing

- Keep existing `tui` unit tests (`parse_cargopit`, round-trip, libconfig comments/semicolons).
- Add a table: for each `FieldId` / catalog entry, a fixture writes the key and `confighelper` C tests (or a `cargopit test --dry-parse` if added) accept it.
- Do **not** put hardware-interactive tests in `tests/`; discovery helpers should accept injected lists (serial/hid/pulse) so unit tests stay offline.
- After Settings spawn-flag work, a test that `process::spawn_play` argument vector contains the selected flags (mock `Command`).
- Manual: add a Sound device from an empty profile, save, `cargopit test --disable_audio` still loads the rest.

## Non-goals

- Rewriting the C game loop in Rust.
- Comment-preserving pretty-print of libconfig in slice 1.
- Editing udev rules or Steam launch options from the TUI (show them in diagnostics; changing Proton prefixes is out of band).
- Reintroducing NAppGUI.
- Live haptic graphs or audio meters.

## Files likely to change (when implementing)

| Area | Files |
| --- | --- |
| Schema / form | `tui/src/consts.rs`, new `tui/src/schema.rs`, `tui/src/form.rs` |
| Discovery | `tui/src/hardware.rs` |
| Screens | `tui/src/app.rs`, `tui/src/ui.rs` (split by tab) |
| Config IO | `tui/src/config.rs`, `tui/src/libconfig.rs`, `tui/src/paths.rs` |
| Spawn flags | `tui/src/process.rs` |
| Single-device test / reload | `src/cargopit/helper/parameters.c`, `src/cargopit/cargopit-cli.c`, `src/cargopit/gameloop/` |
| Docs | `HOW-TO-USE.md` (TUI can edit config; stop implying hand-edits are required) |

C `save_device_config()` is **not** the TUI write path today (the TUI writes the whole file itself). If live reload needs a shared writer, extract one C library or keep Rust as the writer and teach C only to *read*. Do not maintain two incomplete writers (`save_device_config` currently skips subtype for USB, motors, lua path, granularity, `numlights`, `devpath` vs `devid` correctly in all cases). Prefer fixing or deleting the C writer if nothing else calls it after NAppGUI removal.
