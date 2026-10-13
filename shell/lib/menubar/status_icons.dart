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

enum StatusKind { network, wired, battery, search, controls }

class StatusIcon extends StatelessWidget {
  const StatusIcon({
    super.key,
    required this.kind,
    required this.cell,
    this.level = 1,
    this.dim = false,
    this.charging = false,
  });

  final StatusKind kind;

  /// Larghezza della casella (le misure: 22–33 pt).
  final double cell;

  /// Intensità del Wi-Fi o carica della batteria, 0–1.
  final double level;

  /// Spento (Wi-Fi disattivato): tutto al 35 %.
  final bool dim;

  /// Batteria in carica: un fulmine ritagliato nel riempimento.
  final bool charging;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: cell,
    height: barHeight,
    child: CustomPaint(
      painter: _StatusPainter(kind, level: level, dim: dim, charging: charging),
    ),
  );
}

/// Quanti archi del Wi-Fi sono accesi per un'intensità 0–1 (il punto lo è sempre).
int wifiArcs(double level) => level >= 0.67
    ? 3
    : level >= 0.34
    ? 2
    : 1;

class _StatusPainter extends CustomPainter {
  _StatusPainter(
    this.kind, {
    this.level = 1,
    this.dim = false,
    this.charging = false,
  });

  final StatusKind kind;
  final double level;
  final bool dim;
  final bool charging;

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
        // Tre archi e un punto, ~17 × 12 pt; gli archi oltre l'intensità al 35 %.
        final base = c + const Offset(0, 5);
        final faint = statusColor.withValues(alpha: statusColor.a * 0.35);
        final lit = dim ? 0 : wifiArcs(level);
        final radii = [4.5, 8.0, 11.5];
        for (var i = 0; i < radii.length; i++) {
          canvas.drawArc(
            Rect.fromCircle(center: base, radius: radii[i]),
            -math.pi * 0.75,
            math.pi * 0.5,
            false,
            Paint.from(stroke)..color = i < lit ? statusColor : faint,
          );
        }
        canvas.drawCircle(base, 1.4, dim ? (Paint()..color = faint) : fill);
      case StatusKind.wired:
        // Rete via cavo: tre caselle collegate, ~16 × 12 pt.
        final link = stroke..strokeWidth = 1.4;
        final top = Rect.fromCenter(
          center: c - const Offset(0, 3.5),
          width: 6,
          height: 5,
        );
        canvas.drawRect(top, link);
        for (final dx in [-5.5, 5.5]) {
          canvas.drawRect(
            Rect.fromCenter(center: c + Offset(dx, 4), width: 5, height: 4),
            link,
          );
          canvas.drawLine(c + Offset(dx, 2), c + Offset(dx, 0.5), link);
        }
        canvas.drawLine(
          c + const Offset(-5.5, 0.5),
          c + const Offset(5.5, 0.5),
          link,
        );
        canvas.drawLine(
          c + const Offset(0, -1),
          c + const Offset(0, 0.5),
          link,
        );
      case StatusKind.battery:
        // Batteria ~25 × 12 pt con il riempimento della carica (dimensioni provvisorie:
        // il riferimento è un fisso, senza batteria).
        final body = RRect.fromRectAndRadius(
          Rect.fromCenter(
            center: c - const Offset(1.5, 0),
            width: 23,
            height: 11.5,
          ),
          const Radius.circular(3.5),
        );
        canvas.drawRRect(body, Paint.from(stroke)..strokeWidth = 1.1);
        final inner = body.outerRect.deflate(2);
        canvas.saveLayer(body.outerRect.inflate(2), Paint());
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTWH(
              inner.left,
              inner.top,
              inner.width * level.clamp(0, 1),
              inner.height,
            ),
            const Radius.circular(1.5),
          ),
          fill,
        );
        if (charging) {
          final bolt = Path()
            ..moveTo(c.dx + 0.5, c.dy - 6)
            ..lineTo(c.dx - 4, c.dy + 0.8)
            ..lineTo(c.dx - 0.8, c.dy + 0.8)
            ..lineTo(c.dx - 2, c.dy + 6)
            ..lineTo(c.dx + 2.5, c.dy - 0.8)
            ..lineTo(c.dx - 0.6, c.dy - 0.8)
            ..close();
          canvas.drawPath(bolt, Paint()..blendMode = BlendMode.clear);
          canvas.drawPath(bolt, Paint.from(stroke)..strokeWidth = 0.9);
        }
        canvas.restore();
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTWH(body.right + 1, c.dy - 2, 1.6, 4),
            const Radius.circular(0.8),
          ),
          fill,
        );
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
  bool shouldRepaint(_StatusPainter old) =>
      old.kind != kind ||
      old.level != level ||
      old.dim != dim ||
      old.charging != charging;
}
