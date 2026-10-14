#!/usr/bin/env python3
"""Synthesizes Silica's interface sound effects into data/sounds/ (original work).

Every sound is made here from noise and sine waves, with a fixed random seed, so running the
script again rebuilds the same files. Nothing is sampled or recorded.

    python3 tools/sounds/synth.py [OUTPUT_DIR]

Sounds (48 kHz, 16-bit mono WAV, peak at PEAK_DBFS):

- trash-move     items moved to the Trash (menu, keyboard): a soft paper swish that falls in
                 pitch, closed by a muted low knock, ~300 ms.
- trash-drag     items dropped on the Dock's Trash: a short swish and a light metallic tick
                 from a few inharmonic partials, ~280 ms.
- trash-empty    the Trash emptied: a dry crumple of short noise grains thinning out, ~280 ms.
- file-transfer  a copy or move finished: a quiet rising two-partial blip, ~200 ms.

Requires numpy.
"""

import sys
import wave
from pathlib import Path

import numpy as np

RATE = 48_000
# Provisional: the level is set by ear until the reference's is measured (FILE-SOUNDS).
PEAK_DBFS = -12.0
SEED = 20261014


def seconds(n):
    return int(round(n * RATE))


def time(length):
    return np.arange(seconds(length)) / RATE


def bandpass(signal, centers, q):
    """A biquad band-pass (constant peak gain) whose center may change per sample."""
    centers = np.broadcast_to(np.asarray(centers, dtype=float), signal.shape)
    out = np.zeros_like(signal)
    x1 = x2 = y1 = y2 = 0.0
    for i, x in enumerate(signal):
        w = 2 * np.pi * centers[i] / RATE
        alpha = np.sin(w) / (2 * q)
        a0 = 1 + alpha
        b0, b2 = alpha / a0, -alpha / a0
        a1, a2 = -2 * np.cos(w) / a0, (1 - alpha) / a0
        y = b0 * x + b2 * x2 - a1 * y1 - a2 * y2
        x2, x1 = x1, x
        y2, y1 = y1, y
        out[i] = y
    return out


def lowpass(signal, cutoff):
    """A one-pole low-pass."""
    k = 1 - np.exp(-2 * np.pi * cutoff / RATE)
    out = np.zeros_like(signal)
    y = 0.0
    for i, x in enumerate(signal):
        y += k * (x - y)
        out[i] = y
    return out


def envelope(length, attack, decay):
    """A linear attack, then an exponential decay with time constant `decay`."""
    t = time(length)
    rise = np.clip(t / attack, 0, 1)
    fall = np.exp(-np.clip(t - attack, 0, None) / decay)
    return rise * fall


def place(total, sound, at):
    """`sound` mixed into a silent buffer of `total` seconds, starting at `at` seconds."""
    out = np.zeros(seconds(total))
    start = seconds(at)
    end = min(len(out), start + len(sound))
    out[start:end] += sound[: end - start]
    return out


def finish(signal):
    """Short fades at both ends, then the peak at PEAK_DBFS."""
    fade = seconds(0.003)
    ramp = np.linspace(0, 1, fade)
    signal = signal.copy()
    signal[:fade] *= ramp
    signal[-fade:] *= ramp[::-1]
    peak = np.max(np.abs(signal))
    return signal / peak * 10 ** (PEAK_DBFS / 20) if peak > 0 else signal


def swish(rng, length, start_hz, end_hz, q, attack, decay):
    noise = rng.standard_normal(seconds(length))
    centers = np.geomspace(start_hz, end_hz, len(noise))
    # Softened above the band: paper, not hiss.
    return lowpass(bandpass(noise, centers, q), 4500) * envelope(length, attack, decay)


def trash_move(rng):
    total = 0.30
    air = swish(rng, 0.24, 3200, 700, 1.1, 0.018, 0.070)
    t = time(0.14)
    pitch = 170 * np.exp(-t / 0.05) + 105
    knock = np.sin(2 * np.pi * np.cumsum(pitch) / RATE) * envelope(0.14, 0.002, 0.030)
    thud = lowpass(rng.standard_normal(len(t)), 500) * envelope(0.14, 0.001, 0.012)
    return place(total, air, 0) + place(total, 0.55 * knock + 0.9 * thud, 0.15)


def trash_drag(rng):
    total = 0.28
    air = swish(rng, 0.13, 2600, 1100, 1.3, 0.010, 0.040)
    t = time(0.17)
    partials = [(910, 1.0, 0.070), (1493, 0.55, 0.050), (2387, 0.35, 0.032), (3341, 0.2, 0.022)]
    tick = sum(
        amp * np.sin(2 * np.pi * hz * t) * envelope(0.17, 0.0015, decay)
        for hz, amp, decay in partials
    )
    return place(total, 0.8 * air, 0) + place(total, 0.45 * tick, 0.10)


def trash_empty(rng):
    total = 0.28
    out = np.zeros(seconds(total))
    t = 0.004
    while t < total - 0.02:
        length = rng.uniform(0.003, 0.009)
        grain = rng.standard_normal(seconds(length))
        grain = lowpass(bandpass(grain, rng.uniform(1400, 5200), 2.0), 6000)
        grain *= np.hanning(len(grain)) * rng.uniform(0.3, 1.0)
        # The grains thin out and soften towards the end.
        progress = t / total
        out += place(total, grain * (1 - 0.6 * progress), t)
        t += rng.exponential(0.0035 + 0.012 * progress)
    body = lowpass(rng.standard_normal(len(out)), 450) * envelope(total, 0.01, 0.09)
    return out + 0.35 * body / max(np.max(np.abs(body)), 1e-9) * np.max(np.abs(out))


def file_transfer(rng):
    del rng
    length = 0.20
    t = time(length)
    pitch = 560 + 230 * (1 - np.exp(-t / 0.025))
    phase = 2 * np.pi * np.cumsum(pitch) / RATE
    tone = np.sin(phase) + 0.22 * np.sin(2 * phase)
    return tone * envelope(length, 0.004, 0.050)


SOUNDS = {
    "trash-move": trash_move,
    "trash-drag": trash_drag,
    "trash-empty": trash_empty,
    "file-transfer": file_transfer,
}


def write(path, signal):
    samples = np.clip(np.round(signal * 32767), -32768, 32767).astype("<i2")
    with wave.open(str(path), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(samples.tobytes())


def main():
    root = Path(__file__).resolve().parents[2]
    target = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "data" / "sounds"
    target.mkdir(parents=True, exist_ok=True)
    for name, make in SOUNDS.items():
        # One generator per sound: changing one sound leaves the others identical.
        rng = np.random.default_rng([SEED, sum(name.encode())])
        signal = finish(make(rng))
        write(target / f"{name}.wav", signal)
        print(f"{name}.wav: {len(signal) / RATE * 1000:.0f} ms")


if __name__ == "__main__":
    main()
