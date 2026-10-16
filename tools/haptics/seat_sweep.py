#!/usr/bin/env python3
"""Play a seat-shaker sine sweep and record the response with phyphox.

Puts a stepped sine on a PulseAudio / PipeWire sink while phyphox
"Acceleration with g" (or "Acceleration (without g)") records through its
remote HTTP API. After the sweep, each tone is measured with a lock-in at the
drive frequency and written as `frequency_hz,transfer_db` for
fr_to_filterchain.py.

Android and iOS both serve that API once **Allow remote access** is on:

    Android default URL:  http://PHONE_IP:8080
    iPhone default URL:   http://PHONE_IP          (port 80)

iOS will not serve remote access while cellular data is up. Its accelerometer
is typically 100 Hz, so this script drops points above 90% of Nyquist; Android
phones that report 200 Hz or faster can cover the full 10-120 Hz shaker band.

Only the standard library is used. paplay or pw-play plays the stimulus.

    seat_sweep.py --phyphox http://192.168.1.20:8080 \\
        --sink <amp sink from pactl list short sinks> \\
        --output ~/.config/cargopit/seat-sweep.csv
"""
import argparse
import importlib.util
import json
import math
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import wave
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
FILTERCHAIN_PATH = SCRIPT_DIR / "fr_to_filterchain.py"
FILTERCHAIN_MODULE = "fr_to_filterchain"


