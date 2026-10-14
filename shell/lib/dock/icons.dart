// Placeholder icons, generated in code until the Dock reads the app icons from the system
// themes (M3.2): a rounded square with a gradient and the initial.

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

/// Icon radius relative to the side (tile shape, provisional).
const double iconCornerRatio = 0.225;

void paintIcon(Canvas canvas, Rect rect, String label, int index) {
  final radius = Radius.circular(rect.width * iconCornerRatio);
  final shape = RRect.fromRectAndRadius(rect, radius);
  // Short shadow under the icon: darkens ~3 pt below and 1–2 at the sides (dock.md).
  canvas.drawRRect(
    shape.shift(Offset(0, rect.width * 0.03)),
    Paint()
      ..color = const Color(0x55000000)
      ..maskFilter = MaskFilter.blur(BlurStyle.normal, rect.width * 0.03),
  );
  final hue = (index * 29.0) % 360;
  canvas.drawRRect(
    shape,
    Paint()
      ..shader = LinearGradient(
        begin: Alignment.topCenter,
        end: Alignment.bottomCenter,
        colors: [
          HSLColor.fromAHSL(1, hue, 0.65, 0.62).toColor(),
          HSLColor.fromAHSL(1, hue, 0.7, 0.44).toColor(),
        ],
      ).createShader(rect),
  );
  final initial = label.isEmpty ? '' : label.characters.first.toUpperCase();
  final text = TextPainter(
    text: TextSpan(
      text: initial,
      style: uiText(size: rect.width * 0.5, color: const Color(0xF2FFFFFF)),
    ),
    textDirection: TextDirection.ltr,
  )..layout();
  text.paint(canvas, rect.center - Offset(text.width / 2, text.height / 2));
}
