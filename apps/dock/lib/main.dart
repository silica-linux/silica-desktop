// Prototipo del Dock (M1.5): la stessa specifica di crates/dock-iced, in Flutter.
// Specifica: docs/m1.5-dock.md.

import 'dart:math' as math;

import 'package:flutter/physics.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

void main() => runApp(const DockApp());

const int iconCount = 12;
const double iconSize = 48;
const double spacing = 8;
const double step = iconSize + spacing;
const double padding = 8;
const double bottomMargin = 8;
const double influence = 3;
/// Raggio del fondo: arco di cerchio di 22 pt, misurato sul Dock (dock.md).
const double backgroundRadius = 22;
const double restRowWidth = iconCount * iconSize + (iconCount - 1) * spacing;
const SpringDescription spring =
    SpringDescription(mass: 1, stiffness: 400, damping: 30);

/// Quanto cresce un'icona a distanza `d` (in passi) dal puntatore: 0..1.
double bump(double d) =>
    d < influence ? (1 + math.cos(math.pi * d / influence)) / 2 : 0;

/// Il centro a riposo dell'icona `i`, in una superficie larga `width`.
double restCenter(double width, int i) =>
    (width - restRowWidth) / 2 + i * step + iconSize / 2;

/// I lati delle icone con il puntatore a `pointerX` e l'ingrandimento `m`.
List<double> iconSizes(double width, double? pointerX, double m) =>
    List.generate(iconCount, (i) {
      if (pointerX == null) return iconSize;
      final d = (pointerX - restCenter(width, i)).abs() / step;
      return iconSize * (1 + m * bump(d));
    });

/// L'icona più vicina al puntatore.
int nearestIcon(double width, double pointerX) {
  final index = ((pointerX - restCenter(width, 0)) / step).round();
  return index.clamp(0, iconCount - 1);
}

/// Il gradiente dell'icona `i`: tinta i·30°, dalla luminosità 60% alla 40%.
List<Color> iconColors(int i) => [
      HSLColor.fromAHSL(1, i * 30.0, 0.7, 0.6).toColor(),
      HSLColor.fromAHSL(1, i * 30.0, 0.7, 0.4).toColor(),
    ];

class DockApp extends StatelessWidget {
  const DockApp({super.key});

  @override
  Widget build(BuildContext context) => const Directionality(
        textDirection: TextDirection.ltr,
        child: Dock(),
      );
}

class Dock extends StatefulWidget {
  const Dock({super.key});

  @override
  State<Dock> createState() => _DockState();
}

class _DockState extends State<Dock> with SingleTickerProviderStateMixin {
  /// L'ingrandimento `m`, mosso dalla molla.
  late final AnimationController _magnification =
      AnimationController.unbounded(vsync: this);
  double? _pointerX;
  bool _hovering = false;

  void _springTo(double target) {
    _magnification.animateWith(SpringSimulation(
      spring,
      _magnification.value,
      target,
      _magnification.velocity,
    ));
  }

  @override
  void dispose() {
    _magnification.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (event) {
        setState(() {
          _hovering = true;
          _pointerX = event.localPosition.dx;
        });
        _springTo(1);
      },
      onHover: (event) => setState(() => _pointerX = event.localPosition.dx),
      onExit: (_) {
        setState(() => _hovering = false);
        _springTo(0);
      },
      child: LayoutBuilder(
        builder: (context, constraints) => AnimatedBuilder(
          animation: _magnification,
          builder: (context, _) => _paint(constraints.maxWidth),
        ),
      ),
    );
  }

  Widget _paint(double width) {
    final sizes = iconSizes(width, _pointerX, _magnification.value);
    final rowWidth =
        sizes.fold<double>(0, (sum, side) => sum + side) + (iconCount - 1) * spacing;
    final rowLeft = (width - rowWidth) / 2;
    final backgroundWidth = rowWidth + 2 * padding;

    final children = <Widget>[
      Positioned(
        left: (width - backgroundWidth) / 2,
        bottom: bottomMargin,
        width: backgroundWidth,
        height: iconSize + 2 * padding,
        // Il fondo è Liquid Glass di Ossido (silica-glass-v1) con il materiale misurato sul
        // Dock (lente leggera, bordo luminoso): l'app lo lascia trasparente.
        child: const LiquidGlass(
          radius: backgroundRadius,
          material: GlassMaterial.clear,
          interactive: false,
          child: SizedBox.expand(),
        ),
      ),
    ];

    var left = rowLeft;
    for (var i = 0; i < iconCount; i++) {
      final side = sizes[i];
      children.add(Positioned(
        left: left,
        bottom: bottomMargin + padding,
        width: side,
        height: side,
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(side * 0.22),
            gradient: LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: iconColors(i),
            ),
          ),
        ),
      ));
      left += side + spacing;
    }

    final pointerX = _pointerX;
    if (_hovering && pointerX != null) {
      final i = nearestIcon(width, pointerX);
      final before = sizes.take(i).fold<double>(0, (sum, side) => sum + side);
      final center = rowLeft + before + i * spacing + sizes[i] / 2;
      children.add(Positioned(
        left: center,
        bottom: bottomMargin + padding + sizes[i] + 8,
        child: FractionalTranslation(
          translation: const Offset(-0.5, 0),
          child: DecoratedBox(
            decoration: const BoxDecoration(
              color: Color(0xA0000000),
              borderRadius: BorderRadius.all(Radius.circular(6)),
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              child: Text(
                'App ${i + 1}',
                style: uiText(size: 13, color: const Color(0xFFFFFFFF)),
              ),
            ),
          ),
        ),
      ));
    }

    return Stack(children: children);
  }
}