def load_filterchain():
    spec = importlib.util.spec_from_file_location(FILTERCHAIN_MODULE, FILTERCHAIN_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


FILTER = load_filterchain()

CSV_FREQUENCY = FILTER.CSV_FREQUENCY
CSV_TRANSFER = FILTER.CSV_TRANSFER
DEFAULT_START_HZ = FILTER.DEFAULT_HIGHPASS_HZ
DEFAULT_END_HZ = FILTER.DEFAULT_LOWPASS_HZ
DEFAULT_SAMPLE_RATE = int(FILTER.DEFAULT_SAMPLE_RATE)
DEFAULT_CHANNELS = FILTER.DEFAULT_CHANNELS
DB_AMPLITUDE_SCALE = FILTER.DB_AMPLITUDE_SCALE

DEFAULT_SWEEP_POINTS = 36
DEFAULT_SETTLE_S = 0.50
DEFAULT_DWELL_S = 1.00
DEFAULT_FADE_S = 0.05
DEFAULT_AMPLITUDE = 0.25
DEFAULT_HTTP_TIMEOUT_S = 10.0
PHYPOX_ARM_S = 0.40
POST_ROLL_S = 0.25
STATUS_POLL_S = 0.25
STATUS_POLL_TRIES = 8
NYQUIST_KEEP_FRACTION = 0.90
MIN_SAMPLES = 8
MIN_PERIODS = 2.0
REFERENCE_MS2 = 1.0
LOCKIN_SCALE = 2.0
I16_MAX = 32767
I16_MIN = -32768
WAV_SAMPLE_WIDTH = 2
TWO_PI = 2.0 * math.pi
FADE_COSINE_OFFSET = 1.0
FADE_COSINE_SCALE = 0.5
WAV_ONLY_SINK = "-"

URL_SCHEME_HTTP = "http"
HEADER_CONNECTION = "Connection"
CONNECTION_CLOSE = "close"
HEADER_CONTENT_TYPE = "Content-Type"
JSON_CONTENT_TYPE = "application/json"

PATH_GET = "/get"
PATH_CONTROL = "/control"
PATH_CONFIG = "/config"
PATH_META = "/meta"
QUERY_CMD = "cmd"
QUERY_FULL = "full"
CMD_START = "start"
CMD_STOP = "stop"
CMD_CLEAR = "clear"

JSON_RESULT = "result"
JSON_STATUS = "status"
JSON_MEASURING = "measuring"
JSON_BUFFER = "buffer"
JSON_BUFFERS = "buffers"
JSON_NAME = "name"
JSON_TITLE = "title"
JSON_LOCAL_TITLE = "localTitle"
JSON_DEVICE_BRAND = "deviceBrand"
JSON_SENSORS = "sensors"
JSON_UPDATE_BUFFER = "buffer"

BUFFER_TIME = "acc_time"
BUFFER_X = "accX"
BUFFER_Y = "accY"
BUFFER_Z = "accZ"
REQUIRED_BUFFERS = (BUFFER_TIME, BUFFER_X, BUFFER_Y, BUFFER_Z)
ACCELERATION_TITLE_MARKERS = ("acceleration", "accelerometer")

PLATFORM_IOS = "ios"
PLATFORM_ANDROID = "android"
PLATFORM_UNKNOWN = "unknown"
BRAND_APPLE = "apple"

PAPLAY_BIN = "paplay"
PW_PLAY_BIN = "pw-play"
PACTL_BIN = "pactl"
PACTL_LIST_SHORT_SINKS = ("list", "short", "sinks")
PAPLAY_DEVICE_FLAG = "--device="
PW_PLAY_TARGET_FLAG = "--target"

HTTP_UNAUTHORIZED = 401
CSV_DECIMALS = 6
PLAY_SLACK_S = 10.0


class PhyphoxError(Exception):
    """Remote-API failure, including the iOS bare-/get 401."""


class CloseConnectionHandler(urllib.request.HTTPHandler):
    """phyphox breaks if the client keeps the HTTP connection alive."""

    def http_request(self, req):
        req.add_unredirected_header(HEADER_CONNECTION, CONNECTION_CLOSE)
        return urllib.request.HTTPHandler.http_request(self, req)


def normalize_url(url):
    text = url.strip()
    if not text:
        raise PhyphoxError("phyphox URL is empty")
    if "://" not in text:
        text = f"{URL_SCHEME_HTTP}://{text}"
    parsed = urllib.parse.urlparse(text)
    if parsed.scheme != URL_SCHEME_HTTP:
        raise PhyphoxError(f"phyphox URL must be {URL_SCHEME_HTTP}://")
    if not parsed.netloc:
        raise PhyphoxError(f"phyphox URL has no host: {url}")
    return urllib.parse.urlunparse((parsed.scheme, parsed.netloc, "", "", "", ""))


def hint_for_connection(url):
    parsed = urllib.parse.urlparse(url)
    if parsed.port is None:
        return (
            f"If this is an Android phone, try {url}:8080. "
            "Enable Allow remote access in phyphox, same Wi-Fi as this PC."
        )
    host = parsed.hostname or parsed.netloc
    return (
        f"If this is an iPhone, try {URL_SCHEME_HTTP}://{host} (port 80) and "
        "turn cellular data off. Enable Allow remote access in phyphox."
    )


class Phyphox:
    def __init__(self, url, timeout_s=DEFAULT_HTTP_TIMEOUT_S):
        self.base = normalize_url(url)
        self.timeout_s = timeout_s
        self._opener = urllib.request.build_opener(CloseConnectionHandler)

    def request_bytes(self, path):
        target = self.base + path
        try:
            with self._opener.open(target, timeout=self.timeout_s) as response:
                if response.status != 200:
                    raise PhyphoxError(f"{target}: HTTP {response.status}")
                return response.read()
        except urllib.error.HTTPError as err:
            raise PhyphoxError(f"{target}: HTTP {err.code}") from err
        except urllib.error.URLError as err:
            raise PhyphoxError(f"{target}: {err.reason}. {hint_for_connection(self.base)}") from err

    def json(self, path):
        raw = self.request_bytes(path)
        try:
            return json.loads(raw.decode())
        except json.JSONDecodeError as err:
            raise PhyphoxError(f"{self.base}{path}: not JSON") from err

    def control(self, cmd):
        payload = self.json(f"{PATH_CONTROL}?{QUERY_CMD}={cmd}")
        if not payload.get(JSON_RESULT):
            raise PhyphoxError(f"phyphox {cmd} returned {payload!r}")

    def config(self):
        return self.json(PATH_CONFIG)

    def meta(self):
        return self.json(PATH_META)

    def status(self):
        payload = self.json(f"{PATH_GET}?{BUFFER_TIME}")
        status = payload.get(JSON_STATUS)
        if not isinstance(status, dict) or JSON_MEASURING not in status:
            raise PhyphoxError("phyphox status.measuring missing")
        return status

    def is_measuring(self):
        return bool(self.status()[JSON_MEASURING])

    def wait_measuring(self, want, tries=STATUS_POLL_TRIES, interval_s=STATUS_POLL_S):
        for _ in range(tries):
            if self.is_measuring() == want:
                return True
            time.sleep(interval_s)
        return False

    def read_buffers(self, names):
        query = "&".join(f"{name}={QUERY_FULL}" for name in names)
        payload = self.json(f"{PATH_GET}?{query}")
        blocks = payload.get(JSON_BUFFER)
        if not isinstance(blocks, dict):
            raise PhyphoxError("phyphox /get returned no buffer object")
        out = {}
        for name in names:
            block = blocks.get(name)
            if not isinstance(block, dict):
                raise PhyphoxError(f"phyphox buffer {name} missing")
            values = block.get(JSON_UPDATE_BUFFER)
            if not isinstance(values, list):
                raise PhyphoxError(f"phyphox buffer {name} is not a list")
            out[name] = values
        return out


def buffer_names(config):
    buffers = config.get(JSON_BUFFERS)
    if not isinstance(buffers, list):
        return []
    names = []
    for entry in buffers:
        if isinstance(entry, dict) and entry.get(JSON_NAME):
            names.append(entry[JSON_NAME])
            continue
        if isinstance(entry, str):
            names.append(entry)
    return names


def require_acceleration_experiment(config):
    names = set(buffer_names(config))
    missing = [name for name in REQUIRED_BUFFERS if name not in names]
    if missing:
        available = ", ".join(sorted(names)) or "(none)"
        raise PhyphoxError(
            "Open phyphox experiment Acceleration with g (or without g), then "
            f"enable remote access. Missing buffers {missing}; have {available}."
        )
    return names


def experiment_title(config):
    title = config.get(JSON_LOCAL_TITLE) or config.get(JSON_TITLE) or ""
    return str(title)


def title_looks_like_acceleration(title):
    lowered = title.lower()
    return any(marker in lowered for marker in ACCELERATION_TITLE_MARKERS)


def platform_from_meta(meta):
    if not isinstance(meta, dict):
        return PLATFORM_UNKNOWN
    brand = str(meta.get(JSON_DEVICE_BRAND, "")).strip().lower()
    if brand == BRAND_APPLE:
        return PLATFORM_IOS
    if JSON_SENSORS in meta:
        return PLATFORM_ANDROID
    if brand:
        return PLATFORM_ANDROID
    return PLATFORM_UNKNOWN


def log_space(start_hz, end_hz, count):
    if start_hz <= 0.0 or end_hz <= start_hz:
        raise PhyphoxError("sweep frequencies must be positive and ascending")
    if count < 2:
        raise PhyphoxError("need at least two sweep points")
    ratio = (end_hz / start_hz) ** (1.0 / (count - 1))
    return [start_hz * ratio ** index for index in range(count)]


def fade_gain(index, fade_samples, total_samples):
    if fade_samples <= 0:
        return 1.0
    if index < fade_samples:
        return FADE_COSINE_SCALE * (FADE_COSINE_OFFSET - math.cos(math.pi * index / fade_samples))
    remaining = total_samples - 1 - index
    if remaining < fade_samples:
        return FADE_COSINE_SCALE * (FADE_COSINE_OFFSET - math.cos(math.pi * remaining / fade_samples))
    return 1.0


def tone_windows(freqs, settle_s, dwell_s):
    if settle_s < 0.0 or dwell_s <= 0.0:
        raise PhyphoxError("settle must be >= 0 and dwell must be > 0")
    windows = []
    cursor = 0.0
    for freq in freqs:
        start_s = cursor
        analyze_start_s = start_s + settle_s
        analyze_end_s = analyze_start_s + dwell_s
        windows.append((freq, start_s, analyze_start_s, analyze_end_s))
        cursor = analyze_end_s
    return windows, cursor


def render_pcm(windows, rate, channels, amplitude, fade_s):
    if not 0.0 < amplitude <= 1.0:
        raise PhyphoxError("amplitude must be in (0, 1]")
    if channels < 1:
        raise PhyphoxError("channels must be >= 1")
    fade_samples = int(round(fade_s * rate))
    frames = []
    phase = 0.0
    for freq, start_s, _, analyze_end_s in windows:
        duration_s = analyze_end_s - start_s
        total_samples = int(round(duration_s * rate))
        step = TWO_PI * freq / rate
        for index in range(total_samples):
            sample = amplitude * fade_gain(index, fade_samples, total_samples) * math.sin(phase)
            clipped = max(I16_MIN, min(I16_MAX, int(round(sample * I16_MAX))))
            frames.append(clipped)
            phase += step
            if phase > TWO_PI:
                phase -= TWO_PI
    return frames


def write_wav(path, frames, rate, channels):
    packed = bytearray()
    sample = struct.Struct("<h")
    for frame in frames:
        packed.extend(sample.pack(frame) * channels)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(channels)
        handle.setsampwidth(WAV_SAMPLE_WIDTH)
        handle.setframerate(rate)
        handle.writeframes(bytes(packed))


def find_player():
    if shutil.which(PAPLAY_BIN):
        return PAPLAY_BIN
    if shutil.which(PW_PLAY_BIN):
        return PW_PLAY_BIN
    return None


def player_command(binary, sink, path):
    if binary == PAPLAY_BIN:
        return [binary, f"{PAPLAY_DEVICE_FLAG}{sink}", str(path)]
    if binary == PW_PLAY_BIN:
        return [binary, PW_PLAY_TARGET_FLAG, sink, str(path)]
    raise PhyphoxError("no audio player (install paplay or pw-play)")


def play_wav(path, sink, duration_s):
    binary = find_player()
    if binary is None:
        raise PhyphoxError("no audio player (install paplay or pw-play)")
    command = player_command(binary, sink, path)
    timeout_s = duration_s + PLAY_SLACK_S
    try:
        subprocess.run(command, check=True, timeout=timeout_s)
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError) as err:
        raise PhyphoxError(f"failed to play sweep on sink {sink}: {err}") from err


