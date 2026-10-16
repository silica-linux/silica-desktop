// The Screenshot UI's menus (kit `Apps/Screenshot/Options Menu Panel`): an Amalfi menu (Glass/Thick,
// radius 12, 4 pt padding, rows of 24, headers, separators of 8) placed by the layer.
//
// Two ways to draw it. The Options menu sits over the wallpaper and the windows, so its glass is the
// compositor's (`menu` kind: [ShadowedGlass]) with silica_ui's rows on it. The thumbnail's
// menu opens over the thumbnail, which is part of this layer: Ossido's glass would sit behind the
// whole layer, thumbnail included, so it is silica_ui's own menu without blur ([AmalfiMenu] with
// `opaque`).
//
// A menu appears with `fade.instant`; a click outside, Esc or a choice closes it (the choice
// blinks, then [onDismissed] is called at +114 ms, as in every Amalfi menu).

import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/glass_profile.dart';
import '../system/shadowed_glass.dart';

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
    this.glass = false,
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

  /// The compositor's glass instead of silica_ui's.
  final bool glass;
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
              child: SizedBox(
                width: width,
                child: glass ? _glass() : _opaque(),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _opaque() => AmalfiMenu(
    entries: entries,
    width: width,
    opaque: true,
    autofocus: true,
    onDismiss: onDismissed,
  );

  Widget _glass() => ShadowedGlass(
    profile: ShellGlassProfile.menu,
    radius: AmalfiMenuMetrics.radius,
    thick: true,
    child: Padding(
      padding: const EdgeInsets.all(AmalfiMenuMetrics.padding),
      child: AmalfiMenuList(
        entries: entries,
        autofocus: true,
        opaque: true,
        onDismiss: onDismissed,
      ),
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
    final x = at.dx
        .clamp(0.0, math.max(0.0, size.width - childSize.width))
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
