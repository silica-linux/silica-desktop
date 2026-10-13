// I glifi del Centro di controllo, disegnati in codice (nessun asset): Wi-Fi, Bluetooth,
// luna (Non disturbare), batteria, sole e altoparlante agli estremi dei cursori.

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

  /// Livello per la batteria (0–1).
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
        // Tre archi e un punto, ~20 × 15 pt sul disco da 36 (controlcenter.md).
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
        // La runa: 11 × 18 pt.
        final path = Path()
          ..moveTo(c.dx - 4.5 * s, c.dy - 4.5 * s)
          ..lineTo(c.dx + 4.5 * s, c.dy + 4.5 * s)
          ..lineTo(c.dx, c.dy + 9 * s)
          ..lineTo(c.dx, c.dy - 9 * s)
          ..lineTo(c.dx + 4.5 * s, c.dy - 4.5 * s)
          ..lineTo(c.dx - 4.5 * s, c.dy + 4.5 * s);
        canvas.drawPath(path, stroke..strokeWidth = 1.8 * s);
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
