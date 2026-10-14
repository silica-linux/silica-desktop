# Interface sounds

Silica's interface sounds (`data/sounds/*.wav`, played by silica-daemon's `Sound1`) are
original work, synthesized by `synth.py` from physical and granular models. Nothing is
sampled, recorded or modelled on another system's sounds; the seeds are fixed, so the files
rebuild bit for bit.

```
python3 tools/sounds/synth.py                 # needs numpy
python3 tools/sounds/synth.py --spectrograms  # also a PNG per candidate (needs matplotlib)
```

The script writes the candidates of each sound to `candidates/<name>-<variant>.wav`: three
first designs (A, B, C) and four refinements of B (B1–B4, see below). It lists them with a
one-line description in `candidates/index.json` and copies the chosen one (`CHOSEN` in
`synth.py`, B after the first listening) to `data/sounds/<name>.wav`. To ship another
candidate, change its name in `CHOSEN` and run the script again. Each candidate has its own
random generator, so changing one leaves the others identical. The spectrogram PNGs are for
inspection only and are not committed.

## Building blocks

- **Modal synthesis.** A struck object is a bank of damped resonators,
  `Σ aₖ·e^(−6.9t/T₆₀ₖ)·sin(2πfₖt)`, convolved with an excitation. The frequency ratios come
  from the physics of the shape:
  - simply supported rectangular plate, `f_mn ∝ (m/a)² + (n/b)²`, with each mode weighted
    by its shape at the strike point, `|sin(mπx)·sin(nπy)|` (plastic bin walls, the camera
    housing, shutter blades);
  - thin ring or short cylindrical shell, `f_n ∝ n(n² − 1)/√(n² + 1)`, n = 2, 3, … (steel
    bin, glass bowl), each mode split into a slightly detuned pair like the degenerate
    modes of a round object, which gives a slow shimmer;
  - free–free Euler–Bernoulli bar, 1 : 2.756 : 5.404 : 8.933 (wood), and a bar tuned by
    thinning, 1 : 3.9 : 9.2 (the soft woody pop);
  - stiff wire, `f_n = n·√(1 + B·n²)` (the spring of a reflex mirror).

  Damping depends on frequency, `T₆₀(f) = T₆₀ / (1 + (f/f_d)²)`: the high modes die first,
  as in real materials (plastic has a low `f_d` and short `T₆₀`, glass and steel long ones).
  Excitations are raised-cosine force pulses (a wider pulse is a softer mallet, with less
  treble) or short band-limited noise bursts (contacts, latches, paper landing).
- **Granular paper.** Paper is many 2–14 ms grains of band-passed noise (fourth-order band,
  gently low-passed). The grain rate follows a density envelope times a smooth random
  envelope, so the grains come in bursts. Each grain's energy follows a power law, as
  crackling noise does, and a small fraction are short, wide-band, louder crackles. The
  band centre follows a curve over time: a falling centre is the spectral tilt darkening as
  a sheet is folded into a ball. A quiet bed of continuous friction noise follows the same
  density.
- **Air.** Whooshes and swishes are noise through a band-pass that moves along a path of
  centre frequencies (short-time Fourier overlap-add), under a bell envelope; a moving
  sheet can add an irregular flutter.
- **Room.** A generated small-room impulse response (eight early reflections 2–14 ms after
  the direct sound, then a darkening diffuse tail with T₆₀ ≈ 0.2 s) is mixed about 22 dB
  under the dry sound: the sounds sit in a space without sounding reverberant.
- **Finishing.** DC and rumble removed (30 Hz high-pass), a 0.5 ms fade-in from an exact
  zero, a cut where the sound has fallen 60 dB and a fade to an exact zero. Loudness is
  measured with the ITU-R BS.1770 K-weighting as the maximum over 400 ms windows (short
  sounds are measured in one window padded with silence) and normalised to −20 LUFS; a
  look-ahead limiter (1.5 ms look-ahead, 40 ms release) keeps the peaks at −6 dBFS, taking
  at most 6 dB off them. The shutter clicks are almost all transient, so with the peak
  ceiling they measure lower (about −28 LUFS) even though they sound as present as the
  others.

## The sounds