def list_sinks():
    try:
        result = subprocess.run(
            [PACTL_BIN, *PACTL_LIST_SHORT_SINKS],
            check=True,
            capture_output=True,
            text=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError) as err:
        raise PhyphoxError(f"cannot list sinks ({PACTL_BIN}): {err}") from err
    return result.stdout


def aligned_samples(times, xs, ys, zs):
    count = min(len(times), len(xs), len(ys), len(zs))
    rows = []
    last_t = None
    for index in range(count):
        values = (times[index], xs[index], ys[index], zs[index])
        if any(value is None for value in values):
            continue
        stamp = float(values[0])
        if last_t is not None and stamp <= last_t:
            continue
        rows.append((stamp, float(values[1]), float(values[2]), float(values[3])))
        last_t = stamp
    return rows


def median(values):
    ordered = sorted(values)
    mid = len(ordered) // 2
    if not ordered:
        return 0.0
    if len(ordered) % 2:
        return ordered[mid]
    return (ordered[mid - 1] + ordered[mid]) / 2.0


def sample_rate_hz(times):
    deltas = [later - earlier for earlier, later in zip(times, times[1:]) if later > earlier]
    dt = median(deltas)
    if dt <= 0.0:
        raise PhyphoxError("phyphox timestamps are not increasing")
    return 1.0 / dt


