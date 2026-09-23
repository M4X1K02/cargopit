#!/usr/bin/env python3
"""Fit a PipeWire filter-chain tactile preset to a measured rig response.

Input is a CSV of `frequency_hz,transfer_db` points from a sine sweep measured
at the seat (for example phyphox "Acceleration with g"). The correction curve
follows the rules cargopit used when this lived in C: smooth the measurement,
aim for a flat target, never boost, cap the cut, and add an extra cut at the
seat resonance. That curve is approximated with peaking biquads and wrapped in
a filter-chain virtual sink with a subsonic high-pass, a band-top low-pass and
a sample clamp.

    fr_to_filterchain.py MEASUREMENT.csv > cargopit-tactile.conf
    fr_to_filterchain.py MEASUREMENT.csv --check cargopit-tactile.conf

Only the standard library is used so it runs anywhere cargopit builds.
"""
import argparse
import cmath
import csv
import math
import sys
from pathlib import Path

DEFAULT_TARGET_TRANSFER_DB = 16.731778731404
DEFAULT_MAX_CUT_DB = 14.0
DEFAULT_MAX_BOOST_DB = 0.0
DEFAULT_NO_BOOST_BELOW_HZ = 32.0
DEFAULT_BOOST_BLEND_HZ = 8.0
DEFAULT_RESONANCE_HZ = 42.918696
DEFAULT_RESONANCE_SIGMA_HZ = 5.0
DEFAULT_RESONANCE_EXTRA_CUT_DB = 8.0
SMOOTH_RADIUS_BINS = 4
GAUSSIAN_EXP_SCALE = 0.5
SMOOTHSTEP_CUBIC = 3.0
SMOOTHSTEP_QUAD = 2.0

DEFAULT_MAX_FILTERS = 6
DEFAULT_TOLERANCE_DB = 1.0
DEFAULT_SAMPLE_RATE = 48000.0
DEFAULT_HIGHPASS_HZ = 10.0
DEFAULT_LOWPASS_HZ = 120.0
DEFAULT_CLAMP = 0.5
DEFAULT_CHANNELS = 2
DEFAULT_SINK_NAME = "cargopit_tactile"
DEFAULT_SINK_DESCRIPTION = "Cargopit tactile correction"
DEFAULT_TARGET_SINK = "alsa_output.REPLACE_WITH_TACTILE_AMP"

BUTTERWORTH_Q = 1.0 / math.sqrt(2.0)
Q_MIN = 0.3
Q_MAX = 10.0
HALF = 0.5
DB_AMPLITUDE_SCALE = 20.0
DB_PEAKING_SCALE = 40.0
REFINE_PASSES = 40
REFINE_FREQ_STEP = 0.05
REFINE_GAIN_STEP_DB = 0.5
REFINE_Q_STEP = 0.1
REFINE_SHRINK = 0.5
REFINE_MIN_GAIN_STEP_DB = 0.01
OUTPUT_DECIMALS = 2

CSV_FREQUENCY = "frequency_hz"
CSV_TRANSFER = "transfer_db"
OUTPUT_SUFFIX = ".output"

# Same speaker layouts cargopit uses for its PulseAudio channel maps.
CHANNEL_POSITIONS = {
    1: ["MONO"],
    2: ["FL", "FR"],
    4: ["FL", "FR", "RL", "RR"],
    6: ["FL", "FR", "FC", "LFE", "RL", "RR"],
    8: ["FL", "FR", "FC", "LFE", "RL", "RR", "SL", "SR"],
}


def clamp(value, low, high):
    if value < low:
        return low
    if value > high:
        return high
    return value


def read_measurement(path):
    with open(path, newline="") as handle:
        rows = list(csv.DictReader(handle))
    if not rows:
        raise SystemExit(f"{path}: no measurement rows")
    points = [(float(r[CSV_FREQUENCY]), float(r[CSV_TRANSFER])) for r in rows]
    freqs = [f for f, _ in points]
    if freqs != sorted(freqs) or min(freqs) <= 0.0:
        raise SystemExit(f"{path}: frequencies must be positive and ascending")
    return freqs, [db for _, db in points]


