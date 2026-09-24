# Protocol references

These notes record independent protocol sources reviewed while the parity
harness was added. They validate captures. They do not override C golden
output. If a reference disagrees with a golden, keep the C behaviour and add
the disagreement to `docs/rust-port-known-quirks.md` after cutover.

Reviewed: 2026-09-24. Wire agreement is "pending maintainer capture review":
phase 1 goldens are the C contract, not a claim that each public reference
matches every byte.

Only protocol facts may be reimplemented. Do not copy code from GPL-2.0-only
kernel drivers or unlicensed projects into this GPL-3.0-or-later tree.

| Peripheral | Source | License | Claimed wire format | Golden agreement |
| --- | --- | --- | --- | --- |
| Moza R5/R3/R8, R9, KS Pro | [Boxflat](https://github.com/Lawstorant/boxflat) | GPL-3.0 (repository license) | Serial LED frames, byte stuffing, checksum, arm sequence, color tables | Pending maintainer review of `moza_*` goldens. The R5 path writes only when `SerialDevice.port` is set; the capture keeps that C behaviour |
| Moza | [moza-simhub-plugin](https://github.com/giantorth/moza-simhub-plugin) | No license file on the default branch at review | RPM and flag payloads used by SimHub | Pending; do not copy sources |
| Fanatec CSL Elite V3 | [hid-fanatecff](https://github.com/gotzl/hid-fanatecff) | GPL-2.0-only | hid-fanatec sysfs `rumble` integer for slip, lock, and ABS | Pending `csl_elite_v3` goldens. Do not copy driver code |
| Logitech G29 | Linux `hid-lg4ff` | GPL-2.0-only (kernel) | HID RPM LED report | Pending `logitech_g29` goldens. Do not copy driver code |
| Logitech G29 | [new-lg4ff](https://github.com/berarma/new-lg4ff) | GPL-2.0-only | Alternate G29 LED report layout | Pending. Do not copy driver code |
| Cammus C5/C12 | [cammus-simhub-plugin](https://github.com/giantorth/cammus-simhub-plugin) | Derived from monocoque; treat as GPL-3.0-or-later | HID LED reports, including Lua-driven C12 reports | Pending `cammus_*` goldens |
| Cammus | [Boxflat issue 9](https://github.com/Lawstorant/boxflat/issues/9) | Discussion, not a code license | Report layout notes | Pending |
| Thrustmaster rims | [thrustmaster-led-linux](https://github.com/wKoja/thrustmaster-led-linux) | No license file found at review | HID LED reports for T300-class rims | Not a current device. Do not copy sources |
| Thrustmaster | [hid-tmff2](https://github.com/Kimplul/hid-tmff2) | GPL-2.0-only | Kernel force-feedback and LED interface | Not a current device. Do not copy driver code |
| Simagic P1000 and GT Neo | none | n/a | No complete public reverse engineering was found for the P1000 haptic reports or the GT Neo LED feature reports | C goldens are the only contract |

Arduino ShiftLights, SimWind, SerialHaptic, and SimLED use the packed structs
in `src/arduino/`. Those sketches are part of this repository and are the
wire reference for the serial goldens.