def nyquist_hz(times):
    return sample_rate_hz(times) / 2.0


def remove_mean(values):
    mean = sum(values) / len(values)
    return [value - mean for value in values]


def lockin_peak(times, values, freq_hz):
    if len(times) < MIN_SAMPLES:
        return 0.0
    centered = remove_mean(values)
    omega = TWO_PI * freq_hz
    sum_cos = 0.0
    sum_sin = 0.0
    sum_dt = 0.0
    for index in range(len(times) - 1):
        dt = times[index + 1] - times[index]
        if dt <= 0.0:
            continue
        stamp = times[index]
        value = centered[index]
        sum_cos += value * math.cos(omega * stamp) * dt
        sum_sin += value * math.sin(omega * stamp) * dt
        sum_dt += dt
    if sum_dt <= 0.0:
        return 0.0
    return math.hypot(LOCKIN_SCALE * sum_cos / sum_dt, LOCKIN_SCALE * sum_sin / sum_dt)


def vector_lockin(rows, freq_hz):
    times = [row[0] for row in rows]
    amp_x = lockin_peak(times, [row[1] for row in rows], freq_hz)
    amp_y = lockin_peak(times, [row[2] for row in rows], freq_hz)
    amp_z = lockin_peak(times, [row[3] for row in rows], freq_hz)
    return math.sqrt(amp_x * amp_x + amp_y * amp_y + amp_z * amp_z)


def in_window(rows, start_s, end_s):
    return [row for row in rows if start_s <= row[0] < end_s]


def transfer_db(amplitude_ms2):
    if amplitude_ms2 <= 0.0:
        return None
    return DB_AMPLITUDE_SCALE * math.log10(amplitude_ms2 / REFERENCE_MS2)


