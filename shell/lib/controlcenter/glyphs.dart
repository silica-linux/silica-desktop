// The Control Center glyphs, drawn in code (no assets): Wi-Fi, Bluetooth, moon (Do Not
// Disturb), battery, sun and speaker at the ends of the sliders.

import 'dart:math' as math;

import 'package:flutter/widgets.dart';

enum GlyphKind {
  wifi,
  bluetooth,
  moon,
  battery,
  sunSmall,
  sunLarge,
  speakerSmall,
  speakerLarge,

  /// Dark Mode: a ring with its right half filled.
  contrast,

  /// Screen capture: a rounded frame with a band at the bottom.
  screenCapture,

  /// Bluetooth devices: a mouse, headphones, a keyboard.
  mouse,
  headphones,
  keyboard,

  /// Nearby sharing: a dot with two arcs above it, as waves going out.
  nearby,

  /// Window staging: a window with a strip of small windows on its left.
  staging,

  /// Screen mirroring: two overlapping screens.
  mirroring,

  /// Edit mode: the remove badge's minus.
  minus,

  /// The privacy pill: a microphone, a camera.
  microphone,
  camera,
}

class Glyph extends StatelessWidget {
  const Glyph(
    this.kind, {
    super.key,
    required this.color,
    this.size = 20,
    this.level = 1,
  });

  final GlyphKind kind;
  final Color color;
  final double size;

  /// Level for the battery (0–1).
  final double level;

  @override
  Widget build(BuildContext context) => CustomPaint(
    size: Size.square(size),
    painter: _GlyphPainter(kind, color, level),
  );
}

class _GlyphPainter extends CustomPainter {
  const _GlyphPainter(this.kind, this.color, this.level);

  final GlyphKind kind;
  final Color color;
  final double level;

