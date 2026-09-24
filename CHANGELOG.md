# Changelog

## [Unreleased]

### Added

- Sound streams carry `node.name = cargopit.<Effect>[.<Tyre>]` plus
  `cargopit.effect` / `cargopit.tyre` properties, so Easy Effects, Carla,
  PipeWire filter-chain, or WirePlumber rules can pick them up.
- `tools/haptics/fr_to_filterchain.py` turns a seat sweep CSV into a
  PipeWire filter-chain sink with fitted peaking cuts, a subsonic high-pass,
  a band-top low-pass, and a clamp. Sweeps and presets stay in the user's
  config.
- The TUI lists hardware sinks before processor sinks, tags processors, and
  shows the selected sink's full name in the field help.

### Changed

- The built-in seat equalization is gone from cargopit. It was one rig's
  measurement compiled into every build. Generate a preset from your own
  sweep, or use another processor, to correct your seat.

### Fixed

- A sound device whose sink is missing is now reported and skipped. Before,
  cargopit waited on the stream forever or played through the default sink.
- Sound streams no longer move to the default sink when their target sink
  (for example an external processor) disappears. They go silent until
  `reload`.

## [0.4.0] - 2026-09-23

Cargopit is the first release of this independent fork of monocoque. The
original GPL attribution remains in the source and package copyright files.

### Added

- Ratatui terminal manager for starting, testing, restarting, and stopping the
  telemetry stack.
- Shared device profiles with live play profiles, device templates, speaker
  masks, telemetry, diagnostics, and a ready health gauge.
- Per-device worker threads and per-device haptic state.
- USB shaker engine rumble, seat equalization, rolling and slip gates, and
  PipeWire-compatible stream volume controls.
- Higher telemetry and device refresh-rate choices up to 500 FPS.
- C static analysis and legacy unsafe-API checks in CI.
- Version reporting from both `cargopit` and `cargopit-tui`.
- Release packages for Debian/Ubuntu, Fedora, AppImage, and Flatpak.

### Changed

- Renamed the user-facing application and package identity from monocoque to
  cargopit.
- Made the installer, TUI, control socket, and package workflows follow the
  fork's repository and application identity.
- Pinned the simulator API submodule used by this fork, including its DiRT
  Rally 2 telemetry mapping.

### Fixed

- Improved startup and shutdown handling for the TUI and telemetry processes.
- Prevented stale simulator shared memory and ACR helper processes from being
  treated as a live session.
- Hardened serial, PulseAudio, installer, and package error paths.
- Fixed package builds so release artifacts contain the tagged source tree and
  carry the release version.

### Requirements

- Linux with GCC 13 or newer for the current simapi headers.
- Rust 1.88 or newer when building `cargopit-tui`.
- Supported simulator bridges and hardware still require the setup described
  in the simapi documentation.

[0.4.0]: https://github.com/M4X1K02/cargopit/releases/tag/0.4.0
