#!/usr/bin/env python3
"""Checks tools/haptics/seat_sweep.py against a mock phyphox remote API.

Covers Android and iOS behaviour: iOS rejects bare /get with 401, omits
/meta sensors, and may return extra buffer samples. Real phones stay out of CI.
"""
import importlib.util
import io
import json
import math
import sys
import tempfile
import threading
import unittest
import urllib.parse
import wave
from contextlib import redirect_stdout
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

SWEEP_TOOL = Path(__file__).resolve().parent.parent / "tools" / "haptics" / "seat_sweep.py"
FILTER_TOOL = SWEEP_TOOL.with_name("fr_to_filterchain.py")

spec = importlib.util.spec_from_file_location("seat_sweep", SWEEP_TOOL)
sweep = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sweep)

filter_spec = importlib.util.spec_from_file_location("fr_to_filterchain", FILTER_TOOL)
filt = importlib.util.module_from_spec(filter_spec)
filter_spec.loader.exec_module(filt)

SWEEP_START_HZ = 10.0
SWEEP_END_HZ = 80.0
SWEEP_POINTS = 4
SETTLE_S = 0.10
DWELL_S = 0.50
FADE_S = 0.02
AMPLITUDE = 0.25
AUDIO_RATE = 8000
CHANNELS = 2
GRAVITY_MS2 = 9.81
FLAT_MS2 = 0.40
PEAK_HZ = 40.0
PEAK_MS2 = 1.60
PEAK_SIGMA_HZ = 6.0
GAUSSIAN_EXP_SCALE = 0.5
ANDROID_FS = 400.0
IOS_FS = 100.0
IOS_NYQUIST_KEEP_HZ = (IOS_FS / 2.0) * sweep.NYQUIST_KEEP_FRACTION
HTTP_HOST = "127.0.0.1"
STATUS_OK = 200
STATUS_UNAUTHORIZED = 401
CONTENT_JSON = "application/json"
MIN_PEAK_RATIO = 2.0
EXTRA_IOS_SAMPLES = 5
NUL = None
THREAD_JOIN_S = 1.0


def gain_ms2(freq_hz):
    x = (freq_hz - PEAK_HZ) / PEAK_SIGMA_HZ
    return FLAT_MS2 + PEAK_MS2 * math.exp(-GAUSSIAN_EXP_SCALE * x * x)


def plan(start=SWEEP_START_HZ, end=SWEEP_END_HZ, points=SWEEP_POINTS):
    freqs = sweep.log_space(start, end, points)
    windows, duration_s = sweep.tone_windows(freqs, SETTLE_S, DWELL_S)
    return freqs, windows, duration_s


def freq_at(windows, play_s):
    for freq, start_s, _, end_s in windows:
        if start_s <= play_s < end_s:
            return freq
    return None


def synthetic_rows(windows, fs, arm_s, extra=0):
    total_s = arm_s + windows[-1][3]
    count = int(round(total_s * fs)) + extra
    rows = []
    for index in range(count):
        stamp = index / fs
        play_s = stamp - arm_s
        accel_z = GRAVITY_MS2
        freq = freq_at(windows, play_s)
        if freq is not None:
            accel_z += gain_ms2(freq) * math.sin(sweep.TWO_PI * freq * stamp)
        rows.append((stamp, 0.0, 0.0, accel_z))
    return rows


def rows_to_buffers(rows, extra_z=0):
    times = [row[0] for row in rows]
    xs = [row[1] for row in rows]
    ys = [row[2] for row in rows]
    zs = [row[3] for row in rows]
    if extra_z > 0 and zs:
        zs.extend([zs[-1]] * extra_z)
    return {
        sweep.BUFFER_TIME: times,
        sweep.BUFFER_X: xs,
        sweep.BUFFER_Y: ys,
        sweep.BUFFER_Z: zs,
    }


class PhyphoxState:
    def __init__(self, platform, rows):
        self.platform = platform
        self.rows = rows
        self.measuring = False
        self.cleared = False
        self.started = False
        self.stopped = False


def json_bytes(payload):
    return json.dumps(payload).encode()


def parse_get_query(query_string):
    """phyphox /get?acc_time has no '='; parse_qs would drop that key."""
    parsed = {}
    if not query_string:
        return parsed
    for part in query_string.split("&"):
        if not part:
            continue
        if "=" in part:
            name, value = part.split("=", 1)
            parsed[urllib.parse.unquote(name)] = urllib.parse.unquote(value)
            continue
        parsed[urllib.parse.unquote(part)] = ""
    return parsed


class PhyphoxServer(HTTPServer):
    allow_reuse_address = True

    def __init__(self, address, state):
        self.state = state
        HTTPServer.__init__(self, address, PhyphoxHandler)


class PhyphoxHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        return

    def phyphox_state(self):
        return self.server.state

    def _send(self, status, payload):
        body = json_bytes(payload)
        self.send_response(status)
        self.send_header("Content-Type", CONTENT_JSON)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _status_payload(self, buffers=None):
        payload = {
            sweep.JSON_STATUS: {sweep.JSON_MEASURING: self.phyphox_state().measuring},
            sweep.JSON_BUFFER: buffers or {},
        }
        return payload

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        query = parse_get_query(parsed.query)
        if path == sweep.PATH_CONFIG:
            self._send(STATUS_OK, {
                sweep.JSON_TITLE: "Acceleration with g",
                sweep.JSON_LOCAL_TITLE: "Acceleration with g",
                sweep.JSON_BUFFERS: [{"name": name} for name in sweep.REQUIRED_BUFFERS],
            })
            return
        if path == sweep.PATH_META:
            self._send_meta()
            return
        if path == sweep.PATH_CONTROL:
            self._handle_control(query.get(sweep.QUERY_CMD, ""))
            return
        if path == sweep.PATH_GET:
            self._handle_get(query)
            return
        self._send(STATUS_OK, {sweep.JSON_RESULT: False})

    def _send_meta(self):
        state = self.phyphox_state()
        meta = {sweep.JSON_DEVICE_BRAND: "Apple" if state.platform == sweep.PLATFORM_IOS else "Google"}
        if state.platform == sweep.PLATFORM_ANDROID:
            meta[sweep.JSON_SENSORS] = {"accelerometer": {"name": "mock"}}
        self._send(STATUS_OK, meta)

    def _handle_control(self, cmd):
        state = self.phyphox_state()
        if cmd == sweep.CMD_CLEAR:
            state.cleared = True
            state.measuring = False
        elif cmd == sweep.CMD_START:
            state.started = True
            state.measuring = True
        elif cmd == sweep.CMD_STOP:
            state.stopped = True
            state.measuring = False
        else:
            self._send(STATUS_OK, {sweep.JSON_RESULT: False})
            return
        self._send(STATUS_OK, {sweep.JSON_RESULT: True})

    def _handle_get(self, query):
        state = self.phyphox_state()
        if not query:
            if state.platform == sweep.PLATFORM_IOS:
                self.send_error(STATUS_UNAUTHORIZED, "columns required")
                return
            self._send(STATUS_OK, self._status_payload())
            return
        extra = EXTRA_IOS_SAMPLES if state.platform == sweep.PLATFORM_IOS else 0
        packed = rows_to_buffers(state.rows, extra_z=extra)
        buffers = {}
        for name, mode in query.items():
            values = packed.get(name, [])
            if mode == sweep.QUERY_FULL:
                buffers[name] = {sweep.JSON_UPDATE_BUFFER: values}
            else:
                last = values[-1] if values else NUL
                buffers[name] = {sweep.JSON_UPDATE_BUFFER: [last]}
        self._send(STATUS_OK, self._status_payload(buffers))


class MockPhyphox:
    def __init__(self, platform, rows):
        self.state = PhyphoxState(platform, rows)
        self.server = PhyphoxServer((HTTP_HOST, 0), self.state)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def start(self):
        self.thread.start()

    def close(self):
        self.server.shutdown()
        self.thread.join(timeout=THREAD_JOIN_S)
        self.server.server_close()

    def url(self):
        return f"http://{HTTP_HOST}:{self.server.server_address[1]}"


class SilentPlay:
    def __init__(self):
        self.calls = []

    def __call__(self, path, sink, duration_s):
        self.calls.append((Path(path), sink, duration_s))


