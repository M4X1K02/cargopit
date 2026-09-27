# Known quirks preserved from the C host

These behaviours matched the C host during the Rust port. They are not
accidental. Fix them in separate changes after the cutover, not by editing
parity goldens.

- Logitech G29 trailing report byte and an unreachable LED threshold stay as
  captured.
- Serial Lua `led_clear_range` is registered to the same implementation as
  `led_clear_all`.
- Moza-new LED state is process-global.
- HID open still uses `hid_open(vid, pid, NULL)` and the first matching
  device. `devid` is ignored for HID path selection.
- Integer device intervals use `1000 / fps` milliseconds (16 ms at 60 fps),
  not a fractional 16.667 ms period.
- Runtime config lookups stay strict about numeric types: an integer does not
  satisfy a float key, and a float does not satisfy an integer key. Missing
  keys keep C defaults.
