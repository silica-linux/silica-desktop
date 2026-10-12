// Il Dock della shell: disposizione e ingrandimento da model.dart (misure in
// plan/reference/dock.md), fondo di Liquid Glass calcolato da Ossido, icone, separatore,
// indicatori ed etichetta disegnati qui.

import 'package:flutter/scheduler.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import 'icons.dart';
import 'model.dart';

/// Colori dell'aspetto chiaro (dock.md).
const Color separatorColor = Color(0xFF484950);
const Color indicatorColor = Color(0xFF03040B);
const Color labelBody = Color(0xFFBDBEC3);
const Color labelBodyTop = Color(0xFFD7D8DE);
const Color labelBodyBottom = Color(0xFFC1C2C8);
const Color labelBorder = Color(0xFF3C3D41);
const Color labelText = Color(0xFF1C1C1D);

/// Etichetta: corpo alto 27 pt (y 1325–1351), punta larga ~7 pt e alta ~3 pt, con la cima
/// ~13 pt sopra l'icona (icona da 1367, punta a 1354), margine orizzontale ~15,5 pt, testo
/// di ~14 pt (dalle maiuscole di 10 pt in Inter).
const double labelHeight = 27;
const double labelTip = 3;
const double labelTipHalfWidth = 3.5;
const double labelGap = 13;
const double labelPadding = 15.5;
const double labelFontSize = 14;

class Dock extends StatefulWidget {
  const Dock({super.key, required this.items, this.magnification = defaultMagnification});

  final List<Item> items;
  final double magnification;

  @override
  State<Dock> createState() => _DockState();
}

class _DockState extends State<Dock> with SingleTickerProviderStateMixin {
  late final Ticker _ticker = createTicker(_tick);
  double? _pointerX;
  double _amount = 0;
  bool _inside = false;
  // Animazione dell'entrata o dell'uscita: da dove parte e quando.
  double _from = 0;
  Duration? _start;

  @override
  void dispose() {
    _ticker.dispose();
    super.dispose();
  }

  void _tick(Duration elapsed) {
    _start ??= elapsed;
    final duration = enterDuration(widget.magnification);
    final t = (elapsed - _start!).inMicroseconds / duration.inMicroseconds;
    final target = _inside ? 1.0 : 0.0;
    final eased = enterProgress(t);
    setState(() => _amount = _from + (target - _from) * eased);
    if (t >= 1) {
      _ticker.stop();
      _start = null;
      if (!_inside) _pointerX = null;
    }
  }

  void _setInside(bool inside) {
    if (inside == _inside) return;
    _inside = inside;
    _from = _amount;
    _start = null;
    if (_ticker.isActive) _ticker.stop();
    _ticker.start();
  }

  void _hover(Offset position, Size size) {
    final rest = layout(widget.items, width: size.width, height: size.height);
    final inside = inActiveZone(rest, position.dx, position.dy, size.height);
    setState(() {
      // L'ingrandimento segue il puntatore senza inerzia; uscendo resta l'ultima x.
      if (inside) _pointerX = position.dx;
    });
    _setInside(inside);
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, constraints) {
      final size = constraints.biggest;
      final l = layout(
        widget.items,
        width: size.width,
        height: size.height,
        pointerX: _pointerX,
        amount: _amount,
        magnification: widget.magnification,
      );
      final (bx, by, bw, bh) = l.background;
      final hovered = _inside && _pointerX != null ? itemAt(l, widget.items, _pointerX!) : null;
      return MouseRegion(
        onHover: (event) => _hover(event.localPosition, size),
        onExit: (_) => _setInside(false),
        child: Stack(
          children: [
            Positioned(
              left: bx,
              top: by,
              width: bw,
              height: bh,
              child: const LiquidGlass(
                radius: backgroundRadius,
                material: GlassMaterial.clear,
                interactive: false,
                child: SizedBox.expand(),
              ),
            ),
            Positioned.fill(
              child: CustomPaint(
                painter: _DockPainter(widget.items, l, size.height),
              ),
            ),
            if (hovered != null) _label(l, hovered, size.height),
          ],
        ),
      );
    });
  }

  Widget _label(Layout l, int index, double height) {
    final (_, iconTopY, _) = iconRect(l, index, height);
    final tipY = iconTopY - labelGap;
    final style = uiText(size: labelFontSize, color: labelText);
    final text = TextPainter(
      text: TextSpan(text: widget.items[index].label, style: style),
      textDirection: TextDirection.ltr,
    )..layout();
    final width = text.width + 2 * labelPadding;
    return Positioned(
      left: l.center(index) - width / 2,
      top: tipY - labelTip - labelHeight,
      width: width,
      height: labelHeight + labelTip,
      child: CustomPaint(
        painter: const _LabelPainter(),
        child: Padding(
          padding: const EdgeInsets.only(bottom: labelTip),
          child: Center(child: Text(widget.items[index].label, style: style)),
        ),
      ),
    );
  }
}

class _DockPainter extends CustomPainter {
  _DockPainter(this.items, this.l, this.height);

  final List<Item> items;
  final Layout l;
  final double height;

  @override
  void paint(Canvas canvas, Size size) {
    final (_, by, _, bh) = l.background;
    for (var i = 0; i < items.length; i++) {
      final item = items[i];
      if (item.kind == ItemKind.separator) {
        final x = l.center(i).roundToDouble() - 0.5;
        canvas.drawRect(
          Rect.fromLTWH(x, by + (bh - separatorHeight) / 2, 1, separatorHeight),
          Paint()..color = separatorColor,
        );
        continue;
      }
      final (ix, iy, side) = iconRect(l, i, height);
      paintIcon(canvas, Rect.fromLTWH(ix, iy, side, side), item.label, i);
      if (item.running) {
        canvas.drawCircle(
          Offset(l.center(i), by + bh - indicatorAbove),
          indicatorSize / 2,
          Paint()..color = indicatorColor,
        );
      }
    }
  }

  @override
  bool shouldRepaint(_DockPainter old) => true;
}

class _LabelPainter extends CustomPainter {
  const _LabelPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final body = RRect.fromRectAndRadius(
      Rect.fromLTWH(0.5, 0.5, size.width - 1, labelHeight - 1),
      const Radius.circular(labelHeight / 2),
    );
    final center = size.width / 2;
    final tip = Path()
      ..moveTo(center - labelTipHalfWidth, labelHeight - 1)
      ..lineTo(center, labelHeight + labelTip - 0.5)
      ..lineTo(center + labelTipHalfWidth, labelHeight - 1)
      ..close();
    final shape = Path()
      ..addRRect(body)
      ..addPath(tip, Offset.zero);
    canvas.drawPath(
      shape,
      Paint()
        ..shader = const LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          // La prima riga chiara (#d7d8de) e quella sotto ancora quasi uguale (215 nella
          // ripresa), poi il corpo.
          colors: [labelBodyTop, labelBodyTop, labelBody, labelBodyBottom],
          stops: [0, 0.07, 0.16, 1],
        ).createShader(Offset.zero & size),
    );
    canvas.drawPath(
      shape,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = labelBorder,
    );
  }

  @override
  bool shouldRepaint(_LabelPainter old) => false;
}
