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

Writes the candidates of each sound (A, B, C, the refinements B1–B4 and the role designs
C1–C4) to tools/sounds/candidates/<name>-<variant>.wav with an index.json describing them,
and copies the chosen candidate (CHOSEN; ALIASES for sounds that share another's file) to
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
# trash-drag has no choice of its own: it ships trash-move's file (ALIASES).
CHOSEN = {
    "trash-move": "C1",
    "trash-empty": "C1",
    "file-transfer": "C1",
    "screen-capture": "C1",
    "alert-crystal": "A",
    "alert-woodblock": "A",
    "alert-drop": "A",
    "alert-pebble": "A",
    "volume-feedback": "A",
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


def finish(signal, rng, room=ROOM_MIX, max_limit_db=MAX_LIMIT_DB):
    """Room, DC removal, click-free ends, loudness and peak normalisation."""
    signal = np.asarray(signal, dtype=float)
    if room:
        wet = convolve(signal, room_ir(rng))
        signal = np.concatenate([signal, np.zeros(len(wet) - len(signal))]) + room * wet
    # DC and rumble out: a gentle zero-phase high-pass at 30 Hz.
    signal = shape(signal - np.mean(signal), lambda f: highpass_mag(f, 30))
    # Cut where the sound has fallen 60 dB below its peak, then fade to true silence.
    # The last 20 ms are left out of the search: subtracting the mean leaves a step at the
    # buffer's end that the high-pass turns into a tiny transient there, which is not sound.
    level = np.convolve(np.abs(signal), np.ones(n_of(0.005)) / n_of(0.005), "same")[: -n_of(0.02)]
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
    if peak > ceiling * 10 ** (max_limit_db / 20):
        signal *= ceiling * 10 ** (max_limit_db / 20) / peak
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


# --- Alerts and feedback ------------------------------------------------------------------


def alert_crystal(rng):
    """Two small glass bowls struck together, a fifth apart (E6 and B6)."""
    total = 0.9
    low = unit(strike(mallet(0.0005), glass(1319, 0.8)))
    high = unit(strike(mallet(0.0005), glass(1976, 0.6)))
    return mix(total, (low, 0.0, 1.0), (high, 0.012, 0.55))


def alert_woodblock(rng):
    """Two quick knocks on a tuned wooden bar (A5), the second softer."""
    total = 0.35
    bar = modes_from(880, TUNED_BAR, [1.0, 0.22, 0.06], 0.16, 2400)
    knock = unit(strike(mallet(0.0011), bar))
    return mix(total, (knock, 0.0, 1.0), (knock, 0.11, 0.6))


def alert_drop(rng):
    """A water-drop like resonance: a damped sine whose pitch rises fast, as a bubble's does."""
    t = times(0.22)
    f = 900 * (1 + 0.9 * (1 - np.exp(-t / 0.035)))
    phase = 2 * np.pi * np.cumsum(f) / RATE
    body = np.sin(phase) * np.exp(-t / 0.05)
    tick = unit(burst(rng, 0.004, 0.0015, 2000, 9000))
    return mix(0.25, (body, 0.0, 1.0), (tick, 0.0, 0.15))


def alert_pebble(rng):
    """A pebble on a small steel dish: ring modes with a short, bright decay."""
    dish = modes_from(2093, ring_ratios(5), [1.0, 0.5, 0.25, 0.1, 0.05], 0.35, 6000, split=0.003)
    return unit(strike(mallet(0.0003), dish))


def volume_feedback(rng):
    """The volume keys' feedback: one soft, short tap on a tuned bar (E6)."""
    bar = modes_from(1319, TUNED_BAR, [1.0, 0.12, 0.03], 0.16, 2600)
    return unit(strike(mallet(0.003), bar))


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
    "alert-crystal": {
        "A": (alert_crystal, "Alert: two small glass bowls struck together, a fifth apart (E6, B6)."),
    },
    "alert-woodblock": {
        "A": (alert_woodblock, "Alert: two quick knocks on a tuned wooden bar (A5), the second softer."),
    },
    "alert-drop": {
        "A": (alert_drop, "Alert: a bubble-like resonance whose pitch rises quickly, with a faint tick."),
    },
    "alert-pebble": {
        "A": (alert_pebble, "Alert: a pebble on a small steel dish, bright and short."),
    },
    "volume-feedback": {
        "A": (volume_feedback, "Volume keys: one soft, short tap on a tuned bar (E6)."),
    },
    "screen-capture": {
        "A": (screen_capture_a, "Shutter double click, 60 ms apart: filtered transients, a tiny steel blade body, light housing."),
        "B": (screen_capture_b, "Leaf shutter: a tighter, brighter double click 35 ms apart, little body."),
        "C": (screen_capture_c, "Reflex-camera style: a heavier first click with a bounce and a spring ring, then the return 90 ms later."),
    },
}


# --- Refinements of the B candidates ----------------------------------------------------


def paper_grain(rng, centre, size, sigma, q):
    """One fibre event: a short resonant tick of band-passed noise with an impulsive
    envelope (0.1 ms rise, exponential fall), not a smooth window: paper snaps, it does not
    hiss. Sizes are log-normal around `size`."""
    length = float(np.clip(size * np.exp(sigma * rng.standard_normal()), 0.00025, 0.008))
    fc = float(np.clip(centre * 2 ** (0.55 * rng.standard_normal()), 900, 13000))
    width = q * rng.uniform(0.7, 1.4)
    count = n_of(length * 1.6) + 8
    t = np.arange(count) / RATE
    env = np.clip(t / 0.0001, 0, 1) * np.exp(-t / max(length / 3, 0.0002))
    # Filtered after the envelope, so the snap itself stays in band (no wide-band click);
    # the lead-in leaves room for the zero-phase filter's ringing, which fades in from zero.
    lead = n_of(0.001)
    tick = shape(
        np.concatenate([np.zeros(lead), rng.standard_normal(count) * env]),
        lambda f: bandpass_mag(f, fc, width) ** 2 * lowpass_mag(f, 11000, 4),
    )
    tick[:lead] *= np.sin(0.5 * np.pi * np.linspace(0, 1, lead)) ** 2
    return unit(tick)


def fibre_sheen(rng, length, density, level):
    """The fine high-frequency glitter of paper fibres: 4.5–11 kHz noise gated by a sparse,
    spiky random envelope, following the density."""
    count = n_of(length)
    noise = shape(rng.standard_normal(count), lambda f: highpass_mag(f, 4500, 4) * lowpass_mag(f, 11000, 4))
    spikes = shape(rng.standard_normal(count), lambda f: lowpass_mag(f, 400, 2)) ** 4
    spikes /= max(np.max(spikes), 1e-12)
    dens = np.array([density(x) for x in times(length)])
    return level * unit(noise) * spikes * dens


def paper_fine(
    rng,
    length,
    density,
    rate,
    centre,
    size=0.0016,
    sigma=0.6,
    q=2.4,
    cluster=0.25,
    crackle=0.05,
    alpha=1.8,
    sheen=0.25,
    bed=0.05,
):
    """Finer granular paper: impulsive resonant ticks with log-normal sizes, power-law
    energies, clusters of micro-crackles (a few ticks 0.3–2 ms apart, fibres giving way in
    a row), a few wide-band crackles, the fibres' high sheen, and only a trace of the
    continuous friction bed (less hiss)."""
    total = length + 0.03
    out = np.zeros(n_of(total))
    t = 0.0
    while True:
        t += rng.exponential(1 / rate)
        if t >= length:
            break
        d = density(t)
        if rng.uniform() > d:
            continue
        amp = np.sqrt(min((1 - rng.uniform()) ** (-1 / alpha), 10.0)) * (0.4 + 0.6 * d)
        if rng.uniform() < crackle:
            tick = paper_grain(rng, centre(t) * 1.4, 0.0005, 0.3, 1.0)
            out += place(total, tick, t, 2.0 * amp)
            continue
        count = int(rng.integers(2, 6)) if rng.uniform() < cluster else 1
        at = t
        for k in range(count):
            out += place(total, paper_grain(rng, centre(t), size, sigma, q), at, amp * 0.75**k)
            at += rng.uniform(0.0003, 0.002)
    level = np.max(np.abs(out))
    if sheen:
        out += place(total, fibre_sheen(rng, length, density, sheen * level), 0)
    if bed:
        noise = shaped_noise(
            rng, total, lambda tt, f: bandpass_mag(f, 0.6 * centre(min(tt, length)), 0.8) ** 2 * lowpass_mag(f, 6000)
        )
        envelope = np.array([density(min(x, length)) for x in times(total)])
        out += bed * level * unit(noise * envelope * random_envelope(rng, total, 30, 0.35))
    return out


def decaying(tau):
    return lambda t: float(np.exp(-t / tau))


def swelling(length, power=1.5):
    return lambda t: float(np.sin(np.pi * min(t / length, 1.0)) ** power)


def flop(rng, length=0.03, cutoff=1600.0):
    """A sheet or a ball meeting a surface: a soft low-mid puff of compressed air and
    contact, 2 ms rise, ~8 ms fall."""
    t = times(length)
    noise = shape(rng.standard_normal(len(t)), lambda f: lowpass_mag(f, cutoff, 2) * highpass_mag(f, 120))
    env = np.clip(t / 0.002, 0, 1) * np.exp(-t / 0.008)
    return unit(noise * env)


def smooth_whoosh(rng, length, path, q, rise=0.4, tilt=-1.5, top=7000.0, order=2):
    """A smoother whoosh: a steeper (fourth-order) moving band and a firmer top cut, so it
    reads as air rather than broadband hiss."""
    centres = np.asarray(path, dtype=float)

    def centre(t):
        return np.exp(np.interp(t / length, np.linspace(0, 1, len(centres)), np.log(centres)))

    noise = shaped_noise(
        rng,
        length,
        lambda t, f: bandpass_mag(f, centre(t), q) ** order * tilt_mag(f, tilt) * lowpass_mag(f, top, 3),
        frame=2048,
        hop=256,
    )
    return unit(noise * bell(length, rise, 1.6))


def steel_tap(f0, t60, width=0.00025):
    return unit(strike(mallet(width), metal_bin(f0, t60)))


# trash-move: B was a bright whoosh, a crackly landing and a faint steel bin ring.


def trash_move_b1(rng):
    air = smooth_whoosh(rng, 0.17, [2200, 4200, 2800], 1.1, rise=0.35)
    land = paper_fine(rng, 0.07, decaying(0.02), 2400, lambda t: 4500, size=0.0012, sheen=0.3)
    return mix(0.35, (air, 0.0, 1.0), (unit(land), 0.14, 0.6), (flop(rng), 0.14, 0.12), (steel_tap(560, 0.3), 0.145, 0.06))


def trash_move_b2(rng):
    air = smooth_whoosh(rng, 0.23, [1900, 4000, 2500], 1.0, rise=0.35)
    # The sheet crinkles a little in flight.
    flight = paper_fine(rng, 0.2, swelling(0.2), 500, lambda t: 4200, cluster=0.1, sheen=0.15, bed=0)
    land = paper_fine(rng, 0.15, decaying(0.045), 3600, lambda t: 4000 - 6000 * t, cluster=0.35, crackle=0.07, sheen=0.3)
    return mix(
        0.5,
        (air, 0.0, 1.0),
        (unit(flight), 0.01, 0.18),
        (unit(land), 0.18, 0.75),
        (flop(rng), 0.18, 0.2),
        (steel_tap(560, 0.35), 0.186, 0.08),
    )


def trash_move_b3(rng):
    air = smooth_whoosh(rng, 0.19, [2400, 4400, 3000], 1.3, rise=0.4, tilt=-1.0)
    land = paper_fine(rng, 0.1, decaying(0.028), 3000, lambda t: 4800, size=0.0011, sigma=0.5, q=3.0, sheen=0.3, bed=0)
    return mix(0.4, (air, 0.0, 0.85), (unit(land), 0.15, 0.8), (flop(rng, 0.025, 2200), 0.15, 0.15))


def trash_move_b4(rng):
    air = smooth_whoosh(rng, 0.21, [2000, 4200, 2600], 1.2, rise=0.35)
    land = paper_fine(rng, 0.1, decaying(0.03), 3000, lambda t: 4200, sheen=0.25)
    rim = unit(strike(convolve(burst(rng, 0.004, 0.0008, 300, 8000), mallet(0.0003)), metal_bin(600, 0.35)))
    return mix(0.5, (air, 0.0, 1.0), (unit(land), 0.165, 0.65), (flop(rng), 0.165, 0.18), (rim, 0.17, 0.2))


# trash-drag: B was a short rustle and a tiny ringing steel tap.


def trash_drag_b1(rng):
    rustle = paper_fine(rng, 0.07, swelling(0.07), 2600, lambda t: 4400, size=0.0012, sheen=0.3)
    return mix(0.3, (unit(rustle), 0.0, 1.0), (steel_tap(780, 0.32, 0.0002), 0.05, 0.18))


def trash_drag_b2(rng):
    rustle = paper_fine(rng, 0.12, swelling(0.12, 1.2), 3600, lambda t: 4000 - 4000 * t, cluster=0.35, crackle=0.07, sheen=0.3)
    return mix(0.45, (unit(rustle), 0.0, 1.0), (flop(rng, 0.025), 0.075, 0.15), (steel_tap(760, 0.4), 0.08, 0.2))


def trash_drag_b3(rng):
    rustle = paper_fine(rng, 0.09, swelling(0.09), 3000, lambda t: 4800, size=0.0011, sigma=0.5, q=3.0, sheen=0.3, bed=0)
    return mix(0.35, (unit(rustle), 0.0, 1.0), (steel_tap(800, 0.22, 0.0002), 0.065, 0.16))


def trash_drag_b4(rng):
    rustle = paper_fine(rng, 0.09, swelling(0.09), 3000, lambda t: 4200, sheen=0.25)
    contact = convolve(burst(rng, 0.004, 0.0007, 300, 8000), mallet(0.00025))
    tap = unit(strike(contact, metal_bin(780, 0.35)))
    return mix(0.45, (unit(rustle), 0.0, 1.0), (flop(rng, 0.02, 2000), 0.06, 0.12), (tap, 0.065, 0.32))


# trash-empty: B was a tight, quick crumple ending with the ball dropping into the bin.


def crumple_fine(rng, length, rate, top, bottom, attack, decay_from, **kwargs):
    density = crumple_density(rng, length, attack, decay_from)
    return paper_fine(rng, length, density, rate, lambda t: top * (bottom / top) ** (t / length), **kwargs)


def trash_empty_b1(rng):
    body = crumple_fine(rng, 0.34, 3200, 4800, 3200, 0.02, 0.18, size=0.0012, sheen=0.3)
    return mix(0.45, (unit(body), 0.0, 1.0), (flop(rng, 0.03, 1400), 0.35, 0.18), (landing_thump(rng, 170, 0.08, 0.003), 0.352, 0.15))


def trash_empty_b2(rng):
    body = crumple_fine(rng, 0.5, 4600, 4600, 2900, 0.025, 0.28, cluster=0.4, crackle=0.07, sheen=0.3, bed=0.08)
    return mix(0.62, (unit(body), 0.0, 1.0), (flop(rng, 0.035, 1300), 0.5, 0.22), (landing_thump(rng, 160, 0.1, 0.003), 0.503, 0.2))


def trash_empty_b3(rng):
    body = crumple_fine(rng, 0.44, 3600, 5000, 3400, 0.02, 0.25, size=0.0011, sigma=0.5, q=3.0, sheen=0.3, bed=0)
    return mix(0.55, (unit(body), 0.0, 1.0), (flop(rng, 0.025, 1800), 0.45, 0.18))


def trash_empty_b4(rng):
    body = crumple_fine(rng, 0.44, 3800, 4600, 3000, 0.025, 0.25, sheen=0.25)
    contact = convolve(burst(rng, 0.006, 0.0012, 150, 5000), mallet(0.0008))
    drop = unit(strike(contact, metal_bin(420, 0.3)))
    return mix(0.62, (unit(body), 0.0, 1.0), (flop(rng, 0.03, 1400), 0.46, 0.2), (drop, 0.462, 0.3))


# file-transfer: B was an airy rising swish ending on a faint glass tink.


def harmonic_glass(f0, t60=0.5):
    """A tuned chime: modes tuned to near-harmonic ratios, fundamental strongest."""
    return modes_from(f0, [1.0, 2.005, 3.012], [1.0, 0.25, 0.06], t60, 5000, split=0.0015)


def file_transfer_b1(rng):
    air = smooth_whoosh(rng, 0.2, [1600, 3000, 4200], 1.6, rise=0.6)
    tink = unit(strike(mallet(0.0004), glass(2637, 0.35)))  # E7, glass bowl modes
    return mix(0.36, (air, 0.0, 1.0), (tink, 0.15, 0.45))


def file_transfer_b2(rng):
    air = smooth_whoosh(rng, 0.2, [1500, 2900, 4000], 1.6, rise=0.6)
    first = unit(strike(mallet(0.0005), harmonic_glass(1760, 0.3)))  # A6
    second = unit(strike(mallet(0.0005), harmonic_glass(2637, 0.35)))  # E7, a fifth up
    return mix(0.42, (air, 0.0, 1.0), (first, 0.14, 0.4), (second, 0.18, 0.32))


def file_transfer_b3(rng):
    air = smooth_whoosh(rng, 0.16, [1800, 3400, 4600], 1.8, rise=0.6)
    tink = unit(strike(mallet(0.0004), glass(2093, 0.3)))  # C7
    return mix(0.3, (air, 0.0, 0.75), (tink, 0.12, 0.7))


def file_transfer_b4(rng):
    # The swish narrows and settles on the chime's pitch before the strike.
    air = smooth_whoosh(rng, 0.2, [1400, 2400, 2349], 2.6, rise=0.65, tilt=-0.5)
    tink = unit(strike(mallet(0.0005), harmonic_glass(2349, 0.3)))  # D7
    return mix(0.42, (air, 0.0, 1.0), (tink, 0.16, 0.5))


# screen-capture: B was a tight, bright leaf-shutter double click.


def blade_snap(rng, blade_f0, body_f0, body_t60, spring_f0, spring, width=0.0003):
    """One blade transient: a very short metal impact (a narrow force pulse with a trace of
    contact noise) driving a thin steel blade (long-plate modes), the mechanism body and a
    small spring, so the energy sits in resonances rather than in a digital click."""
    pulse = mallet(width)
    hit = np.zeros(n_of(0.0015))
    hit[: len(pulse)] += pulse
    hit += 0.15 * np.max(pulse) * unit(burst(rng, 0.0015, 0.0003, 2000, 10000))
    blade = modes_from(blade_f0, [r for r, _, _ in plate_ratios(3.0, 6)], [1, 0.6, 0.4, 0.3, 0.2, 0.15], 0.03, 9000)
    body = modes_from(body_f0, [r for r, _, _ in plate_ratios(1.25, 6)], [1, 0.7, 0.5, 0.35, 0.25, 0.2], body_t60, 4000)
    coil = modes_from(spring_f0, stiff_string_ratios(0.003, 5), [1, 0.5, 0.3, 0.15, 0.08], 0.09, 8000)
    parts = [0.45 * unit(strike(hit, blade)), unit(strike(hit, body)), spring * unit(strike(hit, coil))]
    out = np.zeros(max(len(p) for p in parts))
    for p in parts:
        out[: len(p)] += p
    return unit(out)


def blade_glide(rng, length, centre):
    """Blades gliding over each other between the snaps: a soft, band-limited friction
    noise with an irregular grain, kept under 9 kHz."""
    noise = shaped_noise(rng, length, lambda t, f: bandpass_mag(f, centre, 1.4) ** 2 * lowpass_mag(f, 9000, 3))
    return unit(noise * bell(length, 0.3) * random_envelope(rng, length, 120, 0.4))


def leaf_shutter(rng, gap, blade_f0, body_f0, body_t60, spring_f0, spring, travel, travel_centre):
    first = blade_snap(rng, blade_f0, body_f0, body_t60, spring_f0, spring)
    second = blade_snap(rng, blade_f0 * 0.94, body_f0 * 0.97, body_t60, spring_f0, spring * 0.7)
    sweep = blade_glide(rng, gap * 0.8, travel_centre)
    return mix(0.2, (first, 0.0, 1.0), (sweep, gap * 0.1, travel), (second, gap, 0.85))


def screen_capture_b1(rng):
    return leaf_shutter(rng, 0.035, 4300, 1100, 0.06, 1500, 0.12, 0.08, 3600)


def screen_capture_b2(rng):
    return leaf_shutter(rng, 0.045, 3800, 700, 0.075, 1200, 0.15, 0.1, 3000)


def screen_capture_b3(rng):
    return leaf_shutter(rng, 0.03, 4800, 1600, 0.05, 2100, 0.1, 0.06, 4200)


def screen_capture_b4(rng):
    return leaf_shutter(rng, 0.055, 4100, 900, 0.07, 1350, 0.25, 0.16, 3300)


REFINEMENTS = {
    "trash-move": {
        "B1": (trash_move_b1, "B, lighter and shorter: a smooth bright whoosh, a brief crisp landing, the faintest steel ring."),
        "B2": (trash_move_b2, "B, richer and denser: the sheet crinkles in flight, a fuller crackling landing with a soft puff and a light steel ring."),
        "B3": (trash_move_b3, "B, drier and closer: almost no room, finer resonant paper ticks with more fibre sheen, no bin ring."),
        "B4": (trash_move_b4, "B with a more defined bin contact: the landing ends on a clear tap of a steel bin rim."),
    },
    "trash-drag": {
        "B1": (trash_drag_b1, "B, lighter and shorter: a brief fine rustle and a small steel tick."),
        "B2": (trash_drag_b2, "B, richer and denser: a fuller rustle with micro-crackles, a soft puff and a ringing steel tap."),
        "B3": (trash_drag_b3, "B, drier and closer: crisp resonant paper ticks, almost no room, a short steel tick."),
        "B4": (trash_drag_b4, "B with a more defined bin contact: a firmer tap with contact noise on the steel bin."),
    },
    "trash-empty": {
        "B1": (trash_empty_b1, "B, lighter and shorter (~0.4 s): a quick fine crumple and a soft drop."),
        "B2": (trash_empty_b2, "B, richer and denser (~0.6 s): clustered crackles darkening as the ball compacts, then the drop."),
        "B3": (trash_empty_b3, "B, drier and closer: crisp resonant crackles with fibre sheen, almost no room, a light puff at the end."),
        "B4": (trash_empty_b4, "B with a more defined bin contact: the ball lands with an audible steel bin tap."),
    },
    "file-transfer": {
        "B1": (file_transfer_b1, "B refined: a smoother, narrower swish into a glass-bowl tink on E7, tink a little more present (~0.36 s)."),
        "B2": (file_transfer_b2, "B refined with a tuned chime: the swish ends on two near-harmonic glass tinks, A6 then E7 (~0.42 s)."),
        "B3": (file_transfer_b3, "B refined, short: a quick soft swish and a clear glass-bowl tink on C7 in front (~0.3 s)."),
        "B4": (file_transfer_b4, "B refined, pitched: the swish narrows onto the chime's pitch and lands on a tuned glass tink on D7 (~0.42 s)."),
    },
    "screen-capture": {
        "B1": (screen_capture_b1, "B with real mechanics: two blade snaps 35 ms apart, a mid mechanism body, a subtle spring, faint blade travel."),
        "B2": (screen_capture_b2, "B, warmer: 45 ms apart, a lower body tone and a slightly longer spring."),
        "B3": (screen_capture_b3, "B, tight and small: 30 ms apart, a bright compact mechanism."),
        "B4": (screen_capture_b4, "B, slower and more mechanical: 55 ms apart, audible blade travel and a defined spring ring."),
    },
}

for _name, _refined in REFINEMENTS.items():
    SOUNDS[_name].update(_refined)

# Candidates finished with less room than ROOM_MIX (the "drier, closer" takes).
DRY = {("trash-move", "B3"), ("trash-drag", "B3"), ("trash-empty", "B3")}

# The refined shutters carry their energy in resonances, so their first milliseconds can
# lose more to the limiter (a gentle transient shaper) and sit nearer the others' loudness.
LIMIT_DB = {("screen-capture", v): 10.0 for v in ("B1", "B2", "B3", "B4")}


# --- The C candidates: one sound per role -----------------------------------------------
#
# The file manager has three file sounds: moving to the Trash (any way: keyboard, menu,
# script or a drop on the Dock's Trash, so trash-drag ships the same file as trash-move,
# see ALIASES), moving / copying / duplicating, and emptying the Trash. The C candidates
# design each role; they keep the refined paper of B1–B4.


def ticks(rng, times_at, centre, gain):
    """Tiny paper ticks after a gesture (the sheet or the ball settling)."""
    total = max(times_at) + 0.02
    out = np.zeros(n_of(total))
    for at in times_at:
        out += place(total, paper_grain(rng, centre * rng.uniform(0.9, 1.15), 0.0012, 0.3, 2.0), at, gain)
    return out


# Trash: a short, bright, dry rustle, ~100 ms.


def trash_c1(rng):
    rustle = paper_fine(rng, 0.09, swelling(0.09, 1.2), 3600, lambda t: 4600 + 15000 * t, size=0.0012, sheen=0.3, bed=0)
    return mix(0.25, (unit(rustle), 0.0, 1.0), (ticks(rng, [0.17], 5200, 1.0), 0.0, 0.25))


def trash_c2(rng):
    rustle = paper_fine(rng, 0.07, swelling(0.07, 1.0), 4200, lambda t: 5200, size=0.001, sigma=0.5, q=3.0, sheen=0.35, bed=0)
    return unit(rustle)


def trash_c3(rng):
    rustle = paper_fine(rng, 0.11, swelling(0.11, 1.4), 3800, lambda t: 4300 + 10000 * t, cluster=0.35, crackle=0.07, sheen=0.25, bed=0.03)
    return mix(0.2, (unit(rustle), 0.0, 1.0), (flop(rng, 0.025, 1800), 0.02, 0.12))


def trash_c4(rng):
    rustle = paper_fine(rng, 0.09, swelling(0.09, 1.2), 3600, lambda t: 4800 + 12000 * t, size=0.0012, sheen=0.3, bed=0)
    return mix(0.3, (unit(rustle), 0.0, 1.0), (steel_tap(820, 0.18, 0.0002), 0.16, 0.12))


# Move / copy / duplicate: a brief bright click, then a soft low tonal body (150–330 Hz)
# that holds for about 0.4 s and fades out, ~0.6 s in all.


def hold(length, sustain, release):
    """1 until `sustain`, then a raised-cosine fade to 0 at `sustain + release`."""
    t = times(length)
    return 0.5 + 0.5 * np.cos(np.pi * np.clip((t - sustain) / release, 0, 1))


# Circular membrane (a drum head), Bessel-zero ratios of the modes (0,1), (1,1), (2,1),
# (0,2), (3,1).
MEMBRANE = [1.0, 1.594, 2.136, 2.296, 2.653]


def low_body(excitation, modes, length=0.6, sustain=0.32, release=0.26):
    body = strike(excitation, modes, length)[: n_of(length)]
    return unit(body * hold(len(body) / RATE, sustain, release))


def file_click(rng, centre=4200.0):
    """The start of the gesture: a short cluster of bright paper ticks, ~25 ms."""
    return unit(ticks(rng, [0.0, 0.004, 0.011, 0.019], centre, 1.0))


def file_transfer_c1(rng):
    # A soft felt mallet on a hollow wooden box: its air cavity (175 Hz) and a few wall
    # modes, long enough to hold.
    walls = modes_from(240, [r for r, _, _ in plate_ratios(1.3, 4)], [0.5, 0.25, 0.12, 0.06], 3.0, 600)
    body = low_body(mallet(0.004), [(175, 1.0, 4.0)] + walls)
    return mix(0.7, (file_click(rng), 0.0, 0.5), (body, 0.008, 1.0))


def file_transfer_c2(rng):
    # A plastic latch clicks, then a felt-covered drum head (membrane modes on 165 Hz).
    latch = unit(strike(mallet(0.0002), plastic_bin(2800, 0.02, 1.2, (0.3, 0.4))))
    head = modes_from(165, MEMBRANE, [1.0, 0.45, 0.25, 0.18, 0.1], 3.5, 500)
    return mix(0.7, (latch, 0.0, 0.45), (low_body(mallet(0.005), head), 0.01, 1.0))


def file_transfer_c3(rng):
    # A low tuned bar (G3, tuned overtones 1 : 3.9 : 9.2) with a soft mallet: woody, tonal.
    bar = modes_from(196, TUNED_BAR, [1.0, 0.22, 0.04], 3.5, 900)
    return mix(0.7, (file_click(rng, 4600), 0.0, 0.45), (low_body(mallet(0.003), bar), 0.006, 1.0))


def file_transfer_c4(rng):
    # A cushioned thud: a slow, low noise push (~30 ms) blooms into three resonances.
    t = times(0.03)
    push = shape(rng.standard_normal(len(t)), lambda f: lowpass_mag(f, 300, 2)) * np.sin(np.pi * t / 0.03) ** 2
    modes = [(150, 1.0, 3.5), (228, 0.55, 3.0), (312, 0.3, 2.5)]
    return mix(0.7, (file_click(rng, 3800), 0.0, 0.4), (low_body(push, modes), 0.004, 1.0))


# Empty Trash: a papery crumple that swells to a peak around 110 ms and fades, ~320 ms.


def swell_fade(rng, length, peak):
    wobble = random_envelope(rng, length + 0.05, 30, 0.45)

    def density(t):
        shape_ = (t / peak) ** 1.5 if t < peak else np.exp(-(t - peak) / ((length - peak) / 2.5))
        return float(np.clip(0.8 * shape_ * wobble[min(len(wobble) - 1, n_of(t))], 0, 1))

    return density


def trash_empty_c1(rng):
    body = paper_fine(rng, 0.3, swell_fade(rng, 0.3, 0.11), 4200, lambda t: 4600 * (3200 / 4600) ** (t / 0.3), sheen=0.3)
    return mix(0.45, (unit(body), 0.0, 1.0), (ticks(rng, [0.33, 0.37], 4800, 1.0), 0.0, 0.2))


def trash_empty_c2(rng):
    body = paper_fine(rng, 0.27, swell_fade(rng, 0.27, 0.1), 4800, lambda t: 5000 * (3600 / 5000) ** (t / 0.27), size=0.0011, sigma=0.5, q=3.0, sheen=0.35, bed=0)
    return unit(body)


def trash_empty_c3(rng):
    body = paper_fine(rng, 0.32, swell_fade(rng, 0.32, 0.12), 3600, lambda t: 3800 * (2600 / 3800) ** (t / 0.32), size=0.002, cluster=0.2, crackle=0.03, sheen=0.2, bed=0.1)
    return mix(0.45, (unit(body), 0.0, 1.0), (ticks(rng, [0.35, 0.385], 4000, 1.0), 0.0, 0.15))


def trash_empty_c4(rng):
    body = paper_fine(rng, 0.3, swell_fade(rng, 0.3, 0.1), 4600, lambda t: 4800 * (3000 / 4800) ** (t / 0.3), cluster=0.4, crackle=0.08, sheen=0.3)
    return mix(0.45, (unit(body), 0.0, 1.0), (ticks(rng, [0.31, 0.335], 5200, 1.0), 0.0, 0.28))


# Screenshot: two soft clicks about 230 ms apart, ~370 ms in all.


def soft_click(rng, blade_f0, body_f0, body_t60, width, air):
    """A soft, fairly broadband shutter click: a blade snap with a wider (softer) pulse and
    a short puff of band-limited air from the moving blades."""
    snap = blade_snap(rng, blade_f0, body_f0, body_t60, 1500, 0.08, width)
    puff = shaped_noise(rng, 0.012, lambda t, f: bandpass_mag(f, 3500, 0.6) * lowpass_mag(f, 10000, 3))
    puff = unit(puff * np.exp(-times(0.012) / 0.003) * np.clip(times(0.012) / 0.0004, 0, 1))
    return mix(0.0, (snap, 0.0, 1.0), (puff, 0.0, air))


def two_clicks(first, second, gap, second_gain=0.85):
    out = mix(0.37, (first, 0.0, 1.0), (second, gap, second_gain))[: n_of(0.37)]
    return out * hold(0.37, 0.3, 0.07)


def screen_capture_c1(rng):
    return two_clicks(soft_click(rng, 4300, 1100, 0.06, 0.0004, 0.35), soft_click(rng, 4050, 1060, 0.06, 0.0004, 0.35), 0.23)


def screen_capture_c2(rng):
    return two_clicks(soft_click(rng, 3600, 800, 0.07, 0.0006, 0.45), soft_click(rng, 3400, 780, 0.07, 0.0006, 0.45), 0.225, 0.8)


def screen_capture_c3(rng):
    return two_clicks(soft_click(rng, 5000, 1500, 0.045, 0.0003, 0.3), soft_click(rng, 4700, 1450, 0.045, 0.0003, 0.3), 0.235, 0.9)


def screen_capture_c4(rng):
    # The first click is a latch and the blades a few milliseconds apart.
    latch = soft_click(rng, 4600, 1300, 0.04, 0.0003, 0.2)
    first = mix(0.0, (latch, 0.0, 0.5), (soft_click(rng, 4200, 1050, 0.06, 0.0004, 0.35), 0.006, 1.0))
    return two_clicks(first, soft_click(rng, 4000, 1000, 0.06, 0.0004, 0.35), 0.23)


ROLES = {
    "trash-move": {
        "C1": (trash_c1, "Trash (also trash-drag): a short bright dry rustle rising in brightness, ~100 ms, with a faint settling tick after."),
        "C2": (trash_c2, "Trash (also trash-drag): the shortest and crispest, ~70 ms of fine paper ticks, no tail."),
        "C3": (trash_c3, "Trash (also trash-drag): a slightly fuller rustle with micro-crackles over a soft puff, ~110 ms."),
        "C4": (trash_c4, "Trash (also trash-drag): the bright rustle, then a tiny steel tick of the bin."),
    },
    "file-transfer": {
        "C1": (file_transfer_c1, "Move/copy: a bright paper click, then a soft felt-mallet thud of a hollow wooden box (175 Hz), held ~0.35 s, ~0.6 s."),
        "C2": (file_transfer_c2, "Move/copy: a plastic latch click, then a felt-covered drum head on 165 Hz, held and faded, ~0.6 s."),
        "C3": (file_transfer_c3, "Move/copy: a bright click, then a low tuned wooden bar on G3, woody and tonal, ~0.6 s."),
        "C4": (file_transfer_c4, "Move/copy: a click, then a cushioned thud blooming into 150/228/312 Hz resonances, ~0.6 s."),
    },
    "trash-empty": {
        "C1": (trash_empty_c1, "Empty Trash: a crumple swelling to its peak at ~110 ms and fading by ~300 ms, then two tiny ticks."),
        "C2": (trash_empty_c2, "Empty Trash: a drier, crisper, slightly shorter crumple (~270 ms), no ticks."),
        "C3": (trash_empty_c3, "Empty Trash: a softer, darker crumple with longer grains (~320 ms), two faint ticks."),
        "C4": (trash_empty_c4, "Empty Trash: a crackly, denser crumple (~300 ms) with two clearer ticks."),
    },
    "screen-capture": {
        "C1": (screen_capture_c1, "Screenshot: two soft shutter clicks 230 ms apart (blade, body, a puff of air), ~0.37 s."),
        "C2": (screen_capture_c2, "Screenshot: two softer, warmer clicks 225 ms apart, the second a little quieter."),
        "C3": (screen_capture_c3, "Screenshot: two brighter, tighter clicks 235 ms apart."),
        "C4": (screen_capture_c4, "Screenshot: a latch and the blades a few ms apart, then the closing click 230 ms later."),
    },
}

for _name, _roles in ROLES.items():
    SOUNDS[_name].update(_roles)

LIMIT_DB.update({("screen-capture", v): 10.0 for v in ("C1", "C2", "C3", "C4")})
# The thud and the clicks have a fixed length; a quarter of the room keeps its tail short.
DRY.update({(n, v) for n in ("file-transfer", "screen-capture") for v in ("C1", "C2", "C3", "C4")})

# Sounds shipped as a copy of another sound's chosen file (same role, same sound).
ALIASES = {"trash-drag": "trash-move"}


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
        chosen = CHOSEN.get(ALIASES.get(name, name)) if name in ALIASES else CHOSEN[name]
        index[name] = {"chosen": chosen, "candidates": {}}
        if name in ALIASES:
            index[name]["same_as"] = ALIASES[name]
        for variant, (make, description) in variants.items():
            # One generator per candidate: changing one leaves the others identical.
            rng = np.random.default_rng([SEED, sum(name.encode()), *variant.encode()])
            room = ROOM_MIX / 4 if (name, variant) in DRY else ROOM_MIX
            signal = finish(make(rng), rng, room, LIMIT_DB.get((name, variant), MAX_LIMIT_DB))
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
        source = ALIASES.get(name, name)
        shutil.copyfile(candidates / f"{source}-{CHOSEN[source]}.wav", shipped / f"{name}.wav")
    (candidates / "index.json").write_text(json.dumps(index, indent=2) + "\n")


if __name__ == "__main__":
    main()