class SeatSweepTest(unittest.TestCase):
    def test_csv_columns_match_filterchain(self):
        self.assertEqual(sweep.CSV_FREQUENCY, filt.CSV_FREQUENCY)
        self.assertEqual(sweep.CSV_TRANSFER, filt.CSV_TRANSFER)
        self.assertEqual(sweep.DEFAULT_START_HZ, filt.DEFAULT_HIGHPASS_HZ)
        self.assertEqual(sweep.DEFAULT_END_HZ, filt.DEFAULT_LOWPASS_HZ)

    def test_log_space_is_positive_and_ascending(self):
        freqs = sweep.log_space(SWEEP_START_HZ, SWEEP_END_HZ, SWEEP_POINTS)
        self.assertEqual(len(freqs), SWEEP_POINTS)
        self.assertEqual(freqs[0], SWEEP_START_HZ)
        self.assertAlmostEqual(freqs[-1], SWEEP_END_HZ)
        self.assertEqual(freqs, sorted(freqs))
        self.assertGreater(min(freqs), 0.0)

    def test_wav_duration_and_channels(self):
        freqs, windows, duration_s = plan()
        frames = sweep.render_pcm(windows, AUDIO_RATE, CHANNELS, AMPLITUDE, FADE_S)
        expected = int(round(duration_s * AUDIO_RATE))
        self.assertEqual(len(frames), expected)
        self.assertTrue(all(sweep.I16_MIN <= sample <= sweep.I16_MAX for sample in frames))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "sweep.wav"
            sweep.write_wav(path, frames, AUDIO_RATE, CHANNELS)
            with wave.open(str(path), "rb") as handle:
                self.assertEqual(handle.getnchannels(), CHANNELS)
                self.assertEqual(handle.getframerate(), AUDIO_RATE)
                self.assertEqual(handle.getnframes(), expected)

    def test_lockin_rejects_gravity_dc(self):
        freq = 12.5
        duration_s = 1.0
        times = [index / ANDROID_FS for index in range(int(ANDROID_FS * duration_s))]
        peak = 0.50
        values = [GRAVITY_MS2 + peak * math.sin(sweep.TWO_PI * freq * stamp)
                  for stamp in times]
        recovered = sweep.lockin_peak(times, values, freq)
        self.assertAlmostEqual(recovered, peak, delta=0.05)

    def test_lockin_recovers_resonance_peak(self):
        _, windows, _ = plan()
        rows = synthetic_rows(windows, ANDROID_FS, arm_s=0.0)
        keep_below = sweep.nyquist_hz([row[0] for row in rows]) * sweep.NYQUIST_KEEP_FRACTION
        points, skipped = sweep.measure_windows(rows, windows, 0.0, keep_below)
        self.assertFalse(skipped)
        self.assertEqual(len(points), SWEEP_POINTS)
        peak_freq, peak_db = max(points, key=lambda item: item[1])
        self.assertAlmostEqual(peak_freq, min(windows, key=lambda w: abs(w[0] - PEAK_HZ))[0])
        quiet_db = min(db for _, db in points)
        self.assertGreater(peak_db - quiet_db, MIN_PEAK_RATIO)

    def test_ios_sample_rate_drops_points_above_nyquist(self):
        freqs = sweep.log_space(sweep.DEFAULT_START_HZ, sweep.DEFAULT_END_HZ, sweep.DEFAULT_SWEEP_POINTS)
        windows, _ = sweep.tone_windows(freqs, SETTLE_S, DWELL_S)
        rows = synthetic_rows(windows, IOS_FS, arm_s=0.0)
        keep_below = sweep.nyquist_hz([row[0] for row in rows]) * sweep.NYQUIST_KEEP_FRACTION
        self.assertAlmostEqual(keep_below, IOS_NYQUIST_KEEP_HZ, places=1)
        points, skipped = sweep.measure_windows(rows, windows, 0.0, keep_below)
        self.assertTrue(skipped)
        self.assertTrue(all(freq < keep_below for freq, _ in points))
        self.assertTrue(all(freq >= keep_below for freq in skipped))
        self.assertTrue(any(freq > keep_below for freq, _, _, _ in windows))

    def test_ios_extra_samples_are_trimmed(self):
        _, windows, _ = plan()
        rows = synthetic_rows(windows, ANDROID_FS, arm_s=0.0)
        packed = rows_to_buffers(rows, extra_z=EXTRA_IOS_SAMPLES)
        aligned = sweep.aligned_samples(
            packed[sweep.BUFFER_TIME], packed[sweep.BUFFER_X],
            packed[sweep.BUFFER_Y], packed[sweep.BUFFER_Z],
        )
        self.assertEqual(len(aligned), len(rows))
        self.assertTrue(all(later[0] > earlier[0] for earlier, later in zip(aligned, aligned[1:])))

    def test_url_normalization(self):
        self.assertEqual(sweep.normalize_url("192.168.1.20:8080"), "http://192.168.1.20:8080")
        self.assertEqual(sweep.normalize_url("http://192.168.1.20/"), "http://192.168.1.20")
        self.assertEqual(sweep.normalize_url("http://192.168.1.20:80"), "http://192.168.1.20:80")
        with self.assertRaises(sweep.PhyphoxError):
            sweep.normalize_url("https://192.168.1.20")

    def test_ios_bare_get_is_unauthorized_android_is_not(self):
        _, windows, _ = plan()
        rows = synthetic_rows(windows, IOS_FS, arm_s=0.0)
        ios = MockPhyphox(sweep.PLATFORM_IOS, rows)
        android = MockPhyphox(sweep.PLATFORM_ANDROID, rows)
        ios.start()
        android.start()
        self.addCleanup(ios.close)
        self.addCleanup(android.close)
        ios_client = sweep.Phyphox(ios.url())
        android_client = sweep.Phyphox(android.url())
        with self.assertRaises(sweep.PhyphoxError) as raised:
            ios_client.json(sweep.PATH_GET)
        self.assertIn(str(STATUS_UNAUTHORIZED), str(raised.exception))
        android_status = android_client.json(sweep.PATH_GET)
        self.assertIn(sweep.JSON_STATUS, android_status)
        self.assertFalse(ios_client.is_measuring())
        self.assertFalse(android_client.is_measuring())

    def test_check_accepts_ios_without_sensor_meta(self):
        _, windows, _ = plan()
        rows = synthetic_rows(windows, IOS_FS, arm_s=0.0)
        mock = MockPhyphox(sweep.PLATFORM_IOS, rows)
        mock.start()
        self.addCleanup(mock.close)
        text, platform, config = sweep.check_phyphox(sweep.Phyphox(mock.url()))
        self.assertEqual(platform, sweep.PLATFORM_IOS)
        self.assertIn("iOS", text)
        self.assertIn("Acceleration with g", text)
        sweep.require_acceleration_experiment(config)
        captured = io.StringIO()
        with redirect_stdout(captured):
            self.assertEqual(sweep.main(["--phyphox", mock.url(), "--check"], play=SilentPlay()), 0)
        self.assertIn("iOS", captured.getvalue())

    def test_android_and_ios_live_sweep_write_filterchain_csv(self):
        for platform, fs in ((sweep.PLATFORM_ANDROID, ANDROID_FS), (sweep.PLATFORM_IOS, IOS_FS)):
            freqs, windows, duration_s = plan()
            rows = synthetic_rows(windows, fs, arm_s=0.0)
            mock = MockPhyphox(platform, rows)
            mock.start()
            self.addCleanup(mock.close)
            player = SilentPlay()
            with tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / "seat-sweep.csv"
                argv = [
                    "--phyphox", mock.url(),
                    "--sink", "alsa_output.test_amp",
                    "--output", str(output),
                    "--start-hz", str(SWEEP_START_HZ),
                    "--end-hz", str(SWEEP_END_HZ),
                    "--points", str(SWEEP_POINTS),
                    "--settle-s", str(SETTLE_S),
                    "--dwell-s", str(DWELL_S),
                    "--fade-s", str(FADE_S),
                    "--rate", str(AUDIO_RATE),
                    "--channels", str(CHANNELS),
                ]
                args = sweep.parse_args(argv)
                info = sweep.run_measurement(args, sweep.Phyphox(mock.url()), player, arm_s=0.0, post_s=0.0)
                self.assertEqual(info["platform"], platform)
                self.assertTrue(mock.state.cleared)
                self.assertTrue(mock.state.started)
                self.assertTrue(mock.state.stopped)
                self.assertEqual(len(player.calls), 1)
                self.assertAlmostEqual(player.calls[0][2], duration_s)
                self.assertTrue(info["points"])
                if platform == sweep.PLATFORM_IOS:
                    self.assertLess(max(freq for freq, _ in info["points"]), info["keep_below_hz"])
                sweep.write_csv(output, info["points"])
                loaded_freq, loaded_db = filt.read_measurement(output)
                self.assertEqual(len(loaded_freq), len(info["points"]))
                self.assertEqual(loaded_freq, sorted(loaded_freq))
                self.assertEqual(len(loaded_freq), len(loaded_db))

    def test_missing_acceleration_buffers_fail_early(self):
        config = {sweep.JSON_BUFFERS: [{"name": "mic"}], sweep.JSON_TITLE: "Audio Amplitude"}
        with self.assertRaises(sweep.PhyphoxError) as raised:
            sweep.require_acceleration_experiment(config)
        self.assertIn("Acceleration with g", str(raised.exception))

    def test_wav_only_writes_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "out.wav"
            code = sweep.main([
                "--wav-only", "--wav", str(path),
                "--start-hz", str(SWEEP_START_HZ),
                "--end-hz", str(SWEEP_END_HZ),
                "--points", str(SWEEP_POINTS),
                "--settle-s", str(SETTLE_S),
                "--dwell-s", str(DWELL_S),
                "--rate", str(AUDIO_RATE),
            ], play=SilentPlay())
            self.assertEqual(code, 0)
            self.assertTrue(path.is_file())
            self.assertGreater(path.stat().st_size, 0)


if __name__ == "__main__":
    sys.exit(unittest.main())
