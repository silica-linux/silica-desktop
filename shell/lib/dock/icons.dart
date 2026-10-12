// Icone segnaposto, generate in codice finché il Dock non legge le icone delle app dai
// temi del sistema (M3.2): un quadrato arrotondato con un gradiente e l'iniziale.

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

/// Raggio dell'icona rispetto al lato (forma da tessera, provvisoria).
const double iconCornerRatio = 0.225;

void paintIcon(Canvas canvas, Rect rect, String label, int index) {
  final radius = Radius.circular(rect.width * iconCornerRatio);
  final shape = RRect.fromRectAndRadius(rect, radius);
  // Ombra corta sotto l'icona: scurisce ~3 pt sotto e 1–2 ai lati (dock.md).
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
  text.paint(
    canvas,
    rect.center - Offset(text.width / 2, text.height / 2),
  );
}
