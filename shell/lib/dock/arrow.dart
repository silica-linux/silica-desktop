// The outline of the Dock's label and menu (kit `Dock/Label`, `Dock/Menu`): a Glass/Thick body
// with smooth corners and, on its bottom edge, a small arrow towards the icon. The arrow is 6
// deep and 12 wide at the base (flanks at 45 degrees), its base corners rounded 3 and its tip 2;
// the body edge is 6 above the arrow's tip. It is the geometry of silica_ui's popover arrow
// (`AmalfiPopoverShape`) at the Dock's smaller size, which that shape cannot take (its arrow and
// radius are fixed constants).

import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

/// How far the arrow sticks out of the body.
const double dockArrowDepth = 6;

/// Half the width of the arrow's base.
const double dockArrowHalfBase = 6;

/// The radius of the corners where the arrow meets the body, and of its tip.
const double dockArrowBaseRadius = 3;
const double dockArrowTipRadius = 2;

/// The outline of a body of corner `radius` whose bottom edge carries the arrow at `tipX` (from
/// the left of the frame; null: in the middle). The frame includes the arrow: the body is the
/// frame minus its bottom [dockArrowDepth].
class DockArrowBorder extends ShapeBorder {
  const DockArrowBorder({required this.radius, this.tipX});

  final double radius;
  final double? tipX;

  /// The body's rectangle in `rect`.
  static Rect body(Rect rect) => Rect.fromLTRB(
    rect.left,
    rect.top,
    rect.right,
    rect.bottom - dockArrowDepth,
  );

  /// The x of the arrow's tip in `rect`: kept where the arrow fits between the corners.
  double tipIn(Rect rect) {
    final room = radius + dockArrowHalfBase;
    final x = tipX ?? rect.width / 2;
    if (rect.width < 2 * room) return rect.left + rect.width / 2;
    return rect.left + x.clamp(room, rect.width - room).toDouble();
  }

  @override
  EdgeInsetsGeometry get dimensions => EdgeInsets.zero;

  @override
  Path getOuterPath(Rect rect, {TextDirection? textDirection}) {
    final b = body(rect);
    final bodyPath = AmalfiSmoothBorder(radius: radius).getOuterPath(b);
    final m = Matrix4.identity()
      ..translateByDouble(tipIn(rect), b.bottom, 0, 1);
    return Path.combine(
      PathOperation.union,
      bodyPath,
      _arrow.transform(m.storage),
    );
  }

  @override
  Path getInnerPath(Rect rect, {TextDirection? textDirection}) =>
      getOuterPath(rect);

  @override
  void paint(Canvas canvas, Rect rect, {TextDirection? textDirection}) {}

  @override
  ShapeBorder scale(double t) => this;

  /// The arrow with its base on y = 0 and its tip on y = depth, centred on x = 0, overlapping
  /// the body by a point so that the union has no seam.
  static final Path _arrow = () {
    const h = dockArrowHalfBase;
    const d = dockArrowDepth;
    const rb = dockArrowBaseRadius;
    const rt = dockArrowTipRadius;
    final theta = math.atan2(d, h);
    final tb = rb * math.tan(theta / 2);
    final tt = rt * math.tan(theta);
    final c = math.cos(theta), s = math.sin(theta);
    return Path()
      ..moveTo(-h - tb - 2, -1)
      ..lineTo(-h - tb - 2, 0)
      ..lineTo(-h - tb, 0)
      ..arcToPoint(
        Offset(-h + tb * c, tb * s),
        radius: const Radius.circular(rb),
      )
      ..lineTo(-tt * c, d - tt * s)
      ..arcToPoint(
        Offset(tt * c, d - tt * s),
        radius: const Radius.circular(rt),
        clockwise: false,
      )
      ..lineTo(h - tb * c, tb * s)
      ..arcToPoint(Offset(h + tb, 0), radius: const Radius.circular(rb))
      ..lineTo(h + tb + 2, 0)
      ..lineTo(h + tb + 2, -1)
      ..close();
  }();

  @override
  bool operator ==(Object other) =>
      other is DockArrowBorder && other.radius == radius && other.tipX == tipX;

  @override
  int get hashCode => Object.hash(radius, tipX);
}
