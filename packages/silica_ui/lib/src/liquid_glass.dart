// Liquid Glass: vetro che reagisce al tocco. Alla pressione si gonfia con una molla, al
// rilascio torna con un piccolo rimbalzo; un riflesso morbido segue il puntatore. La forma
// gonfiata arriva anche al compositor (la regione segue le trasformazioni), quindi lente,
// bordo e fusione con il vetro vicino si muovono con lei.
//
// PROVVISORIO: scala della pressione, molle e riflesso non sono ancora misurati su macOS 27
// (plan/execution-plan.md, "Da misurare sul Mac per il Liquid Glass").

import 'package:flutter/physics.dart';
import 'package:flutter/widgets.dart';

import 'glass.dart';

/// Quanto si gonfia il vetro premuto (provvisorio).
const double liquidPressScale = 1.08;

/// Molla della pressione: rapida e quasi senza rimbalzo (provvisoria).
const SpringDescription liquidPressSpring = SpringDescription(
  mass: 1,
  stiffness: 600,
  damping: 40,
);

/// Molla del rilascio: un piccolo rimbalzo, come un liquido (provvisoria).
const SpringDescription liquidReleaseSpring = SpringDescription(
  mass: 1,
  stiffness: 350,
  damping: 18,
);

/// Un'area di Liquid Glass che reagisce al puntatore.
class LiquidGlass extends StatefulWidget {
  const LiquidGlass({
    super.key,
    this.radius = 0,
    this.material = GlassMaterial.regular,
    this.interactive = true,
    this.child,
  });

  final double radius;
  final GlassMaterial material;

  /// Senza interazione è un [Glass] e basta.
  final bool interactive;
  final Widget? child;

  @override
  State<LiquidGlass> createState() => _LiquidGlassState();
}

class _LiquidGlassState extends State<LiquidGlass>
    with SingleTickerProviderStateMixin {
  late final AnimationController _press = AnimationController.unbounded(
    vsync: this,
  )..addListener(() => setState(() {}));
  Offset? _pointer;

  @override
  void dispose() {
    _press.dispose();
    super.dispose();
  }

  void _springTo(double target, SpringDescription spring) {
    _press.animateWith(
      SpringSimulation(spring, _press.value, target, _press.velocity),
    );
  }

  @override
  Widget build(BuildContext context) {
    final glass = Glass(
      radius: widget.radius,
      material: widget.material,
      child: Stack(
        fit: StackFit.passthrough,
        children: [
          if (widget.child != null) widget.child!,
          if (widget.interactive && _pointer != null)
            Positioned.fill(
              child: IgnorePointer(
                child: CustomPaint(
                  painter: _HighlightPainter(_pointer!, widget.radius),
                ),
              ),
            ),
        ],
      ),
    );
    if (!widget.interactive) return glass;
    final scale = 1 + (liquidPressScale - 1) * _press.value;
    return MouseRegion(
      onHover: (event) => setState(() => _pointer = event.localPosition),
      onExit: (_) => setState(() => _pointer = null),
      child: Listener(
        // Tutta l'area di vetro risponde, anche dove il figlio è trasparente.
        behavior: HitTestBehavior.opaque,
        onPointerDown: (event) {
          _pointer = event.localPosition;
          _springTo(1, liquidPressSpring);
        },
        onPointerMove: (event) =>
            setState(() => _pointer = event.localPosition),
        onPointerUp: (_) => _springTo(0, liquidReleaseSpring),
        onPointerCancel: (_) => _springTo(0, liquidReleaseSpring),
        child: Transform.scale(scale: scale, child: glass),
      ),
    );
  }
}

/// Riflesso morbido attorno al puntatore, più forte verso il bordo (provvisorio).
class _HighlightPainter extends CustomPainter {
  _HighlightPainter(this.pointer, this.radius);

  final Offset pointer;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    final shape = RRect.fromRectAndRadius(
      Offset.zero & size,
      Radius.circular(radius),
    );
    canvas.save();
    canvas.clipRRect(shape);
    final reach = size.shortestSide;
    final paint = Paint()
      ..shader = RadialGradient(
        colors: const [Color(0x33FFFFFF), Color(0x00FFFFFF)],
      ).createShader(Rect.fromCircle(center: pointer, radius: reach));
    canvas.drawCircle(pointer, reach, paint);
    canvas.restore();
  }

  @override
  bool shouldRepaint(_HighlightPainter old) =>
      old.pointer != pointer || old.radius != radius;
}
