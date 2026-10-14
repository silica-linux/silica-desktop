#!/usr/bin/env python3
"""Synthesizes Silica's interface sound effects (original work, nothing sampled or recorded).

Every sound is built here from physical and granular models with fixed random seeds, so
running the script again rebuilds the same files bit for bit:

- modal synthesis: struck objects (bins, glass, shutter blades) are banks of damped
  resonances whose frequency ratios come from the physics of the shape (plates, bars, rings,
  stiff wires), excited by shaped force pulses or noise bursts, with frequency-dependent
  damping (high modes die faster);
- granular synthesis: paper (rustle, crumple) is many short grains of band-passed noise
  whose density follows a random envelope, with a few louder crackles and a spectral tilt
  that changes over time;
- time-varying filtered noise for air (whooshes, swishes);
- polish: a generated small-room impulse response mixed very low, DC removal, a click-free
  start, a fade to true silence, a K-weighted loudness normalisation (ITU-R BS.1770
  weighting, maximum 400 ms loudness) to TARGET_LUFS and a look-ahead soft limiter at
  CEILING_DBFS.

    python3 tools/sounds/synth.py [--spectrograms]

Writes three candidates per sound to tools/sounds/candidates/<name>-<A|B|C>.wav with an
index.json describing them, and copies the chosen candidate (CHOSEN) to
data/sounds/<name>.wav. `--spectrograms` also renders a PNG per candidate next to it
(needs matplotlib; the PNGs are not committed). See tools/sounds/README.md for the design.

Output: 48 kHz, 16-bit mono WAV. Requires numpy.
"""

import json
import shutil
import sys
import wave
from pathlib import Path

import numpy as np
from numpy.lib.stride_tricks import sliding_window_view

RATE = 48_000
SEED = 20261015
TARGET_LUFS = -20.0
CEILING_DBFS = -6.0
# The limiter may take at most this much off the peaks; beyond it the sound is left quieter.
MAX_LIMIT_DB = 6.0
ROOM_MIX = 0.08  # about -22 dB of the room under the dry sound

# The candidate shipped in data/sounds/ for each sound.
CHOSEN = {
    "trash-move": "A",
    "trash-drag": "A",
    "trash-empty": "A",
    "file-transfer": "A",
    "screen-capture": "A",
}


# --- Basics -----------------------------------------------------------------------------


def n_of(seconds):
    return int(round(seconds * RATE))


def times(length):
    return np.arange(n_of(length)) / RATE


def pow2(n):
    return 1 << int(n - 1).bit_length()


def convolve(a, b):
    n = len(a) + len(b) - 1
    m = pow2(n)
    return np.fft.irfft(np.fft.rfft(a, m) * np.fft.rfft(b, m), m)[:n]


def place(total, sound, at, gain=1.0):
    """`sound` times `gain` in a silent buffer of `total` seconds, starting at `at` seconds."""
    out = np.zeros(n_of(total))
    start = n_of(at)
    end = min(len(out), start + len(sound))
    out[start:end] += gain * sound[: end - start]
    return out


def mix(total, *parts):
    """Sum of (sound, at, gain) parts in a buffer of at least `total` seconds (longer if a
    part rings past it: nothing is cut)."""
    total = max([total] + [at + len(sound) / RATE + 0.001 for sound, at, _ in parts])
    return sum(place(total, sound, at, gain) for sound, at, gain in parts)


def unit(x):
    peak = np.max(np.abs(x))
    return x / peak if peak > 0 else x


# --- Filters (magnitude responses applied in the frequency domain) ----------------------


def bandpass_mag(f, fc, q):
    """Magnitude of a second-order band-pass (unity at fc)."""
    f = np.maximum(f, 1e-3)
    return 1 / np.sqrt(1 + q * q * (f / fc - fc / f) ** 2)


def lowpass_mag(f, fc, order=2):
    return 1 / np.sqrt(1 + (f / fc) ** (2 * order))


def highpass_mag(f, fc, order=2):
    f = np.maximum(f, 1e-3)
    return 1 / np.sqrt(1 + (fc / f) ** (2 * order))


def tilt_mag(f, db_per_octave, ref=1000.0):
    return (np.maximum(f, 20.0) / ref) ** (db_per_octave / 6.0206)