  @override
  void paint(Canvas canvas, Size size) {
    final s = size.width / 20;
    final c = size.center(Offset.zero);
    final stroke = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2 * s
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    final fill = Paint()..color = color;
    switch (kind) {
      case GlyphKind.wifi:
        // Three arcs and a dot, ~20 × 15 pt on the 36 disc (controlcenter.md).
        final base = c + Offset(0, 6 * s);
        for (final r in [4.5, 8.5, 12.5]) {
          canvas.drawArc(
            Rect.fromCircle(center: base, radius: r * s),
            -math.pi * 0.75,
            math.pi * 0.5,
            false,
            stroke,
          );
        }
        canvas.drawCircle(base, 1.8 * s, fill);
      case GlyphKind.bluetooth:
        // The rune: 11 × 18 pt.
        final path = Path()
          ..moveTo(c.dx - 4.5 * s, c.dy - 4.5 * s)
          ..lineTo(c.dx + 4.5 * s, c.dy + 4.5 * s)
          ..lineTo(c.dx, c.dy + 9 * s)
          ..lineTo(c.dx, c.dy - 9 * s)
          ..lineTo(c.dx + 4.5 * s, c.dy - 4.5 * s)
          ..lineTo(c.dx - 4.5 * s, c.dy + 4.5 * s);
        canvas.drawPath(path, stroke..strokeWidth = 1.8 * s);
      case GlyphKind.mouse:
        final body = RRect.fromRectAndRadius(
          Rect.fromCenter(center: c, width: 10 * s, height: 16 * s),
          Radius.circular(5 * s),
        );
        canvas
          ..drawRRect(body, stroke..strokeWidth = 1.6 * s)
          ..drawLine(c + Offset(0, -6 * s), c + Offset(0, -2.5 * s), stroke);
      case GlyphKind.headphones:
        canvas
          ..drawArc(
            Rect.fromCircle(center: c + Offset(0, 1 * s), radius: 7 * s),
            math.pi,
            math.pi,
            false,
            stroke..strokeWidth = 1.6 * s,
          )
          ..drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromLTWH(c.dx - 8 * s, c.dy + 1 * s, 4 * s, 7 * s),
              Radius.circular(1.5 * s),
            ),
            fill,
          )
          ..drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromLTWH(c.dx + 4 * s, c.dy + 1 * s, 4 * s, 7 * s),
              Radius.circular(1.5 * s),
            ),
            fill,
          );
      case GlyphKind.keyboard:
        final body = RRect.fromRectAndRadius(
          Rect.fromCenter(center: c, width: 18 * s, height: 11 * s),
          Radius.circular(2 * s),
        );
        canvas.drawRRect(body, stroke..strokeWidth = 1.4 * s);
        for (var row = 0; row < 2; row++) {
          for (var col = 0; col < 5; col++) {
            canvas.drawRect(
              Rect.fromLTWH(
                body.left + (2.2 + col * 2.9) * s,
                body.top + (2.2 + row * 2.6) * s,
                1.6 * s,
                1.4 * s,
              ),
              fill,
            );
          }
        }
        canvas.drawLine(
          Offset(body.left + 5 * s, body.bottom - 2.6 * s),
          Offset(body.right - 5 * s, body.bottom - 2.6 * s),
          stroke..strokeWidth = 1.2 * s,
        );
      case GlyphKind.nearby:
        final base = c + Offset(0, 5 * s);
        canvas.drawCircle(base, 2.2 * s, fill);
        for (final r in [6.0, 10.5]) {
          canvas.drawArc(
            Rect.fromCircle(center: base, radius: r * s),
            -math.pi * 0.8,
            math.pi * 0.6,
            false,
            stroke..strokeWidth = 1.8 * s,
          );
        }
      case GlyphKind.staging:
        final main = RRect.fromRectAndRadius(
          Rect.fromLTWH(c.dx - 3 * s, c.dy - 6 * s, 12 * s, 12 * s),
          Radius.circular(2 * s),
        );
        canvas.drawRRect(main, stroke..strokeWidth = 1.6 * s);
        for (var i = 0; i < 3; i++) {
          canvas.drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromLTWH(
                c.dx - 9 * s,
                c.dy - 6 * s + i * 4.4 * s,
                4 * s,
                3.2 * s,
              ),
              Radius.circular(0.8 * s),
            ),
            fill,
          );
        }
      case GlyphKind.mirroring:
        final back = RRect.fromRectAndRadius(
          Rect.fromLTWH(c.dx - 8 * s, c.dy - 7 * s, 12 * s, 9 * s),
          Radius.circular(1.6 * s),
        );
        final front = RRect.fromRectAndRadius(
          Rect.fromLTWH(c.dx - 3 * s, c.dy - 2 * s, 12 * s, 9 * s),
          Radius.circular(1.6 * s),
        );
        canvas
          ..drawRRect(back, stroke..strokeWidth = 1.5 * s)
          ..drawRRect(front, fill);
      case GlyphKind.microphone:
        canvas
          ..drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromCenter(
                center: c - Offset(0, 3 * s),
                width: 6 * s,
                height: 10 * s,
              ),
              Radius.circular(3 * s),
            ),
            fill,
          )
          ..drawArc(
            Rect.fromCircle(center: c - Offset(0, 2 * s), radius: 5.5 * s),
            0,
            math.pi,
            false,
            stroke..strokeWidth = 1.6 * s,
          )
          ..drawLine(c + Offset(0, 3.5 * s), c + Offset(0, 7 * s), stroke);
      case GlyphKind.camera:
        canvas
          ..drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromLTWH(c.dx - 8 * s, c.dy - 5 * s, 11 * s, 10 * s),
              Radius.circular(2 * s),
            ),
            fill,
          )
          ..drawPath(
            Path()
              ..moveTo(c.dx + 4 * s, c.dy - 1.5 * s)
              ..lineTo(c.dx + 8 * s, c.dy - 4 * s)
              ..lineTo(c.dx + 8 * s, c.dy + 4 * s)
              ..lineTo(c.dx + 4 * s, c.dy + 1.5 * s)
              ..close(),
            fill,
          );
      case GlyphKind.minus:
        canvas.drawLine(
          c - Offset(5 * s, 0),
          c + Offset(5 * s, 0),
          stroke..strokeWidth = 2.2 * s,
        );
      case GlyphKind.contrast:
        final r = 8 * s;
        canvas
          ..drawCircle(c, r, stroke..strokeWidth = 2 * s)
          ..drawArc(
            Rect.fromCircle(center: c, radius: r - 2.5 * s),
            -math.pi / 2,
            math.pi,
            true,
            fill,
          );
      case GlyphKind.screenCapture:
        final frame = RRect.fromRectAndRadius(
          Rect.fromCenter(center: c, width: 18 * s, height: 14 * s),
          Radius.circular(3 * s),
        );
        canvas
          ..drawRRect(frame, stroke..strokeWidth = 1.8 * s)
          ..drawRect(
            Rect.fromLTRB(
              frame.left + 2 * s,
              c.dy + 1.5 * s,
              frame.right - 2 * s,
              c.dy + 4 * s,
            ),
            fill,
          );
      case GlyphKind.moon:
        final moon = Path.combine(
          PathOperation.difference,
          Path()..addOval(Rect.fromCircle(center: c, radius: 8 * s)),
          Path()..addOval(
            Rect.fromCircle(center: c + Offset(5 * s, -4 * s), radius: 7 * s),
          ),
        );
        canvas.drawPath(moon, fill);
      case GlyphKind.battery:
        final body = RRect.fromRectAndRadius(
          Rect.fromCenter(
            center: c - Offset(1 * s, 0),
            width: 16 * s,
            height: 9 * s,
          ),
          Radius.circular(2.5 * s),
        );
        canvas.drawRRect(body, stroke..strokeWidth = 1.4 * s);
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTWH(
              body.left + 2 * s,
              body.top + 2 * s,
              (body.width - 4 * s) * level.clamp(0, 1),
              body.height - 4 * s,
            ),
            Radius.circular(1 * s),
          ),
          fill,
        );
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTWH(body.right + 1 * s, c.dy - 2 * s, 1.6 * s, 4 * s),
            Radius.circular(0.8 * s),
          ),
          fill,
        );
      case GlyphKind.sunSmall || GlyphKind.sunLarge:
        final large = kind == GlyphKind.sunLarge;
        final core = (large ? 4.2 : 3.0) * s;
        canvas.drawCircle(c, core, fill);
        final rays = stroke..strokeWidth = (large ? 1.6 : 1.3) * s;
        for (var i = 0; i < 8; i++) {
          final a = i * math.pi / 4;
          final dir = Offset(math.cos(a), math.sin(a));
          canvas.drawLine(
            c + dir * (core + 2 * s),
            c + dir * (core + (large ? 4.5 : 3.5) * s),
            rays,
          );
        }
      case GlyphKind.speakerSmall || GlyphKind.speakerLarge:
        final large = kind == GlyphKind.speakerLarge;
        final left = c.dx - (large ? 8 : 5) * s;
        final cone = Path()
          ..moveTo(left, c.dy - 2.5 * s)
          ..lineTo(left + 3 * s, c.dy - 2.5 * s)
          ..lineTo(left + 7 * s, c.dy - 6 * s)
          ..lineTo(left + 7 * s, c.dy + 6 * s)
          ..lineTo(left + 3 * s, c.dy + 2.5 * s)
          ..lineTo(left, c.dy + 2.5 * s)
          ..close();
        canvas.drawPath(cone, fill);
        if (large) {
          final waves = stroke..strokeWidth = 1.5 * s;
          for (final r in [4.0, 7.5]) {
            canvas.drawArc(
              Rect.fromCircle(
                center: Offset(left + 7 * s, c.dy),
                radius: r * s,
              ),
              -math.pi / 4,
              math.pi / 2,
              false,
              waves,
            );
          }
        }
    }
  }

  @override
  bool shouldRepaint(_GlyphPainter old) =>
      old.kind != kind || old.color != color || old.level != level;
}