| Sound | When | Candidates |
| --- | --- | --- |
| `trash-move` | items moved to the Trash (menu, keyboard) | **A** airy sheet: a passing whoosh (band 0.9 → 2.6 → 1.5 kHz, light flutter), a soft granular rustle landing, and a faint damped plastic bin bottom, ~0.5 s. **B** crisp and quick: a brighter whoosh, a crackly rustle, a faint ring of a steel bin, ~0.4 s. **C** soft flutter: a slower, darker, flapping whoosh, a muffled rustle and a deeper thump, ~0.5 s. |
| `trash-drag` | items dropped on the Dock's Trash | **A** a short rustle, then a light tap on a moulded plastic bin (plate modes, soft mallet), ~0.2 s. **B** the same rustle with a tiny tap on a thin steel bin that rings briefly (shell modes), ~0.4 s. **C** a lighter rustle and a hollow woody tock (bar modes plus the bin's air cavity), ~0.2 s. |
| `trash-empty` | the Trash emptied | **A** crisp crumple: crackling bursts with a band falling 4.3 → 2.6 kHz as the ball compacts, ~0.65 s. **B** tight and quick, ending with the ball dropping softly into the bin, ~0.6 s. **C** soft, tissue-like: longer grains, darker band, few crackles, ~0.75 s. |
| `file-transfer` | a copy or move finished | **A** a two-note glass chime, D6 then G6 (a fourth up), from glass bowl modes with a soft mallet, ~0.7 s. **B** an airy rising swish ending on a faint glass tink, ~0.4 s. **C** a two-note soft woody pop from tuned-bar modes, G5 then D6, ~0.3 s. |
| `screen-capture` | a screenshot taken | **A** a shutter double click 60 ms apart: band-passed noise transients through a tiny steel blade (plate modes) and a damped housing, with the blades' friction between them. **B** a leaf shutter: tighter (35 ms) and brighter. **C** a reflex-camera style: a heavier first click with a bounce and a spring ring, the return 90 ms later. |

## Refinements of B (B1–B4)

B was preferred for every sound. B1–B4 stay close to its character and try improvements:

- **Paper** (`paper_fine`): each grain is an impulsive resonant tick (0.1 ms rise,
  exponential fall, filtered after the envelope so the snap stays in band) instead of a
  smooth noise window; grain lengths are log-normal around 1–1.6 ms; a quarter of the
  events are clusters of 2–5 micro-crackles 0.3–2 ms apart (fibres giving way in a row);
  a separate 4.5–11 kHz fibre sheen glitters under a spiky random envelope; the continuous
  friction bed is reduced to a trace or removed (less hiss). Landings add a soft low-mid
  puff (`flop`) of the sheet or ball meeting the bin. Per sound: **B1** lighter and
  shorter, **B2** richer and denser (the sheet also crinkles in flight), **B3** drier and
  closer (a quarter of the room, finer and crisper ticks), **B4** a more defined steel bin
  contact (a tap with contact noise).
- **file-transfer**: a smoother swish (fourth-order moving band, steeper top cut, about
  15 dB less energy above 12 kHz than B) and two tink partial sets: glass bowl modes (B1
  on E7, B3 on C7) and a tuned chime with near-harmonic modes 1 : 2.005 : 3.012 (B2, A6
  then E7; B4, D7, where the swish narrows onto the chime's pitch first). Lengths
  0.35–0.42 s.
- **screen-capture**: each blade transient is a narrow force pulse (with a trace of contact
  noise) driving a thin steel blade (long-plate modes), the mechanism body and a small
  spring (stiff-wire modes), so the energy sits in resonances rather than in a click; the
  blades glide between the snaps (band-limited friction noise). The limiter may take up
  to 10 dB off their first milliseconds, which acts as a gentle transient shaper and brings
  them about 3 dB nearer the others (about −26.5 LUFS against −29.4 for B). Gaps and
  bodies: **B1** 35 ms, mid body; **B2** 45 ms, warmer; **B3** 30 ms, small and bright;
  **B4** 55 ms, audible blade travel and spring.

## Alerts

The system has no alert sounds yet (silica-daemon plays only the names above); alert tones
will be designed the same way when an alert list exists.

The timing and level of each sound are set by ear, not measured from a reference.
