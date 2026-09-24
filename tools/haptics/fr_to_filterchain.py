#!/usr/bin/env python3
"""Fit a PipeWire filter-chain tactile preset to a measured rig response.

Input is a CSV of `frequency_hz,transfer_db` points from a sine sweep measured
at the seat of your rig (for example phyphox "Acceleration with g"). The
measurement is smoothed and corrected towards a flat target (by default the
median level of the shaker band), never boosting and capping the cut, with an
optional extra cut at a seat resonance. That curve is approximated with
peaking biquads and wrapped in a filter-chain virtual sink with a subsonic
high-pass, a band-top low-pass and a sample clamp.

Measurements and the presets made from them belong to one rig, so keep them in
your own config rather than in the cargopit tree:

    fr_to_filterchain.py sweep.csv --target-sink <amp sink> \\
        > ~/.config/pipewire/pipewire.conf.d/cargopit-tactile.conf
    fr_to_filterchain.py sweep.csv --check ~/.config/pipewire/pipewire.conf.d/cargopit-tactile.conf

Only the standard library is used so it runs anywhere cargopit builds.
"""
import argparse
import cmath
import csv
import math
import sys
from pathlib import Path

DEFAULT_MAX_CUT_DB = 14.0
DEFAULT_MAX_BOOST_DB = 0.0
DEFAULT_NO_BOOST_BELOW_HZ = 32.0
DEFAULT_BOOST_BLEND_HZ = 8.0
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
COEFFICIENT_DIGITS = 12
COEFFICIENT_NAMES = ("b0", "b1", "b2", "a0", "a1", "a2")
RAW_FILTER_RATES = (44100, 48000, 88200, 96000, 176400, 192000)
HIGHPASS = "highpass"
LOWPASS = "lowpass"
CEILING = "ceiling"
NODE_INDENT = " " * 20

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


def median(values):
    ordered = sorted(values)
    mid = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[mid]
    return (ordered[mid - 1] + ordered[mid]) * HALF


def band_target_db(freqs, smoothed, args):
    """Flat target: the explicit --target-db, else the median of the shaker band."""
    if args.target_db is not None:
        return args.target_db
    in_band = [db for hz, db in zip(freqs, smoothed)
               if args.no_boost_below_hz <= hz <= args.lowpass_hz]
    return median(in_band or smoothed)


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
    if args.resonance_hz is None:
        return 0.0
    if args.resonance_sigma_hz <= 0.0 or args.resonance_extra_cut_db <= 0.0:
        return 0.0
    x = (hz - args.resonance_hz) / args.resonance_sigma_hz
    return args.resonance_extra_cut_db * math.exp(-GAUSSIAN_EXP_SCALE * x * x)


def limit_correction(db, args):
    return clamp(db, -args.max_cut_db, args.max_boost_db)


def correction_curve(freqs, transfer_db, args):
    smoothed = smooth(transfer_db)
    target_db = band_target_db(freqs, smoothed, args)
    raw = []
    for hz, measured in zip(freqs, smoothed):
        corr = limit_correction(target_db - measured, args)
        if corr > 0.0:
            corr *= boost_blend(hz, args)
        raw.append(corr)
    blended = smooth(raw)
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
        # Only chase error a band may fix: with boost capped, a positive
        # residual would just add a 0 dB filter.
        residual = [limit_correction(t - f, args) for t, f in zip(target, fitted)]
        peak = max(range(len(residual)), key=lambda i: abs(residual[i]))
        if abs(residual[peak]) < args.tolerance_db:
            break
        band = (freqs[peak], residual[peak], half_width_q(freqs, residual, peak))
        bands = refine(freqs, target, bands + [constrain(band, freqs, args)], args)
    rounded = [tuple(round(v, OUTPUT_DECIMALS) for v in b) for b in bands]
    return sorted(b for b in rounded if b[1] != 0.0)


def fmt(value):
    return f"{value:.{OUTPUT_DECIMALS}f}"


