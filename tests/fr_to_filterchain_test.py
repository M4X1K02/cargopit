#!/usr/bin/env python3
"""Checks tools/haptics/fr_to_filterchain.py against a synthetic seat sweep.

Real sweeps belong to one rig and stay in the user's config, so the test
builds its own: a flat seat with one resonance peak.
"""
import cmath
import importlib.util
import math
import sys
import tempfile
import unittest
from pathlib import Path

TOOL = Path(__file__).resolve().parent.parent / "tools" / "haptics" / "fr_to_filterchain.py"

SWEEP_START_HZ = 10.0
SWEEP_END_HZ = 190.0
SWEEP_POINTS = 96
FLAT_DB = 3.0
PEAK_HZ = 45.0
PEAK_DB = 12.0
PEAK_SIGMA_HZ = 4.0
GAUSSIAN_EXP_SCALE = 0.5
RATE = 48000.0
CUTOFF_DB = -3.01
CUTOFF_TOLERANCE_DB = 0.05
FIT_TOLERANCE_DB = 1.5
TARGET_SINK = "alsa_output.test_amp"

spec = importlib.util.spec_from_file_location("fr_to_filterchain", TOOL)
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)


def sweep_rows(peak_db=PEAK_DB):
    ratio = (SWEEP_END_HZ / SWEEP_START_HZ) ** (1.0 / (SWEEP_POINTS - 1))
    rows = []
    for i in range(SWEEP_POINTS):
        hz = SWEEP_START_HZ * ratio ** i
        x = (hz - PEAK_HZ) / PEAK_SIGMA_HZ
        rows.append((hz, FLAT_DB + peak_db * math.exp(-GAUSSIAN_EXP_SCALE * x * x)))
    return rows


def write_sweep(directory, rows):
    path = Path(directory) / "sweep.csv"
    lines = [f"{gen.CSV_FREQUENCY},{gen.CSV_TRANSFER}"]
    lines += [f"{hz:.6f},{db:.6f}" for hz, db in rows]
    path.write_text("\n".join(lines) + "\n")
    return path


def biquad_db(coeffs, hz, rate):
    b0, b1, b2, a0, a1, a2 = coeffs
    z = cmath.exp(-1j * 2.0 * math.pi * hz / rate)
    return 20.0 * math.log10(abs((b0 + b1 * z + b2 * z * z) / (a0 + a1 * z + a2 * z * z)))


class FilterChainGeneratorTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.sweep = write_sweep(self.tmp.name, sweep_rows())

    def tearDown(self):
        self.tmp.cleanup()

    def fit(self, *extra):
        args = gen.parse_args([str(self.sweep), *extra])
        freqs, transfer = gen.read_measurement(args.measurement)
        target = gen.correction_curve(freqs, transfer, args)
        return args, freqs, target, gen.fit_bands(freqs, target, args)

    def test_resonance_is_cut_towards_the_band_median(self):
        args, freqs, target, bands = self.fit()
        self.assertTrue(bands, "a resonance peak needs at least one peaking filter")
        self.assertTrue(all(gain <= 0.0 for _, gain, _ in bands), bands)
        self.assertLess(gen.max_error_db(freqs, target, bands, args.rate), FIT_TOLERANCE_DB)
        cut_at_peak = sum(gen.peaking_db(PEAK_HZ, band, args.rate) for band in bands)
        self.assertLess(cut_at_peak, -PEAK_DB * gen.HALF)

    def test_flat_seat_needs_no_eq(self):
        flat = write_sweep(self.tmp.name, sweep_rows(peak_db=0.0))
        args = gen.parse_args([str(flat)])
        freqs, transfer = gen.read_measurement(args.measurement)
        target = gen.correction_curve(freqs, transfer, args)
        self.assertEqual(gen.fit_bands(freqs, target, args), [])

    def test_resonance_option_adds_cut(self):
        _, _, plain, _ = self.fit()
        _, _, extra, _ = self.fit("--resonance-hz", str(PEAK_HZ))
        self.assertLess(min(extra), min(plain))

    def test_guard_filters_are_butterworth_at_cutoff(self):
        for kind, cutoff in ((gen.HIGHPASS, gen.DEFAULT_HIGHPASS_HZ), (gen.LOWPASS, gen.DEFAULT_LOWPASS_HZ)):
            coeffs = gen.butterworth(kind, cutoff, RATE)
            self.assertAlmostEqual(biquad_db(coeffs, cutoff, RATE), CUTOFF_DB, delta=CUTOFF_TOLERANCE_DB)

    def test_preset_is_a_filter_chain_sink(self):
        args, freqs, target, bands = self.fit("--target-sink", TARGET_SINK, "--channels", "4")
        text = gen.render(bands, gen.max_error_db(freqs, target, bands, args.rate), freqs, self.sweep.name, args)
        self.assertIn("libpipewire-module-filter-chain", text)
        self.assertIn(f'node.name   = "{gen.DEFAULT_SINK_NAME}"', text)
        self.assertIn(f'target.object  = "{TARGET_SINK}"', text)
        self.assertIn("audio.position = [ FL FR RL RR ]", text)
        self.assertEqual(text.count("label = bq_raw"), 2)
        self.assertEqual(text.count("label = bq_peaking"), len(bands))
        self.assertIn("label = clamp", text)
        self.assertIn("node.dont-fallback = true", text)
        settings = [line.strip() for line in text.splitlines() if not line.strip().startswith("#")]
        self.assertFalse(any(line.startswith("node.dont-reconnect") for line in settings))

    def test_check_mode_detects_stale_preset(self):
        preset = Path(self.tmp.name) / "cargopit-tactile.conf"
        args, freqs, target, bands = self.fit()
        preset.write_text(gen.render(bands, gen.max_error_db(freqs, target, bands, args.rate),
                                     freqs, self.sweep.name, args))
        self.assertEqual(gen.main([str(self.sweep), "--check", str(preset)]), 0)
        preset.write_text(preset.read_text() + "# edited\n")
        self.assertEqual(gen.main([str(self.sweep), "--check", str(preset)]), 1)


if __name__ == "__main__":
    sys.exit(unittest.main())
