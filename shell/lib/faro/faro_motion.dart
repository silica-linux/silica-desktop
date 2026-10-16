// Faro's motion vocabulary. Every transition of Faro and the Launcher takes its timing from
// here, so they all move alike (docs/spec/faro.md › Opening animation, docs/spec/launcher.md
// › Animations).
//
// There are two families:
// - Measured tables, replayed by time: the fade of every opening, the Launcher's opening
//   scale, the return from the Launcher.
// - Damped springs fitted to the measured overshoot and settling time, for the shapes that
//   change while the user acts: width, height, the round buttons. A spring starts from the
//   current value and velocity, so a change of mind halfway reverses without a jump.

import 'dart:math' as math;

import 'package:flutter/physics.dart';
import 'package:silica_ui/silica_ui.dart';

/// A spring of mass 1 from its damping ratio `zeta` and natural frequency `omega` (rad/s).
SpringDescription dampedSpring({required double zeta, required double omega}) =>
    SpringDescription(
      mass: 1,
      stiffness: omega * omega,
      damping: 2 * zeta * omega,
    );

/// The damping ratio that gives `overshoot` (a fraction of the travel).
double zetaForOvershoot(double overshoot) {
  final l = math.log(overshoot);
  return -l / math.sqrt(math.pi * math.pi + l * l);
}

// Amalfi motion tokens (docs/design/amalfi/motion.md, "Surfaces"). The measured Faro
// overshoots are the tokens' own: spring.panel for heights, spring.morph for shapes.

/// Vertical spring: the opening height, the panel growing with results and the scopes
/// (`spring.panel`).
const SpringDescription verticalSpring = AmalfiSprings.panel;

/// Width spring of the bar when the buttons come out or go away (`spring.morph`).
const SpringDescription widthSpring = AmalfiSprings.morph;
const SpringDescription engageWidthSpring = AmalfiSprings.morph;

/// The round buttons sliding out from behind the bar and back (`spring.morph`).
const SpringDescription engageButtonsSpring = AmalfiSprings.morph;
const SpringDescription buttonsSpring = AmalfiSprings.morph;

/// How long the pointer on the empty capsule waits before it starts to change
/// (`delay.hoverIntent`).
const Duration engageDelay = AmalfiDelays.hoverIntent;

/// A change of scope grows or shrinks the panel (`spring.panel`).
const SpringDescription scopeSpring = AmalfiSprings.panel;

/// Closing takes `fade.instant` (the fade played backwards, the height back to 0.97).
final Duration closeDuration = AmalfiFades.instant.duration;

/// The buttons start with the bar's narrowing but are still merged into it (the split is
/// ~175 ms in: 60 ms hover intent plus this).
const Duration buttonsDelay = Duration(milliseconds: 80);

/// First frame of an opening: the panel is 0.97 of its height, centred, then it settles with the springs above.
const double openScaleY = 0.97;

/// Faro never exceeds 640 pt: the opening only scales vertically.
const double openScaleX = 1;

/// Length of the opening fade (the table below), in ms.
const double fadeMs = 66.7;

/// Value of a piecewise-linear table of (ms, value) at `ms`, clamped at both ends.
double tableAt(List<(double, double)> table, double ms) {
  if (ms <= table.first.$1) return table.first.$2;
  for (var i = 1; i < table.length; i++) {
    final (t1, v1) = table[i];
    if (ms <= t1) {
      final (t0, v0) = table[i - 1];
      return v0 + (v1 - v0) * (ms - t0) / (t1 - t0);
    }
  }
  return table.last.$2;
}

/// Opacity of every opening (Faro and the Launcher), at 0, 13, 27, 53 and 67 ms from the
/// first frame: steep, already 0.22 on the first frame. Closing plays the same fade backwards
/// in [closeDuration], then the panel is gone.
const List<(double, double)> openOpacity = [
  (0, 0.22),
  (13.3, 0.66),
  (26.7, 0.79),
  (53.3, 0.98),
  (66.7, 1),
];

/// Opacity at `ms` from the first frame of an opening.
double opacityAt(double ms) => tableAt(openOpacity, ms);

/// Launcher opening: it starts ~6 % larger and shrinks (residual overshoot measured at 0,
/// 13, 40, 53, 67, 93, 107, 120 and 147 ms), with the same fade as Faro.
const double launcherOpenScale = 0.06;
const List<(double, double)> launcherExcess = [
  (0, 1),
  (13.3, 0.79),
  (40, 0.43),
  (53.3, 0.34),
  (66.7, 0.24),
  (93.3, 0.09),
  (106.7, 0.03),
  (120, 0.01),
  (146.7, 0),
];
const double launcherOpenMs = 146.7;

/// The Launcher's scale at `ms` from the first frame of its opening.
double launcherScaleAt(double ms) =>
    1 + launcherOpenScale * tableAt(launcherExcess, ms);

/// Esc in the Launcher: the panel shrinks towards Faro (measured height, S curve, 215 ms).
/// Width, position and radius follow the same progress.
const List<(double, double)> backHeights = [
  (0, 577),
  (26.7, 556),
  (40, 509),
  (53.3, 468),
  (80, 349),
  (93.3, 294),
  (106.7, 234),
  (133.3, 170),
  (146.7, 143),
  (160, 122),
  (186.7, 90),
  (200, 59),
  (213.3, 57),
];
const double backDuration = 213.3;

/// Progress 0–1 of the return to Faro after `ms`.
double backProgress(double ms) {
  final first = backHeights.first.$2;
  final last = backHeights.last.$2;
  return (first - tableAt(backHeights, ms)) / (first - last);
}