def measure_windows(rows, windows, play_offset_s, max_hz):
    points = []
    skipped = []
    for freq, _, analyze_start_s, analyze_end_s in windows:
        if freq >= max_hz:
            skipped.append(freq)
            continue
        start_s = play_offset_s + analyze_start_s
        end_s = play_offset_s + analyze_end_s
        slice_rows = in_window(rows, start_s, end_s)
        duration_s = end_s - start_s
        if len(slice_rows) < MIN_SAMPLES or duration_s * freq < MIN_PERIODS:
            skipped.append(freq)
            continue
        db = transfer_db(vector_lockin(slice_rows, freq))
        if db is None:
            skipped.append(freq)
            continue
        points.append((freq, db))
    return points, skipped


def write_csv(path, points):
    lines = [f"{CSV_FREQUENCY},{CSV_TRANSFER}"]
    lines += [f"{freq:.{CSV_DECIMALS}f},{db:.{CSV_DECIMALS}f}" for freq, db in points]
    text = "\n".join(lines) + "\n"
    if path is None:
        sys.stdout.write(text)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--phyphox", help="phyphox remote URL (Android :8080, iPhone port 80)")
    parser.add_argument("--sink", help="Pulse/PipeWire sink that feeds the tactile amplifier")
    parser.add_argument("--output", type=Path, help=f"CSV path ({CSV_FREQUENCY},{CSV_TRANSFER})")
    parser.add_argument("--wav", type=Path, help="write the stimulus WAV here")
    parser.add_argument("--start-hz", type=float, default=DEFAULT_START_HZ)
    parser.add_argument("--end-hz", type=float, default=DEFAULT_END_HZ)
    parser.add_argument("--points", type=int, default=DEFAULT_SWEEP_POINTS)
    parser.add_argument("--settle-s", type=float, default=DEFAULT_SETTLE_S)
    parser.add_argument("--dwell-s", type=float, default=DEFAULT_DWELL_S)
    parser.add_argument("--fade-s", type=float, default=DEFAULT_FADE_S)
    parser.add_argument("--amplitude", type=float, default=DEFAULT_AMPLITUDE)
    parser.add_argument("--rate", type=int, default=DEFAULT_SAMPLE_RATE)
    parser.add_argument("--channels", type=int, default=DEFAULT_CHANNELS)
    parser.add_argument("--timeout-s", type=float, default=DEFAULT_HTTP_TIMEOUT_S)
    parser.add_argument("--list-sinks", action="store_true", help="print pactl sinks and exit")
    parser.add_argument("--check", action="store_true",
                        help="connect to phyphox, print platform and experiment, exit")
    parser.add_argument("--wav-only", action="store_true",
                        help="write the WAV and exit without talking to phyphox")
    return parser.parse_args(argv)


def describe_platform(platform, meta):
    brand = ""
    if isinstance(meta, dict):
        brand = str(meta.get(JSON_DEVICE_BRAND, "")).strip()
    if platform == PLATFORM_IOS:
        detail = brand or "Apple"
        return (
            f"{detail} (iOS). Remote access uses port 80; turn cellular data off. "
            "Accelerometer rate is typically 100 Hz, so points above Nyquist are dropped."
        )
    if platform == PLATFORM_ANDROID:
        detail = brand or "Android"
        return f"{detail} (Android). Default remote port is 8080."
    return "unknown phyphox platform (Android and iOS both work; /meta sensors are Android-only)."


def check_phyphox(client):
    config = client.config()
    require_acceleration_experiment(config)
    try:
        meta = client.meta()
    except PhyphoxError:
        meta = {}
    platform = platform_from_meta(meta)
    title = experiment_title(config)
    measuring = client.is_measuring()
    lines = [
        f"phyphox {client.base}",
        f"experiment: {title or '(untitled)'}",
        f"platform: {describe_platform(platform, meta)}",
        f"measuring: {measuring}",
    ]
    if title and not title_looks_like_acceleration(title):
        lines.append("warning: title is not an acceleration experiment; buffers matched anyway.")
    return "\n".join(lines) + "\n", platform, config