def mean_window(values, center, radius):
    lo = max(0, center - radius)
    hi = min(len(values) - 1, center + radius)
    window = values[lo:hi + 1]
    return sum(window) / len(window)


def smooth(values):
    return [mean_window(values, i, SMOOTH_RADIUS_BINS) for i in range(len(values))]


def smooth_in_place(values):
    """Running smooth where each point sees the already-smoothed points before it.

    The correction pass has always been computed this way, and the shipped
    preset has to reproduce the curve the rig was tuned against.
    """
    out = list(values)
    for i in range(len(out)):
        out[i] = mean_window(out, i, SMOOTH_RADIUS_BINS)
    return out


def smoothstep(t):
    t = clamp(t, 0.0, 1.0)
    return t * t * (SMOOTHSTEP_CUBIC - SMOOTHSTEP_QUAD * t)


def boost_blend(hz, args):
    if hz <= args.no_boost_below_hz:
        return 0.0
    if args.boost_blend_hz <= 0.0:
        return 1.0
    return smoothstep((hz - args.no_boost_below_hz) / args.boost_blend_hz)


def resonance_extra_cut(hz, args):
    if args.resonance_sigma_hz <= 0.0 or args.resonance_extra_cut_db <= 0.0:
        return 0.0
    x = (hz - args.resonance_hz) / args.resonance_sigma_hz
    return args.resonance_extra_cut_db * math.exp(-GAUSSIAN_EXP_SCALE * x * x)


def limit_correction(db, args):
    return clamp(db, -args.max_cut_db, args.max_boost_db)


def correction_curve(freqs, transfer_db, args):
    smoothed = smooth(transfer_db)
    raw = []
    for hz, measured in zip(freqs, smoothed):
        corr = limit_correction(args.target_db - measured, args)
        if corr > 0.0:
            corr *= boost_blend(hz, args)
        raw.append(corr)
    blended = smooth_in_place(raw)
    return [limit_correction(c - resonance_extra_cut(hz, args), args)
            for hz, c in zip(freqs, blended)]


def peaking_db(hz, band, rate):
    center, gain_db, q = band
    a = 10.0 ** (gain_db / DB_PEAKING_SCALE)
    w0 = 2.0 * math.pi * center / rate
    alpha = math.sin(w0) / (2.0 * q)
    cos_w0 = math.cos(w0)
    z = cmath.exp(-1j * 2.0 * math.pi * hz / rate)
    num = (1.0 + alpha * a) - 2.0 * cos_w0 * z + (1.0 - alpha * a) * z * z
    den = (1.0 + alpha / a) - 2.0 * cos_w0 * z + (1.0 - alpha / a) * z * z
    return DB_AMPLITUDE_SCALE * math.log10(abs(num / den))


def response_db(freqs, bands, rate):
    return [sum(peaking_db(hz, b, rate) for b in bands) for hz in freqs]


def fit_error(freqs, target, bands, rate):
    fitted = response_db(freqs, bands, rate)
    return sum((t - f) ** 2 for t, f in zip(target, fitted))


def half_width_q(freqs, residual, peak):
    half = abs(residual[peak]) * HALF
    lo = peak
    while lo > 0 and abs(residual[lo]) > half:
        lo -= 1
    hi = peak
    while hi < len(freqs) - 1 and abs(residual[hi]) > half:
        hi += 1
    if hi == lo:
        return BUTTERWORTH_Q
    octaves = math.log2(freqs[hi] / freqs[lo])
    if octaves <= 0.0:
        return Q_MAX
    ratio = 2.0 ** octaves
    return clamp(math.sqrt(ratio) / (ratio - 1.0), Q_MIN, Q_MAX)


