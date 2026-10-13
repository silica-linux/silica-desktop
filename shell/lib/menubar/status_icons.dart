// Icone della barra disegnate in codice (forme generiche, non asset): il marchio di
// Silica, la rete, la ricerca (Faro), il Centro di controllo. Bianche al 90 % come le
// icone di stato misurate; dimensioni dell'inchiostro 12–18 pt nelle caselle.

import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import 'menubar.dart';

/// Il marchio di Silica: un cristallo esagonale con una faccetta (14 × 16 pt, come
/// l'inchiostro del logo nella casella misurata).
class SilicaMark extends StatelessWidget {
  const SilicaMark({super.key, required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) =>
      CustomPaint(size: const Size(14, 16), painter: _MarkPainter(color));
}

class _MarkPainter extends CustomPainter {
  _MarkPainter(this.color);

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final w = size.width;
    final h = size.height;
    final outline = Path()
      ..moveTo(w / 2, 0)
      ..lineTo(w, h * 0.25)
      ..lineTo(w, h * 0.75)
      ..lineTo(w / 2, h)
      ..lineTo(0, h * 0.75)
      ..lineTo(0, h * 0.25)
      ..close();
    canvas.drawPath(outline, Paint()..color = color);
    // Faccetta: un taglio più scuro che dà il volume.
    final facet = Path()
      ..moveTo(w / 2, h * 0.5)
      ..lineTo(w, h * 0.25)
      ..lineTo(w, h * 0.75)
      ..lineTo(w / 2, h)
      ..close();
    canvas.drawPath(
      facet,
      Paint()..color = color.withValues(alpha: 0.55 * color.a),
    );
  }

  @override
  bool shouldRepaint(_MarkPainter old) => old.color != color;
}

enum StatusKind { network, search, controls }

class StatusIcon extends StatelessWidget {
  const StatusIcon({super.key, required this.kind, required this.cell});

  final StatusKind kind;

  /// Larghezza della casella (le misure: 22–33 pt).
  final double cell;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: cell,
    height: barHeight,
    child: CustomPaint(painter: _StatusPainter(kind)),
  );
}

class _StatusPainter extends CustomPainter {
  _StatusPainter(this.kind);

  final StatusKind kind;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final stroke = Paint()
      ..color = statusColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.6
      ..strokeCap = StrokeCap.round;
    final fill = Paint()..color = statusColor;
    switch (kind) {
      case StatusKind.network:
        // Tre archi e un punto, ~17 × 12 pt.
        final base = c + const Offset(0, 5);
        for (final r in [4.5, 8.0, 11.5]) {
          canvas.drawArc(
            Rect.fromCircle(center: base, radius: r),
            -math.pi * 0.75,
            math.pi * 0.5,
            false,
            stroke,
          );
        }
        canvas.drawCircle(base, 1.4, fill);
      case StatusKind.search:
        // Lente: 13 × 13 pt.
        canvas.drawCircle(c + const Offset(-1.5, -1.5), 4.6, stroke);
        canvas.drawLine(
          c + const Offset(1.8, 1.8),
          c + const Offset(5.5, 5.5),
          stroke,
        );
      case StatusKind.controls:
        // Due interruttori sovrapposti: 14 × 14 pt.
        for (final (dy, knobLeft) in [(-3.5, true), (3.5, false)]) {
          final pill = RRect.fromRectAndRadius(
            Rect.fromCenter(center: c + Offset(0, dy), width: 14, height: 6),
            const Radius.circular(3),
          );
          canvas.drawRRect(pill, stroke..strokeWidth = 1.3);
          canvas.drawCircle(c + Offset(knobLeft ? -4 : 4, dy), 1.8, fill);
        }
    }
  }

  @override
  bool shouldRepaint(_StatusPainter old) => old.kind != kind;
}
