// The Screenshot UI's menus (kit `Apps/Screenshot/Options Menu Panel`): an Amalfi menu (Glass/Thick,
// radius 12, 4 pt padding, rows of 24, headers, separators of 8) placed by the layer.
//
// This is the thumbnail's menu. It opens over the thumbnail, which is part of this layer:
// Ossido's glass would sit behind the whole layer, thumbnail included, so it keeps silica_ui's
// own Flutter-drawn menu without blur ([AmalfiMenu] with `opaque`), whatever backend the rest of
// the layer uses. (The Options menu is the pull-down's own, [AmalfiPullDownButton.entries], on
// Ossido's glass.)
//
// A menu appears with `fade.instant`; a click outside, Esc or a choice closes it (the choice
// blinks, then [onDismissed] is called at +114 ms, as in every Amalfi menu).

import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

/// The height of [entries] as [AmalfiMenu] lays them out: 24 pt rows, 8 pt separators, 4 pt
/// of padding above and below.
double shotMenuHeight(List<AmalfiMenuEntry> entries) =>
    2 * AmalfiMenuMetrics.padding +
    entries.fold<double>(
      0,
      (sum, e) =>
          sum +
          (e is AmalfiMenuDivider
              ? AmalfiMenuMetrics.separatorHeight
              : AmalfiMenuMetrics.itemHeight),
    );

/// A menu over the whole layer, placed at [anchor]: opening [upwards] its bottom is at the
/// anchor's y, otherwise its top; its left edge at the anchor's x, moved left when it would
/// leave the screen. With [alternative] (the anchor below the bar) it flips when there is no
/// room between [top] and the anchor.
class ShotMenu extends StatelessWidget {
  const ShotMenu({
    super.key,
    required this.entries,
    required this.anchor,
    required this.onDismissed,
    this.upwards = true,
    this.alternative,
    this.top = 0,
    this.width = optionsMenuWidthDefault,
  });

  static const double optionsMenuWidthDefault = 208;

  final List<AmalfiMenuEntry> entries;
  final Offset anchor;

  /// Where the menu goes when it does not fit on the [upwards] side.
  final Offset? alternative;
  final bool upwards;

  /// The least y a menu may reach (the menu bar).
  final double top;

  final double width;
  final VoidCallback onDismissed;

  @override
  Widget build(BuildContext context) {
    final height = shotMenuHeight(entries);
    return Stack(
      children: [
        // A click outside closes it (and does nothing else).
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapDown: (_) => onDismissed(),
            onSecondaryTapDown: (_) => onDismissed(),
          ),
        ),
        Positioned.fill(
          child: CustomSingleChildLayout(
            delegate: _Place(
              anchor: anchor,
              alternative: alternative,
              upwards: upwards,
              top: top,
              height: height,
            ),
            child: TweenAnimationBuilder<double>(
              tween: Tween(begin: 0, end: 1),
              duration: AmalfiFades.instant.duration,
              curve: AmalfiFades.instant.curve,
              builder: (context, opacity, child) =>
                  Opacity(opacity: opacity, child: child),
              child: SizedBox(width: width, child: _opaque()),
            ),
          ),
        ),
      ],
    );
  }

  // Flutter draws this one: the thumbnail is under it, in the same layer.
  Widget _opaque() => AmalfiGlassBackendScope(
    backend: AmalfiGlassBackend.flutter,
    child: AmalfiMenu(
      entries: entries,
      width: width,
      opaque: true,
      autofocus: true,
      onDismiss: onDismissed,
    ),
  );
}

class _Place extends SingleChildLayoutDelegate {
  const _Place({
    required this.anchor,
    required this.alternative,
    required this.upwards,
    required this.top,
    required this.height,
  });

  final Offset anchor;
  final Offset? alternative;
  final bool upwards;
  final double top;
  final double height;

  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      BoxConstraints(maxWidth: constraints.maxWidth, maxHeight: height);

  @override
  Offset getPositionForChild(Size size, Size childSize) {
    var at = anchor;
    var up = upwards;
    if (upwards && at.dy - childSize.height < top && alternative != null) {
      at = alternative!;
      up = false;
    }
    final y = up ? at.dy - childSize.height : at.dy;
    // Whole on the screen, 8 pt from its right edge.
    final x = at.dx
        .clamp(0.0, math.max(0.0, size.width - childSize.width - 8))
        .toDouble();
    return Offset(
      x,
      y.clamp(top, math.max(top, size.height - childSize.height)).toDouble(),
    );
  }

  @override
  bool shouldRelayout(_Place old) =>
      old.anchor != anchor ||
      old.alternative != alternative ||
      old.upwards != upwards ||
      old.top != top ||
      old.height != height;
}
