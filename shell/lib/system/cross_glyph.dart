// The × of the small round buttons, drawn instead of typed: a text glyph sits on the
// font's baseline and is off-centre in its circle (#166). The two strokes are centred on
// the circle and snapped to the half pixel their width needs, so the cross is optically
// centred at any scale.

import 'package:flutter/widgets.dart';

/// Paints a × centred in its box. `arm` is the half-length of each diagonal (default 0.32
/// of the box, the notification close button's measured proportion), `stroke` its width.
class CrossGlyph extends CustomPainter {
  const CrossGlyph(this.color, {this.arm, this.stroke = 2});

  final Color color;
  final double? arm;
  final double stroke;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final a = arm ?? size.shortestSide * 0.32;
    final paint = Paint()
      ..color = color
      ..strokeWidth = stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(c + Offset(-a, -a), c + Offset(a, a), paint);
    canvas.drawLine(c + Offset(-a, a), c + Offset(a, -a), paint);
  }

  @override
  bool shouldRepaint(CrossGlyph old) =>
      old.color != color || old.arm != arm || old.stroke != stroke;
}