def shape(x, gain):
    """`x` through the zero-phase filter whose magnitude is `gain(freqs)`."""
    m = pow2(2 * len(x))
    f = np.fft.rfftfreq(m, 1 / RATE)
    return np.fft.irfft(np.fft.rfft(x, m) * gain(f), m)[: len(x)]


def shaped_noise(rng, length, spectrum, frame=1024, hop=256):
    """Noise whose spectrum at time t is `spectrum(t, freqs)`: a short-time Fourier
    overlap-add, so the filter can move continuously (whooshes, swishes)."""
    count = n_of(length)
    total = count + frame
    noise = rng.standard_normal(total)
    window = np.hanning(frame + 1)[:-1]
    freqs = np.fft.rfftfreq(frame, 1 / RATE)
    out = np.zeros(total)
    norm = np.zeros(total)
    for start in range(0, total - frame + 1, hop):
        segment = np.fft.rfft(noise[start : start + frame] * window)
        segment *= spectrum(start / RATE, freqs)
        out[start : start + frame] += np.fft.irfft(segment, frame) * window
        norm[start : start + frame] += window**2
    out /= np.maximum(norm, 1e-9)
    return out[frame // 2 : frame // 2 + count]


def random_envelope(rng, length, rate_hz, depth):
    """A smooth random gain around 1 (log-normal), changing about `rate_hz` times a second."""
    count = n_of(length)
    points = max(4, int(length * rate_hz) + 4)
    knots = rng.standard_normal(points)
    curve = np.interp(np.linspace(0, points - 3, count), np.arange(points), knots)
    # Smooth the linear interpolation into gentle curves.
    width = max(1, n_of(0.5 / rate_hz))
    curve = np.convolve(curve, np.hanning(width + 2)[1:-1] / np.sum(np.hanning(width + 2)), "same")
    curve /= max(np.std(curve), 1e-9)
    return np.exp(depth * curve)


def bell(length, rise, fall_power=1.0):
    """0 → 1 → 0 over `length`: a sine-squared rise over `rise` of it, then a raised-cosine
    fall (to the power `fall_power`)."""
    t = np.linspace(0, 1, n_of(length), endpoint=False)
    up = np.sin(0.5 * np.pi * np.clip(t / rise, 0, 1)) ** 2
    down = (0.5 + 0.5 * np.cos(np.pi * np.clip((t - rise) / (1 - rise), 0, 1))) ** fall_power
    return up * down


# --- Modal synthesis --------------------------------------------------------------------


def plate_ratios(aspect, count=8):
    """Simply supported rectangular plate: f_mn ∝ (m/a)² + (n/b)², with b = a·aspect.
    Returns (ratio, m, n) of the lowest modes."""
    modes = sorted(
        (m * m + (n / aspect) ** 2, m, n) for m in range(1, 6) for n in range(1, 6)
    )[:count]
    base = modes[0][0]
    return [(f / base, m, n) for f, m, n in modes]


def ring_ratios(count=5):
    """Flexural modes of a thin ring or short cylindrical shell (bins, glasses, bells):
    f_n ∝ n(n² − 1)/√(n² + 1), n = 2, 3, …"""
    raw = [n * (n * n - 1) / np.sqrt(n * n + 1) for n in range(2, 2 + count)]
    return [r / raw[0] for r in raw]


# Free-free Euler–Bernoulli bar: (βL)² ratios.
FREE_BAR = [1.0, 2.756, 5.404, 8.933, 13.344]
# A bar thinned underneath to tune its overtones (marimba-like): 1 : 3.9 : 9.2.
TUNED_BAR = [1.0, 3.9, 9.2]


def stiff_string_ratios(stiffness, count=6):
    """A stiff wire (spring): f_n = n·√(1 + B·n²), normalised."""
    raw = [n * np.sqrt(1 + stiffness * n * n) for n in range(1, count + 1)]
    return [r / raw[0] for r in raw]


def modes_from(f0, ratios, amps, t60, damping_hz, split=0.0):
    """Mode list (freq, amp, t60): the decay time shrinks with frequency,
    t60(f) = t60 / (1 + (f/damping_hz)²), so the high modes die first. `split` detunes a
    twin of each mode (the degenerate pairs of round objects), giving a slow beat."""
    out = []
    for ratio, amp in zip(ratios, amps):
        f = f0 * ratio
        decay = t60 / (1 + (f / damping_hz) ** 2)
        if split:
            out.append((f * (1 - split / 2), amp * 0.6, decay))
            out.append((f * (1 + split / 2), amp * 0.4, decay))
        else:
            out.append((f, amp, decay))
    return out


def modal_ir(modes, length=None):
    """Impulse response of a bank of damped resonators: Σ a·e^(−6.9 t/t60)·sin(2πft)."""
    if length is None:
        length = 1.2 * max(t60 for _, _, t60 in modes)
    t = times(length)
    out = np.zeros_like(t)
    for f, amp, t60 in modes:
        if f < 0.45 * RATE:
            out += amp * np.exp(-6.9078 * t / t60) * np.sin(2 * np.pi * f * t)
    return out


def mallet(width):
    """A raised-cosine force pulse of `width` seconds: wider is softer (less treble)."""
    pulse = np.hanning(max(3, n_of(width)) + 2)[1:-1]
    return pulse / np.sum(pulse)


def burst(rng, length, decay, low=200.0, high=9000.0):
    """A short noise burst (scraping, a paper contact, a latch), band-limited."""
    t = times(length)
    attack = np.clip(t / 0.0003, 0, 1)
    noise = shape(rng.standard_normal(len(t)), lambda f: highpass_mag(f, low) * lowpass_mag(f, high))
    return noise * attack * np.exp(-t / decay)


def strike(excitation, modes, length=None):
    return convolve(excitation, modal_ir(modes, length))


# --- Granular paper ---------------------------------------------------------------------


def paper(
    rng,
    length,
    density,
    rate,
    center,
    spread=1.0,
    q=1.6,
    grain=(0.002, 0.009),
    crackle=0.08,
    alpha=1.7,
    bed=0.2,
):
    """Paper rustle or crumple by granular synthesis.

    density(t) in [0, 1] scales the grain `rate` (grains/s); center(t) is the grains' mean
    band centre (Hz), drawn log-normally ±`spread` octaves around it, so a falling center
    is the spectral tilt darkening as the paper folds into a ball. Each grain's energy
    follows a power law (exponent `alpha`), as crackling noise does; a fraction `crackle`
    of the grains are short, wide-band, louder snaps. A quiet `bed` of continuous friction
    noise follows the same density, with a slow random wobble."""
    total = length + 0.03
    out = np.zeros(n_of(total))
    peak_rate = rate
    t = 0.0
    while True:
        t += rng.exponential(1 / peak_rate)
        if t >= length:
            break
        d = density(t)
        if rng.uniform() > d:
            continue
        energy = min((1 - rng.uniform()) ** (-1 / alpha), 12.0)
        if rng.uniform() < crackle:
            size = rng.uniform(0.0004, 0.0016)
            fc = center(t) * 2 ** rng.uniform(0.3, 1.3)
            noise = shape(
                rng.standard_normal(n_of(size) + 8),
                lambda f: bandpass_mag(f, fc, 0.6) * highpass_mag(f, 900) * lowpass_mag(f, 10000, 4),
            )
            n = len(noise)
            env = np.exp(-np.arange(n) / (0.35 * n)) * np.clip(np.arange(n) / 8, 0, 1)
            amp = 2.2 * np.sqrt(energy)
        else:
            size = rng.uniform(*grain)
            fc = center(t) * 2 ** (spread * rng.standard_normal() * 0.5)
            fc = np.clip(fc, 400, 15000)
            noise = shape(
                rng.standard_normal(n_of(size) + 8),
                lambda f: bandpass_mag(f, fc, q * rng.uniform(0.7, 1.4)) ** 2 * lowpass_mag(f, 9000),
            )
            env = np.hanning(len(noise))
            amp = np.sqrt(energy)
        out += place(total, unit(noise) * env, t, amp * (0.35 + 0.65 * d))
    if bed:
        noise = shaped_noise(
            rng,
            total,
            lambda tt, f: bandpass_mag(f, center(min(tt, length)), 0.9) ** 2 * lowpass_mag(f, 8000),
        )
        envelope = np.array([density(min(x, length)) for x in times(total)])
        envelope *= random_envelope(rng, total, 30, 0.35)
        bed_signal = unit(noise * envelope) * bed * np.max(np.abs(out)) * 0.5
        out += bed_signal
    return out


# --- Room and finishing -----------------------------------------------------------------


def room_ir(rng, length=0.16):
    """A small room: eight early reflections 2–14 ms after the direct sound, then a short
    darkening diffuse tail (t60 ≈ 0.2 s). Energy normalised to 1."""
    ir = np.zeros(n_of(length))
    for delay in np.sort(rng.uniform(0.0022, 0.014, 8)):
        start = n_of(delay)
        smear = mallet(0.0003) * rng.choice([-1, 1]) * 0.7 * np.exp(-delay / 0.008)
        ir[start : start + len(smear)] += smear * len(smear)
    t = times(length)
    tail = rng.standard_normal(len(t)) * np.exp(-6.9078 * t / 0.2)
    tail *= np.clip((t - 0.006) / 0.008, 0, 1)
    tail = shape(tail, lambda f: lowpass_mag(f, 4500, 1) * highpass_mag(f, 150))
    ir += 0.25 * tail
    return ir / np.sqrt(np.sum(ir**2))


def biquad(x, b, a):
    out = np.zeros_like(x)
    x1 = x2 = y1 = y2 = 0.0
    b0, b1, b2 = b
    _, a1, a2 = a
    for i, v in enumerate(x):
        y = b0 * v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2
        x2, x1, y2, y1 = x1, v, y1, y
        out[i] = y
    return out


def loudness(x):
    """Maximum momentary loudness (LUFS): ITU-R BS.1770 K-weighting (48 kHz coefficients),
    mean square over 400 ms windows every 100 ms. Short sounds are measured in one window
    padded with silence."""
    y = biquad(
        x,
        (1.53512485958697, -2.69169618940638, 1.19839281085285),
        (1.0, -1.69065929318241, 0.73248077421585),
    )
    y = biquad(y, (1.0, -2.0, 1.0), (1.0, -1.99004745483398, 0.99007225036621))
    window, hop = n_of(0.4), n_of(0.1)
    if len(y) < window:
        y = np.concatenate([y, np.zeros(window - len(y))])
    power = max(np.mean(y[s : s + window] ** 2) for s in range(0, len(y) - window + 1, hop))
    return -0.691 + 10 * np.log10(max(power, 1e-20))


def limit(x, ceiling):
    """A look-ahead peak limiter: the gain reaches its reduction 1.5 ms before a peak
    (smooth, no distortion), then recovers with a 40 ms release."""
    look = n_of(0.0015)
    need = np.minimum(1.0, ceiling / np.maximum(np.abs(x), 1e-12))
    ahead = sliding_window_view(np.concatenate([need, np.ones(look)]), look + 1).min(axis=1)
    # Before the start the gain is already where the first samples need it.
    lead = np.full(look, ahead[0])
    smooth = np.convolve(np.concatenate([lead, ahead]), np.ones(look + 1) / (look + 1), "valid")
    release = 1 - np.exp(-1 / (0.040 * RATE))
    gain = np.empty_like(x)
    g = smooth[0]
    for i, s in enumerate(smooth):
        g = min(s, g + (1 - g) * release)
        gain[i] = g
    return np.clip(x * gain, -ceiling, ceiling)


def finish(signal, rng, room=ROOM_MIX):
    """Room, DC removal, click-free ends, loudness and peak normalisation."""
    signal = np.asarray(signal, dtype=float)
    if room:
        wet = convolve(signal, room_ir(rng))
        signal = np.concatenate([signal, np.zeros(len(wet) - len(signal))]) + room * wet
    # DC and rumble out: a gentle zero-phase high-pass at 30 Hz.
    signal = shape(signal - np.mean(signal), lambda f: highpass_mag(f, 30))
    # Cut where the sound has fallen 60 dB below its peak, then fade to true silence.
    level = np.convolve(np.abs(signal), np.ones(n_of(0.005)) / n_of(0.005), "same")
    loud = np.nonzero(level > np.max(level) * 10 ** (-60 / 20))[0]
    end = min(len(signal), loud[-1] + n_of(0.005))
    signal = signal[:end]
    fade = min(n_of(0.06), len(signal) // 4)
    signal[-fade:] *= 0.5 + 0.5 * np.cos(np.linspace(0, np.pi, fade))
    signal[-1] = 0.0
    rise = n_of(0.0005)
    signal[:rise] *= np.sin(0.5 * np.pi * np.linspace(0, 1, rise)) ** 2
    signal[0] = 0.0
    # Loudness to TARGET_LUFS with peaks under CEILING_DBFS.
    ceiling = 10 ** (CEILING_DBFS / 20) * 0.995
    signal *= 10 ** ((TARGET_LUFS - loudness(signal)) / 20)
    peak = np.max(np.abs(signal))
    if peak > ceiling * 10 ** (MAX_LIMIT_DB / 20):
        signal *= ceiling * 10 ** (MAX_LIMIT_DB / 20) / peak
    return limit(signal, ceiling)


# --- The sounds -------------------------------------------------------------------------


def plastic_bin(f0, t60, aspect=1.35, strike_at=(0.31, 0.43)):
    """A moulded plastic bin wall: plate modes weighted by the strike position (mode shape
    sin(mπx)·sin(nπy)), heavily damped."""
    x, y = strike_at
    ratios = plate_ratios(aspect)
    amps = [abs(np.sin(m * np.pi * x) * np.sin(n * np.pi * y)) / r**0.3 for r, m, n in ratios]
    return modes_from(f0, [r for r, _, _ in ratios], amps, t60, 2200)


def metal_bin(f0, t60):
    """A thin steel bin (a short cylindrical shell): ring modes with split pairs."""
    ratios = ring_ratios(5)
    amps = [1.0, 0.7, 0.45, 0.3, 0.2]
    return modes_from(f0, ratios, amps, t60, 5000, split=0.004)


def landing_thump(rng, f0, t60, softness=0.004):
    """Paper settling on a bin bottom: a soft noisy contact driving a damped plastic body."""
    contact = burst(rng, 0.02, 0.004, 80, 1500) * np.hanning(n_of(0.02))
    contact = convolve(contact, mallet(softness))
    return unit(strike(contact, plastic_bin(f0, t60, 1.6, (0.5, 0.5))))


def whoosh(rng, length, path, q, flutter=0.0, flutter_rate=20.0, rise=0.4, tilt=-2.0):
    """Air displaced by a moving sheet: noise through a band-pass that follows `path`
    (centre frequencies across the gesture), under a bell envelope; `flutter` adds the
    irregular flapping of a sheet."""
    centres = np.asarray(path, dtype=float)

    def centre(t):
        return np.exp(np.interp(t / length, np.linspace(0, 1, len(centres)), np.log(centres)))

    noise = shaped_noise(
        rng,
        length,
        lambda t, f: bandpass_mag(f, centre(t), q) * tilt_mag(f, tilt) * lowpass_mag(f, 7000),
    )
    env = bell(length, rise, 1.4)
    if flutter:
        env *= 1 + flutter * np.sin(2 * np.pi * flutter_rate * times(length) + 6 * random_envelope(rng, length, 8, 0.4))
    return unit(noise * env)


def trash_move_a(rng):
    total = 0.62
    air = whoosh(rng, 0.34, [900, 2600, 1500], 0.8, flutter=0.15, flutter_rate=23)
    land = paper(rng, 0.16, lambda t: np.exp(-t / 0.05), 1800, lambda t: 3200 - 4000 * t, crackle=0.05)
    thump = landing_thump(rng, 190, 0.09)
    return mix(total, (air, 0.0, 1.0), (unit(land), 0.27, 0.55), (thump, 0.28, 0.22))


def trash_move_b(rng):
    total = 0.5
    air = whoosh(rng, 0.21, [2000, 4200, 2600], 1.2, rise=0.35, tilt=-1.0)
    land = paper(rng, 0.11, lambda t: np.exp(-t / 0.03), 2600, lambda t: 4200, crackle=0.12)
    exc = convolve(burst(rng, 0.006, 0.0012, 300, 7000), mallet(0.0006))
    ring = unit(strike(exc, metal_bin(560, 0.35)))
    return mix(total, (air, 0.0, 1.0), (unit(land), 0.165, 0.7), (ring, 0.172, 0.12))


def trash_move_c(rng):
    total = 0.72
    air = whoosh(rng, 0.42, [600, 1400, 800], 0.9, flutter=0.5, flutter_rate=16, rise=0.45, tilt=-3.0)
    land = paper(rng, 0.14, lambda t: np.exp(-t / 0.045), 1500, lambda t: 1900, crackle=0.02, bed=0.35)
    thump = landing_thump(rng, 125, 0.11, softness=0.007)
    return mix(total, (air, 0.0, 1.0), (unit(land), 0.33, 0.45), (thump, 0.335, 0.35))


def trash_drag_a(rng):
    total = 0.36
    rustle = paper(rng, 0.11, lambda t: np.sin(np.pi * t / 0.11) ** 1.5, 2000, lambda t: 3400 - 6000 * t, crackle=0.06)
    tap = unit(strike(mallet(0.00035), plastic_bin(420, 0.10)))
    return mix(total, (unit(rustle), 0.0, 1.0), (tap, 0.075, 0.42))


def trash_drag_b(rng):
    total = 0.48
    rustle = paper(rng, 0.10, lambda t: np.sin(np.pi * t / 0.10) ** 1.5, 2000, lambda t: 3600, crackle=0.08)
    tap = unit(strike(mallet(0.0002), metal_bin(780, 0.4)))
    return mix(total, (unit(rustle), 0.0, 1.0), (tap, 0.07, 0.22))


def trash_drag_c(rng):
    total = 0.32
    rustle = paper(rng, 0.08, lambda t: np.sin(np.pi * t / 0.08) ** 2, 1600, lambda t: 2800, crackle=0.03, bed=0.3)
    wood = modes_from(600, FREE_BAR, [1.0, 0.5, 0.25, 0.12, 0.06], 0.07, 3000)
    cavity = [(260, 0.5, 0.05)]  # the air inside the bin, a Helmholtz-like low mode
    tock = unit(strike(mallet(0.0005), wood + cavity))
    return mix(total, (unit(rustle), 0.0, 0.85), (tock, 0.055, 0.5))


def crumple_density(rng, length, attack, decay_from):
    """Bursts of crackling: an overall rise and fall times a fast random envelope."""
    wobble = random_envelope(rng, length + 0.05, 25, 0.6)

    def density(t):
        rise = np.clip(t / attack, 0, 1)
        fall = np.clip(1 - (t - decay_from) / (length - decay_from), 0, 1) ** 1.5 if t > decay_from else 1
        return float(np.clip(0.7 * rise * fall * wobble[min(len(wobble) - 1, n_of(t))], 0, 1))

    return density


def trash_empty_a(rng):
    length = 0.62
    density = crumple_density(rng, length, 0.04, 0.3)
    body = paper(rng, length, density, 1500, lambda t: 4300 * (2600 / 4300) ** (t / length), spread=1.2, crackle=0.12, alpha=1.6, bed=0.22)
    return body


def trash_empty_b(rng):
    length = 0.46
    density = crumple_density(rng, length, 0.025, 0.25)
    body = paper(rng, length, density, 2000, lambda t: 4600 * (3000 / 4600) ** (t / length), spread=1.0, crackle=0.15, alpha=1.5, bed=0.18)
    drop = landing_thump(rng, 160, 0.1, softness=0.003)
    return mix(0.58, (unit(body), 0.0, 1.0), (drop, 0.47, 0.3))


def trash_empty_c(rng):
    length = 0.74
    density = crumple_density(rng, length, 0.07, 0.35)
    return paper(rng, length, density, 1100, lambda t: 2300 * (1400 / 2300) ** (t / length), spread=0.9, q=1.2, grain=(0.004, 0.014), crackle=0.03, alpha=2.2, bed=0.4)


def glass(f0, t60=0.7):
    """A small glass bowl: ring modes, split pairs, fundamental dominant."""
    return modes_from(f0, ring_ratios(5), [1.0, 0.32, 0.12, 0.05, 0.02], t60, 3800, split=0.0025)


def file_transfer_a(rng):
    total = 0.85
    first = unit(strike(mallet(0.0006), glass(1175)))  # D6
    second = unit(strike(mallet(0.0006), glass(1568)))  # G6, a fourth up
    return mix(total, (first, 0.0, 0.8), (second, 0.085, 1.0))


def file_transfer_b(rng):
    total = 0.55
    air = whoosh(rng, 0.24, [1400, 3200, 5200], 1.4, rise=0.55, tilt=-1.0)
    tink = unit(strike(mallet(0.0004), glass(2350, 0.4)))
    return mix(total, (air, 0.0, 1.0), (tink, 0.17, 0.3))


def file_transfer_c(rng):
    total = 0.55
    bar = lambda f0: modes_from(f0, TUNED_BAR, [1.0, 0.18, 0.05], 0.24, 2600)  # noqa: E731
    first = unit(strike(mallet(0.0009), bar(784)))  # G5
    second = unit(strike(mallet(0.0009), bar(1175)))  # D6, a fifth up
    return mix(total, (first, 0.0, 0.75), (second, 0.065, 1.0))


def shutter_click(rng, gain, blade_f0, body_f0, body_t60, band, blade_mix):
    """One shutter transient: a filtered noise impact driving a tiny steel blade (plate
    modes) and the damped plastic housing."""
    hit = burst(rng, 0.008, 0.0016, 400, 8000)
    hit = shape(hit, lambda f: bandpass_mag(f, band, 0.7) + 0.1)
    # The zero-phase filter rings ahead of the onset: re-open it smoothly (no step).
    hit[: n_of(0.0004)] *= np.sin(0.5 * np.pi * np.linspace(0, 1, n_of(0.0004))) ** 2
    blade = modes_from(blade_f0, [r for r, _, _ in plate_ratios(2.2, 6)], [1, 0.7, 0.5, 0.35, 0.25, 0.18], 0.07, 9000)
    housing = plastic_bin(body_f0, body_t60, 1.2, (0.4, 0.3))
    parts = [0.6 * unit(hit), blade_mix * unit(strike(hit, blade)), 0.9 * unit(strike(mallet(0.0008), housing))]
    out = np.zeros(max(len(p) for p in parts))
    for p in parts:
        out[: len(p)] += p
    return gain * unit(out)


def blade_travel(rng, length, centre):
    """The blades sliding between the two clicks: a faint band of friction noise."""
    noise = shaped_noise(rng, length, lambda t, f: bandpass_mag(f, centre, 1.2) ** 2)
    return unit(noise * bell(length, 0.3))


def screen_capture_a(rng):
    total = 0.24
    open_ = shutter_click(rng, 1.0, 2900, 380, 0.05, 3500, 0.4)
    close = shutter_click(rng, 0.8, 2700, 350, 0.05, 3100, 0.4)
    travel = blade_travel(rng, 0.055, 2200)
    return mix(total, (open_, 0.0, 1.0), (travel, 0.006, 0.12), (close, 0.06, 1.0))


def screen_capture_b(rng):
    total = 0.18
    open_ = shutter_click(rng, 1.0, 4200, 520, 0.05, 5200, 0.45)
    close = shutter_click(rng, 0.85, 4000, 500, 0.05, 4800, 0.45)
    travel = blade_travel(rng, 0.032, 3000)
    return mix(total, (open_ * 0.9, 0.0, 1.0), (travel, 0.003, 0.18), (close, 0.035, 1.0))


def screen_capture_c(rng):
    total = 0.32
    mirror_up = shutter_click(rng, 1.0, 2400, 220, 0.05, 2400, 0.3)
    bounce = shutter_click(rng, 0.45, 2400, 220, 0.04, 2800, 0.3)
    down = shutter_click(rng, 0.85, 2200, 200, 0.05, 2200, 0.3)
    travel = blade_travel(rng, 0.08, 1600)
    spring = modes_from(1900, stiff_string_ratios(0.002), [1, 0.5, 0.3, 0.2, 0.1, 0.05], 0.12, 7000)
    ring = unit(strike(mallet(0.0003), spring))
    return mix(total, (mirror_up, 0.0, 1.0), (bounce, 0.004, 1.0), (ring, 0.002, 0.08), (travel, 0.01, 0.1), (down, 0.09, 1.0))


SOUNDS = {
    "trash-move": {
        "A": (trash_move_a, "Airy sheet: a passing whoosh with slight flutter, a soft paper rustle landing on a plastic bin bottom."),
        "B": (trash_move_b, "Crisp and quick: a brighter, shorter whoosh, a crackly rustle and a faint ring of a steel mesh bin."),
        "C": (trash_move_c, "Soft flutter: a slower, darker whoosh of a flapping sheet, a muffled rustle and a deeper thump."),
    },
    "trash-drag": {
        "A": (trash_drag_a, "A short soft rustle, then a light tap on a moulded plastic bin."),
        "B": (trash_drag_b, "A short rustle, then a tiny tap on a thin steel bin that rings briefly."),
        "C": (trash_drag_c, "A lighter rustle, then a hollow woody tock with the bin's air cavity."),
    },
    "trash-empty": {
        "A": (trash_empty_a, "Crisp crumple, about 0.6 s: crackling bursts that darken as the ball compacts."),
        "B": (trash_empty_b, "Tight and quick crumple, about 0.5 s, ending with the ball dropping softly into the bin."),
        "C": (trash_empty_c, "Soft tissue-like crumple, about 0.75 s: smoother, darker, few crackles."),
    },
    "file-transfer": {
        "A": (file_transfer_a, "Two-note glass chime (D6 then G6, a fourth up) from small glass bowl modes."),
        "B": (file_transfer_b, "Airy rising swish ending on a faint glass tink."),
        "C": (file_transfer_c, "Two-note soft woody pop from tuned-bar modes (G5 then D6)."),
    },
    "screen-capture": {
        "A": (screen_capture_a, "Shutter double click, 60 ms apart: filtered transients, a tiny steel blade body, light housing."),
        "B": (screen_capture_b, "Leaf shutter: a tighter, brighter double click 35 ms apart, little body."),
        "C": (screen_capture_c, "Reflex-camera style: a heavier first click with a bounce and a spring ring, then the return 90 ms later."),
    },
}


# --- Output -----------------------------------------------------------------------------


def write(path, signal):
    samples = np.clip(np.round(signal * 32767), -32768, 32767).astype("<i2")
    with wave.open(str(path), "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(samples.tobytes())


def spectrogram(path, signal, title):
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, (wave_ax, spec_ax) = plt.subplots(2, 1, figsize=(8, 5), sharex=True, height_ratios=[1, 3])
    t = np.arange(len(signal)) / RATE
    wave_ax.plot(t, signal, linewidth=0.5)
    wave_ax.set_ylim(-0.6, 0.6)
    wave_ax.set_title(title)
    spec_ax.specgram(signal, NFFT=512, Fs=RATE, noverlap=448, cmap="magma", vmin=-140)
    spec_ax.set_ylabel("Hz")
    spec_ax.set_xlabel("s")
    fig.tight_layout()
    fig.savefig(path, dpi=90)
    plt.close(fig)


def main():
    root = Path(__file__).resolve().parents[2]
    candidates = root / "tools" / "sounds" / "candidates"
    shipped = root / "data" / "sounds"
    candidates.mkdir(parents=True, exist_ok=True)
    shipped.mkdir(parents=True, exist_ok=True)
    index = {}
    for name, variants in SOUNDS.items():
        index[name] = {"chosen": CHOSEN[name], "candidates": {}}
        for variant, (make, description) in variants.items():
            # One generator per candidate: changing one leaves the others identical.
            rng = np.random.default_rng([SEED, sum(name.encode()), ord(variant)])
            signal = finish(make(rng), rng)
            path = candidates / f"{name}-{variant}.wav"
            write(path, signal)
            index[name]["candidates"][variant] = {
                "file": f"{name}-{variant}.wav",
                "description": description,
            }
            peak = 20 * np.log10(np.max(np.abs(signal)))
            print(
                f"{path.name}: {len(signal) / RATE * 1000:.0f} ms, "
                f"{loudness(signal):.1f} LUFS, peak {peak:.1f} dBFS"
            )
            if "--spectrograms" in sys.argv:
                spectrogram(path.with_suffix(".png"), signal, f"{name} {variant}")
        shutil.copyfile(candidates / f"{name}-{CHOSEN[name]}.wav", shipped / f"{name}.wav")
    (candidates / "index.json").write_text(json.dumps(index, indent=2) + "\n")


if __name__ == "__main__":
    main()