def constrain(band, freqs, args):
    center, gain_db, q = band
    return (clamp(center, freqs[0], freqs[-1]),
            limit_correction(gain_db, args),
            clamp(q, Q_MIN, Q_MAX))


def nudged(band, index, step):
    values = list(band)
    if index == 0:
        values[0] *= 1.0 + step
    else:
        values[index] += step
    return tuple(values)


def refine_band(freqs, target, bands, index, steps, best, args):
    for param, step in enumerate(steps):
        for signed in (step, -step):
            trial = list(bands)
            trial[index] = constrain(nudged(bands[index], param, signed), freqs, args)
            err = fit_error(freqs, target, trial, args.rate)
            if err < best:
                bands, best = trial, err
    return bands, best


def refine(freqs, target, bands, args):
    steps = [REFINE_FREQ_STEP, REFINE_GAIN_STEP_DB, REFINE_Q_STEP]
    best = fit_error(freqs, target, bands, args.rate)
    for _ in range(REFINE_PASSES):
        before = best
        for index in range(len(bands)):
            bands, best = refine_band(freqs, target, bands, index, steps, best, args)
        if best >= before:
            steps = [s * REFINE_SHRINK for s in steps]
        if steps[1] < REFINE_MIN_GAIN_STEP_DB:
            break
    return bands


def max_error_db(freqs, target, bands, rate):
    fitted = response_db(freqs, bands, rate)
    return max(abs(t - f) for t, f in zip(target, fitted))


def fit_bands(freqs, target, args):
    bands = []
    for _ in range(args.max_filters):
        fitted = response_db(freqs, bands, args.rate)
        residual = [t - f for t, f in zip(target, fitted)]
        peak = max(range(len(residual)), key=lambda i: abs(residual[i]))
        if abs(residual[peak]) < args.tolerance_db:
            break
        band = (freqs[peak], residual[peak], half_width_q(freqs, residual, peak))
        bands = refine(freqs, target, bands + [constrain(band, freqs, args)], args)
    return sorted(tuple(round(v, OUTPUT_DECIMALS) for v in b) for b in bands)


def fmt(value):
    return f"{value:.{OUTPUT_DECIMALS}f}"


def node_line(name, label, controls):
    body = " ".join(f'"{k}" = {fmt(v)}' for k, v in controls)
    return f"                    {{ type = builtin name = {name} label = {label} control = {{ {body} }} }}"


def graph_nodes(bands, args):
    nodes = [("highpass", "bq_highpass", [("Freq", args.highpass_hz), ("Q", BUTTERWORTH_Q)])]
    for index, (center, gain_db, q) in enumerate(bands, start=1):
        nodes.append((f"eq{index}", "bq_peaking",
                      [("Freq", center), ("Q", q), ("Gain", gain_db)]))
    nodes.append(("lowpass", "bq_lowpass", [("Freq", args.lowpass_hz), ("Q", BUTTERWORTH_Q)]))
    nodes.append(("ceiling", "clamp", [("Min", -args.clamp), ("Max", args.clamp)]))
    return nodes


def link_lines(nodes):
    names = [name for name, _, _ in nodes]
    return [f'                    {{ output = "{a}:Out" input = "{b}:In" }}'
            for a, b in zip(names, names[1:])]