def run_measurement(args, client, play, arm_s=PHYPOX_ARM_S, post_s=POST_ROLL_S):
    config = client.config()
    require_acceleration_experiment(config)
    try:
        meta = client.meta()
    except PhyphoxError:
        meta = {}
    platform = platform_from_meta(meta)
    freqs = log_space(args.start_hz, args.end_hz, args.points)
    windows, duration_s = tone_windows(freqs, args.settle_s, args.dwell_s)
    frames = render_pcm(windows, args.rate, args.channels, args.amplitude, args.fade_s)

    wav_path = args.wav
    tmp = None
    if wav_path is None:
        tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
        tmp.close()
        wav_path = Path(tmp.name)
    try:
        write_wav(wav_path, frames, args.rate, args.channels)
        client.control(CMD_CLEAR)
        if not client.wait_measuring(False):
            raise PhyphoxError("phyphox did not stop after clear")
        client.control(CMD_START)
        if not client.wait_measuring(True):
            raise PhyphoxError("phyphox did not start recording")
        time.sleep(arm_s)
        play_started = time.monotonic()
        play(wav_path, args.sink, duration_s)
        play_elapsed = time.monotonic() - play_started
        time.sleep(post_s)
        client.control(CMD_STOP)
        if not client.wait_measuring(False):
            raise PhyphoxError("phyphox did not stop recording")
        buffers = client.read_buffers(REQUIRED_BUFFERS)
        rows = aligned_samples(
            buffers[BUFFER_TIME], buffers[BUFFER_X], buffers[BUFFER_Y], buffers[BUFFER_Z],
        )
        if len(rows) < MIN_SAMPLES:
            raise PhyphoxError("phyphox returned too few acceleration samples")
        times = [row[0] for row in rows]
        rate_hz = sample_rate_hz(times)
        keep_below = nyquist_hz(times) * NYQUIST_KEEP_FRACTION
        play_offset_s = max(times[0], arm_s)
        points, skipped = measure_windows(rows, windows, play_offset_s, keep_below)
        info = {
            "platform": platform,
            "sample_rate_hz": rate_hz,
            "keep_below_hz": keep_below,
            "points": points,
            "skipped": skipped,
            "play_elapsed_s": play_elapsed,
            "duration_s": duration_s,
            "title": experiment_title(config),
        }
        return info
    finally:
        if tmp is not None:
            os.unlink(tmp.name)


def report_measurement(info, stream):
    stream.write(f"experiment: {info['title'] or '(untitled)'}\n")
    stream.write(f"platform: {info['platform']}\n")
    stream.write(f"phone sample rate: {info['sample_rate_hz']:.1f} Hz\n")
    stream.write(f"keeping frequencies below {info['keep_below_hz']:.1f} Hz (Nyquist headroom)\n")
    stream.write(f"measured {len(info['points'])} points, skipped {len(info['skipped'])}\n")
    if info["platform"] == PLATFORM_IOS and info["skipped"]:
        stream.write(
            "iOS accelerometer is typically ~100 Hz, so the upper shaker band "
            "is above Nyquist and was dropped. Android (200 Hz+) covers 10-120 Hz.\n"
        )
    elif info["skipped"]:
        stream.write(
            "skipped drive frequencies above the phone's Nyquist or with too few samples.\n"
        )


def main(argv, play=play_wav):
    args = parse_args(argv)
    if args.list_sinks:
        sys.stdout.write(list_sinks())
        return 0
    if args.wav_only:
        freqs = log_space(args.start_hz, args.end_hz, args.points)
        windows, _ = tone_windows(freqs, args.settle_s, args.dwell_s)
        frames = render_pcm(windows, args.rate, args.channels, args.amplitude, args.fade_s)
        if args.wav is None:
            raise PhyphoxError("--wav-only needs --wav")
        write_wav(args.wav, frames, args.rate, args.channels)
        return 0
    if not args.phyphox:
        raise PhyphoxError("--phyphox is required unless --list-sinks or --wav-only")
    client = Phyphox(args.phyphox, timeout_s=args.timeout_s)
    if args.check:
        text, _, _ = check_phyphox(client)
        sys.stdout.write(text)
        return 0
    if not args.sink or args.sink == WAV_ONLY_SINK:
        raise PhyphoxError("--sink is required (pactl list short sinks)")
    info = run_measurement(args, client, play)
    if not info["points"]:
        raise PhyphoxError("no usable frequency-response points")
    report_measurement(info, sys.stderr)
    write_csv(args.output, info["points"])
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except PhyphoxError as err:
        sys.stderr.write(f"{err}\n")
        sys.exit(1)