def butterworth(kind, cutoff_hz, rate):
    """RBJ second-order Butterworth coefficients (b0, b1, b2, a0, a1, a2)."""
    w0 = 2.0 * math.pi * cutoff_hz / rate
    alpha = math.sin(w0) / (2.0 * BUTTERWORTH_Q)
    cos_w0 = math.cos(w0)
    if kind == HIGHPASS:
        b0 = (1.0 + cos_w0) / 2.0
        b1 = -(1.0 + cos_w0)
    else:
        b0 = (1.0 - cos_w0) / 2.0
        b1 = 1.0 - cos_w0
    return (b0, b1, b0, 1.0 + alpha, -2.0 * cos_w0, 1.0 - alpha)


def fmt_coefficient(value):
    return f"{value:.{COEFFICIENT_DIGITS}g}"


def control_node(name, label, controls):
    body = " ".join(f'"{k}" = {fmt(v)}' for k, v in controls)
    return f"{NODE_INDENT}{{ type = builtin name = {name} label = {label} control = {{ {body} }} }}"


def raw_node(name, kind, cutoff_hz):
    """bq_raw keeps the guard filters exact: bq_lowpass/bq_highpass read "Q" as
    a dB resonance before PipeWire 1.4 and as a real Q from 1.4 on."""
    rows = []
    for rate in RAW_FILTER_RATES:
        coeffs = butterworth(kind, cutoff_hz, rate)
        pairs = ", ".join(f"{k}={fmt_coefficient(v)}" for k, v in zip(COEFFICIENT_NAMES, coeffs))
        rows.append(f"{NODE_INDENT}        {{ rate = {rate}, {pairs} }}")
    return "\n".join([
        f"{NODE_INDENT}# {kind} {fmt(cutoff_hz)} Hz, Butterworth",
        f"{NODE_INDENT}{{ type = builtin name = {name} label = bq_raw",
        f"{NODE_INDENT}  config = {{ coefficients = [",
        *rows,
        f"{NODE_INDENT}  ] }} }}",
    ])


def graph_nodes(bands, args):
    nodes = [(HIGHPASS, raw_node(HIGHPASS, HIGHPASS, args.highpass_hz))]
    for index, (center, gain_db, q) in enumerate(bands, start=1):
        name = f"eq{index}"
        nodes.append((name, control_node(name, "bq_peaking",
                                         [("Freq", center), ("Q", q), ("Gain", gain_db)])))
    nodes.append((LOWPASS, raw_node(LOWPASS, LOWPASS, args.lowpass_hz)))
    nodes.append((CEILING, control_node(CEILING, "clamp", [("Min", -args.clamp), ("Max", args.clamp)])))
    return nodes


def link_lines(nodes):
    names = [name for name, _ in nodes]
    return [f'{NODE_INDENT}{{ output = "{a}:Out" input = "{b}:In" }}'
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
        *[text for _, text in nodes],
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
        "                # WirePlumber 0.5+: stay unlinked instead of falling back to the",
        "                # default sink when target.object is missing. Do not add",
        "                # node.dont-reconnect: WirePlumber destroys the stream if it is",
        "                # handled before the amplifier sink appears, e.g. at login.",
        "                node.dont-fallback = true",
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
    p.add_argument("--target-db", type=float, default=None,
                   help="flat target level (default: median of the shaker band)")
    p.add_argument("--max-cut-db", type=float, default=DEFAULT_MAX_CUT_DB)
    p.add_argument("--max-boost-db", type=float, default=DEFAULT_MAX_BOOST_DB)
    p.add_argument("--no-boost-below-hz", type=float, default=DEFAULT_NO_BOOST_BELOW_HZ)
    p.add_argument("--boost-blend-hz", type=float, default=DEFAULT_BOOST_BLEND_HZ)
    p.add_argument("--resonance-hz", type=float, default=None,
                   help="extra cut centred on this seat resonance (default: none)")
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