def render(bands, fit_db, freqs, measurement_name, args):
    nodes = graph_nodes(bands, args)
    positions = " ".join(CHANNEL_POSITIONS[args.channels])
    lines = [
        "# Cargopit tactile correction: PipeWire filter-chain virtual sink.",
        f"# Generated by tools/haptics/fr_to_filterchain.py from {measurement_name}.",
        f"# {len(bands)} peaking filters, max fit error {fmt(fit_db)} dB over "
        f"{fmt(freqs[0])}-{fmt(freqs[-1])} Hz.",
        "# Do not edit the eq nodes by hand; re-run the generator with your own sweep.",
        "#",
        "# Install: copy to ~/.config/pipewire/pipewire.conf.d/ and set",
        "# target.object to your tactile amplifier sink (pactl list short sinks),",
        "# then restart PipeWire. Point cargopit Sound devices at the sink name below.",
        "context.modules = [",
        "    {   name = libpipewire-module-filter-chain",
        "        args = {",
        f'            node.description = "{args.sink_description}"',
        f'            media.name       = "{args.sink_description}"',
        "            filter.graph = {",
        "                nodes = [",
        *[node_line(n, label, c) for n, label, c in nodes],
        "                ]",
        "                links = [",
        *link_lines(nodes),
        "                ]",
        "            }",
        f"            audio.channels = {args.channels}",
        f"            audio.position = [ {positions} ]",
        "            capture.props = {",
        f'                node.name   = "{args.sink_name}"',
        "                media.class = Audio/Sink",
        "            }",
        "            playback.props = {",
        f'                node.name      = "{args.sink_name}{OUTPUT_SUFFIX}"',
        "                node.passive   = true",
        f'                target.object  = "{args.target_sink}"',
        "                # Stay silent instead of falling back to desktop speakers.",
        "                node.dont-fallback  = true",
        "                node.dont-reconnect = true",
        "            }",
        "        }",
        "    }",
        "]",
    ]
    return "\n".join(lines) + "\n"


def parse_args(argv):
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("measurement", type=Path)
    p.add_argument("--check", type=Path, help="exit non-zero if FILE differs from the output")
    p.add_argument("--target-db", type=float, default=DEFAULT_TARGET_TRANSFER_DB)
    p.add_argument("--max-cut-db", type=float, default=DEFAULT_MAX_CUT_DB)
    p.add_argument("--max-boost-db", type=float, default=DEFAULT_MAX_BOOST_DB)
    p.add_argument("--no-boost-below-hz", type=float, default=DEFAULT_NO_BOOST_BELOW_HZ)
    p.add_argument("--boost-blend-hz", type=float, default=DEFAULT_BOOST_BLEND_HZ)
    p.add_argument("--resonance-hz", type=float, default=DEFAULT_RESONANCE_HZ)
    p.add_argument("--resonance-sigma-hz", type=float, default=DEFAULT_RESONANCE_SIGMA_HZ)
    p.add_argument("--resonance-extra-cut-db", type=float, default=DEFAULT_RESONANCE_EXTRA_CUT_DB)
    p.add_argument("--max-filters", type=int, default=DEFAULT_MAX_FILTERS)
    p.add_argument("--tolerance-db", type=float, default=DEFAULT_TOLERANCE_DB)
    p.add_argument("--rate", type=float, default=DEFAULT_SAMPLE_RATE)
    p.add_argument("--highpass-hz", type=float, default=DEFAULT_HIGHPASS_HZ)
    p.add_argument("--lowpass-hz", type=float, default=DEFAULT_LOWPASS_HZ)
    p.add_argument("--clamp", type=float, default=DEFAULT_CLAMP)
    p.add_argument("--channels", type=int, choices=sorted(CHANNEL_POSITIONS), default=DEFAULT_CHANNELS)
    p.add_argument("--sink-name", default=DEFAULT_SINK_NAME)
    p.add_argument("--sink-description", default=DEFAULT_SINK_DESCRIPTION)
    p.add_argument("--target-sink", default=DEFAULT_TARGET_SINK)
    return p.parse_args(argv)


def main(argv):
    args = parse_args(argv)
    freqs, transfer_db = read_measurement(args.measurement)
    target = correction_curve(freqs, transfer_db, args)
    bands = fit_bands(freqs, target, args)
    fit_db = max_error_db(freqs, target, bands, args.rate)
    output = render(bands, fit_db, freqs, args.measurement.name, args)
    if args.check is None:
        sys.stdout.write(output)
        return 0
    if args.check.read_text() == output:
        return 0
    sys.stderr.write(f"{args.check} is out of date; regenerate it with {Path(__file__).name}\n")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
