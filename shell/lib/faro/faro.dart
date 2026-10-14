// Faro, la ricerca delle app: da plan/reference/spotlight.md (aspetto chiaro). Pannello di
// vetro spesso centrato, con la cima al 30 % dello schermo; testata di 56 pt con la lente e
// il campo; vuoto: largo 384 pt con quattro pulsanti circolari da 54 pt (passo 64); con
// risultati: largo 640 pt, righe da 57 pt, al più 467 pt di altezza.
//
// Il componente resta sempre in esecuzione su un layer overlay a schermo intero: chiuso non
// disegna niente e non prende né puntatore né tastiera (silica/layer).

import 'dart:async';

import 'package:flutter/physics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import '../system/silica_icons.dart';
import 'launcher.dart';
import 'search.dart';

const double wideWidth = 640;
const double narrowWidth = 384;
const double topFraction = 0.30;
const double headerHeight = 56;
const double rowHeight = 57;

/// Icona dell'app nella riga: ~38–39 pt visibili (spotlight.md).
const double rowIconSide = 38;
const double maxHeight = 467;
const double radius = 28.5;

/// Pulsanti circolari dello stato vuoto: diametro 54, passo 64, 10 pt dal pannello.
const double circle = 54;
const double circlePitch = 64;

const Color placeholderColor = Color(0xFF494A4F);
const Color textColor = Color(0xFF16171C);
const Color selectedTextColor = Color(0xFF000003);
const Color subtitleColor = Color(0xFF88898E);

/// Riga selezionata e separatore: `#afb0b5` sopra il riempimento `#c8c9ce` (−25 livelli):
/// nero al ~12 %, così segue il materiale sopra sfondi diversi.
const Color selection = Color(0x20000000);

/// Opacità durante l'apertura (0, 13, 27, 53, 67 ms dal primo fotogramma).
const List<(double, double)> openOpacity = [
  (0, 0.22),
  (13.3, 0.66),
  (26.7, 0.79),
  (53.3, 0.98),
  (66.7, 1),
];

double opacityAt(double ms) {
  if (ms <= 0) return openOpacity.first.$2;
  for (var i = 1; i < openOpacity.length; i++) {
    final (t1, a1) = openOpacity[i];
    if (ms <= t1) {
      final (t0, a0) = openOpacity[i - 1];
      return a0 + (a1 - a0) * (ms - t0) / (t1 - t0);
    }
  }
  return 1;
}

/// Apertura del launcher: parte ~8 % più grande e si riduce (eccesso residuo misurato a 0,
/// 13, 40, 53, 67, 93, 107, 120 e 147 ms), con la stessa dissolvenza di Faro.
const double launcherOpenScale = 0.08;
const List<(double, double)> launcherExcess = [
  (0, 1),
  (13.3, 0.79),
  (40, 0.43),
  (53.3, 0.34),
  (66.7, 0.24),
  (93.3, 0.09),
  (106.7, 0.03),
  (120, 0.01),
  (146.7, 0),
];

double launcherScaleAt(double ms) {
  if (ms <= 0) return 1 + launcherOpenScale;
  for (var i = 1; i < launcherExcess.length; i++) {
    final (t1, e1) = launcherExcess[i];
    if (ms <= t1) {
      final (t0, e0) = launcherExcess[i - 1];
      return 1 + launcherOpenScale * (e0 + (e1 - e0) * (ms - t0) / (t1 - t0));
    }
  }
  return 1;
}

/// Esc nel launcher: il pannello si stringe verso Faro (altezza misurata, curva a S, 215 ms);
/// larghezza, posizione e raggio seguono lo stesso avanzamento.
const List<(double, double)> backHeights = [
  (0, 577),
  (26.7, 556),
  (40, 509),
  (53.3, 468),
  (80, 349),
  (93.3, 294),
  (106.7, 234),
  (133.3, 170),
  (146.7, 143),
  (160, 122),
  (186.7, 90),
  (200, 59),
  (213.3, 57),
];
const double backDuration = 213.3;

/// Avanzamento 0–1 del ritorno a Faro dopo `ms`.
double backProgress(double ms) {
  if (ms <= 0) return 0;
  final first = backHeights.first.$2;
  final last = backHeights.last.$2;
  for (var i = 1; i < backHeights.length; i++) {
    final (t1, h1) = backHeights[i];
    if (ms <= t1) {
      final (t0, h0) = backHeights[i - 1];
      final h = h0 + (h1 - h0) * (ms - t0) / (t1 - t0);
      return (first - h) / (first - last);
    }
  }
  return 1;
}

class Faro extends StatefulWidget {
  const Faro({
    super.key,
    required this.entries,
    this.startOpen = false,
    this.startLauncher = false,
    this.recents,
  });

  final Map<String, DesktopEntry> entries;
  final bool startOpen;

  /// Apre subito il launcher (prove e riprese).
  final bool startLauncher;

  /// Dove si ricordano le app recenti (predefinito: ~/.local/state/silica).
  final RecentApps? recents;

  @override
  State<Faro> createState() => _FaroState();
}

class _FaroState extends State<Faro> with TickerProviderStateMixin {
  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  late final AnimationController _open = AnimationController.unbounded(
    vsync: this,
  )..addListener(() => setState(() {}));
  StreamSubscription<String>? _commands;
  bool _visible = false;
  bool _launcher = false;
  int _selected = 0;
  DateTime? _openedAt;

  /// Il ritorno dal launcher (valore = ms dall'inizio).
  late final AnimationController _back = AnimationController(
    vsync: this,
    upperBound: backDuration,
    value: backDuration,
    duration: const Duration(microseconds: 213300),
  )..addListener(() => setState(() {}));
  late final RecentApps _recents = widget.recents ?? RecentApps();
  List<String> _recentIds = const [];

  @override
  void initState() {
    super.initState();
    _query.addListener(() => setState(() => _selected = 0));
    _commands = ShellChannel.instance.commands.listen((id) {
      if (id == 'faro.toggle') _toggle();
      if (id == 'launcher.toggle') _toggleLauncher();
    });
    if (widget.startLauncher) {
      _showLauncher();
    } else if (widget.startOpen) {
      _show();
    } else {
      LayerControl.inputRegion(const []);
      LayerControl.keyboard('none');
    }
  }

  @override
  void dispose() {
    _commands?.cancel();
    _open.dispose();
    _back.dispose();
    _query.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _toggle() => _visible && !_launcher ? _hide() : _show();

  /// Esc dal launcher: niente dissolvenza, il pannello si trasforma in Faro.
  void _backToFaro() {
    setState(() {
      _launcher = false;
      _openedAt = null;
      _query.clear();
    });
    _open.value = 1;
    _back.forward(from: 0);
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _toggleLauncher() => _visible && _launcher ? _hide() : _showLauncher();

  void _showLauncher() {
    setState(() {
      _visible = true;
      _launcher = true;
      _openedAt = DateTime.now();
      _recentIds = _recents.load();
    });
    // Il controller ricostruisce a ogni fotogramma finché dura l'apertura.
    _open.value = 0;
    _open.animateTo(1, duration: const Duration(milliseconds: 200));
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
  }

  void _show() {
    setState(() {
      _visible = true;
      _launcher = false;
      _openedAt = DateTime.now();
      _query.clear();
    });
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
    // Altezza: da 0,966 a 1 con un piccolo superamento (+2 pt su 447).
    _open.value = 0;
    _open.animateWith(
      SpringSimulation(
        const SpringDescription(mass: 1, stiffness: 700, damping: 42),
        0,
        1,
        0,
      ),
    );
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _hide() {
    setState(() => _visible = false);
    LayerControl.keyboard('none');
    LayerControl.inputRegion(const []);
  }

  void _launch(DesktopEntry entry) {
    ShellChannel.instance.launch(entry.exec);
    _recents.add(entry.id);
    _hide();
  }

  KeyEventResult _key(
    FocusNode node,
    KeyEvent event,
    List<DesktopEntry> results,
  ) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent)
      return KeyEventResult.ignored;
    switch (event.logicalKey) {
      case LogicalKeyboardKey.escape:
        _hide();
      case LogicalKeyboardKey.arrowDown:
        setState(
          () => _selected = (_selected + 1).clamp(0, results.length - 1),
        );
      case LogicalKeyboardKey.arrowUp:
        setState(
          () => _selected = (_selected - 1).clamp(0, results.length - 1),
        );
      case LogicalKeyboardKey.enter || LogicalKeyboardKey.numpadEnter:
        if (results.isNotEmpty)
          _launch(results[_selected.clamp(0, results.length - 1)]);
      default:
        return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  FaroColors get _c => FaroColors.of(context);

  @override
  Widget build(BuildContext context) {
    if (!_visible) return const SizedBox.expand();
    if (_launcher) return _launcherView();
    final results = searchApps(widget.entries.values, _query.text);
    final searching = _query.text.trim().isNotEmpty;
    final width = searching ? wideWidth : narrowWidth;
    final listHeight = searching
        ? (results.length * rowHeight).clamp(0.0, maxHeight - headerHeight - 1)
        : 0.0;
    final height = headerHeight + (searching ? 1 + listHeight : 0);
    final elapsed = _openedAt == null
        ? 1000.0
        : DateTime.now().difference(_openedAt!).inMicroseconds / 1000;
    final opacity = opacityAt(elapsed);
    final scaleY = 0.966 + 0.034 * _open.value;
    if (elapsed < 70) WidgetsBinding.instance.scheduleFrame();

    return LayoutBuilder(
      builder: (context, constraints) {
        final screen = constraints.biggest;
        var left = (screen.width - wideWidth) / 2;
        final top = screen.height * topFraction;
        // Dal launcher: dimensioni e posizione dal pannello grande a quello di Faro.
        final back = _back.isAnimating ? backProgress(_back.value) : 1.0;
        var panelWidth = width;
        var panelHeight = height;
        var panelRadius = radius;
        if (back < 1) {
          double lerp(double a, double b) => a + (b - a) * back;
          left = lerp((screen.width - launcherWidth) / 2, left);
          panelWidth = lerp(launcherWidth, width);
          panelHeight = lerp(launcherHeight, height);
          panelRadius = lerp(launcherRadius, radius);
        }
        return Stack(
          children: [
            // Un clic fuori chiude Faro.
            Positioned.fill(
              child: GestureDetector(
                onTap: _hide,
                behavior: HitTestBehavior.opaque,
              ),
            ),
            Positioned(
              left: left,
              top: top,
              width: panelWidth,
              height: panelHeight,
              child: Transform(
                alignment: Alignment.center,
                transform: Matrix4.diagonal3Values(1, scaleY, 1),
                child: Opacity(
                  opacity: opacity,
                  child: _panel(
                    results,
                    searching,
                    listHeight,
                    panelRadius: panelRadius,
                  ),
                ),
              ),
            ),
            if (!searching && back >= 1)
              for (var i = 0; i < 4; i++)
                Positioned(
                  left: left + narrowWidth + 10 + i * circlePitch,
                  top: top + (headerHeight - circle) / 2,
                  width: circle,
                  height: circle,
                  child: Opacity(opacity: opacity, child: _circleButton(i)),
                ),
          ],
        );
      },
    );
  }

  Widget _panel(
    List<DesktopEntry> results,
    bool searching,
    double listHeight, {
    double panelRadius = radius,
  }) {
    return OuterShadow(
      // Ombra spostata in basso (~20 pt): sotto si estende ~65 pt, sopra ~25, ai lati ~45.
      radius: panelRadius,
      blurRadius: 42,
      offset: const Offset(0, 20),
      color: _c.shadow(const Color(0x40000000)),
      child: LiquidGlass(
        radius: panelRadius,
        material: GlassMaterial.thick,
        interactive: false,
        child: Column(
          children: [
            SizedBox(height: headerHeight, child: _field(results)),
            if (searching) ...[
              Container(
                height: 1,
                margin: const EdgeInsets.symmetric(horizontal: 20),
                color: _c.selection,
              ),
              SizedBox(
                height: listHeight,
                child: ListView.builder(
                  padding: EdgeInsets.zero,
                  itemExtent: rowHeight,
                  itemCount: results.length,
                  itemBuilder: (context, i) => _row(results[i], i == _selected),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _field(List<DesktopEntry> results) {
    return Focus(
      onKeyEvent: (node, event) => _key(node, event, results),
      child: Row(
        children: [
          const SizedBox(width: 23),
          CustomPaint(
            size: const Size(23, 23),
            painter: _LensPainter(_c.placeholder),
          ),
          const SizedBox(width: 16),
          Expanded(
            child: Stack(
              alignment: Alignment.centerLeft,
              children: [
                if (_query.text.isEmpty)
                  IgnorePointer(
                    child: Text(
                      'Cerca con Faro',
                      style: uiText(size: 26, color: _c.placeholder),
                    ),
                  ),
                EditableText(
                  controller: _query,
                  focusNode: _focus,
                  style: uiText(size: 26, color: _c.text),
                  cursorColor: _c.text,
                  cursorWidth: 2,
                  cursorHeight: 31,
                  backgroundCursorColor: const Color(0x00000000),
                ),
              ],
            ),
          ),
          const SizedBox(width: 23),
        ],
      ),
    );
  }

  Widget _row(DesktopEntry entry, bool selected) {
    final layered = silicaIconFor(entry);
    final flat = findIcon(entry.icon);
    final IconSource? icon = layered != null
        ? LayeredSource(layered)
        : flat != null
        ? FlatSource(flat)
        : null;
    return GestureDetector(
      onTap: () => _launch(entry),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: selected ? _c.selection : null,
            borderRadius: const BorderRadius.all(Radius.circular(16.5)),
          ),
          child: Row(
            children: [
              const SizedBox(width: 9),
              SizedBox(
                width: 38,
                height: 38,
                child: icon == null
                    ? null
                    : Center(child: SilicaIcon(icon, side: rowIconSide)),
              ),
              const SizedBox(width: 15),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      entry.name,
                      style: uiText(
                        size: 16.5,
                        color: selected ? _c.selectedText : _c.text,
                      ),
                    ),
                    Text(
                      'Applicazione',
                      style: uiText(size: 15, color: _c.subtitle),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _circleButton(int index) => GestureDetector(
    // Il primo pulsante apre il launcher.
    onTap: index == 0 ? _showLauncher : null,
    child: LiquidGlass(
      radius: circle / 2,
      material: GlassMaterial.thick,
      interactive: true,
      child: CustomPaint(painter: _CircleGlyphPainter(index, _c.placeholder)),
    ),
  );

  Widget _launcherView() {
    final elapsed = _openedAt == null
        ? 1000.0
        : DateTime.now().difference(_openedAt!).inMicroseconds / 1000;
    if (elapsed < 160) WidgetsBinding.instance.scheduleFrame();
    return LayoutBuilder(
      builder: (context, constraints) {
        final screen = constraints.biggest;
        return Stack(
          children: [
            Positioned.fill(
              child: GestureDetector(
                onTap: _hide,
                behavior: HitTestBehavior.opaque,
              ),
            ),
            Positioned(
              left: (screen.width - launcherWidth) / 2,
              top: screen.height * topFraction,
              width: launcherWidth,
              height: launcherHeight,
              child: Transform.scale(
                scale: launcherScaleAt(elapsed),
                child: Opacity(
                  opacity: opacityAt(elapsed),
                  // Ombra spostata in basso di ~17 pt, estesa ~35 pt.
                  child: OuterShadow(
                    radius: launcherRadius,
                    blurRadius: 36,
                    color: _c.shadow(const Color(0x5C000000)),
                    offset: const Offset(0, 17),
                    child: LiquidGlass(
                      radius: launcherRadius,
                      material: GlassMaterial.thick,
                      interactive: false,
                      child: Launcher(
                        entries: widget.entries,
                        recents: _recentIds,
                        onLaunch: _launch,
                        onBack: _backToFaro,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _LensPainter extends CustomPainter {
  const _LensPainter(this.color);
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.4
      ..strokeCap = StrokeCap.round;
    canvas.drawCircle(
      Offset(size.width * 0.42, size.height * 0.42),
      size.width * 0.32,
      paint,
    );
    canvas.drawLine(
      Offset(size.width * 0.66, size.height * 0.66),
      Offset(size.width * 0.94, size.height * 0.94),
      paint,
    );
  }

  @override
  bool shouldRepaint(_LensPainter old) => old.color != color;
}

/// Glifi generici dei quattro pulsanti: app, file, azioni, appunti.
class _CircleGlyphPainter extends CustomPainter {
  const _CircleGlyphPainter(this.index, this.color);

  final int index;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2
      ..strokeJoin = StrokeJoin.round;
    switch (index) {
      case 0:
        for (final dx in [-5.0, 5.0]) {
          for (final dy in [-5.0, 5.0]) {
            canvas.drawRRect(
              RRect.fromRectAndRadius(
                Rect.fromCenter(
                  center: c + Offset(dx, dy),
                  width: 8,
                  height: 8,
                ),
                const Radius.circular(2.5),
              ),
              paint,
            );
          }
        }
      case 1:
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromCenter(center: c, width: 20, height: 15),
            const Radius.circular(3),
          ),
          paint,
        );
        canvas.drawLine(
          c + const Offset(-10, -3),
          c + const Offset(10, -3),
          paint,
        );
      case 2:
        final bolt = Path()
          ..moveTo(c.dx + 2, c.dy - 11)
          ..lineTo(c.dx - 6, c.dy + 2)
          ..lineTo(c.dx, c.dy + 2)
          ..lineTo(c.dx - 2, c.dy + 11)
          ..lineTo(c.dx + 6, c.dy - 2)
          ..lineTo(c.dx, c.dy - 2)
          ..close();
        canvas.drawPath(bolt, paint);
      default:
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromCenter(
              center: c + const Offset(0, 2),
              width: 16,
              height: 22,
            ),
            const Radius.circular(3),
          ),
          paint,
        );
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromCenter(
              center: c + const Offset(0, -9),
              width: 8,
              height: 5,
            ),
            const Radius.circular(1.5),
          ),
          paint,
        );
    }
  }

  @override
  bool shouldRepaint(_CircleGlyphPainter old) =>
      old.index != index || old.color != color;
}

/// L'ombra di un pannello di vetro disegnata solo fuori dal pannello: sotto il vetro, che
/// Ossido compone dietro la superficie, non deve scurire niente (un BoxShadow lo farebbe).
class OuterShadow extends StatelessWidget {
  const OuterShadow({
    super.key,
    required this.radius,
    required this.blurRadius,
    required this.offset,
    required this.child,
    this.color = const Color(0x40000000),
  });

  final double radius;
  final double blurRadius;
  final Offset offset;
  final Color color;
  final Widget child;

  @override
  Widget build(BuildContext context) => CustomPaint(
    painter: _OuterShadowPainter(radius, blurRadius, offset, color),
    child: child,
  );
}

class _OuterShadowPainter extends CustomPainter {
  const _OuterShadowPainter(this.radius, this.blur, this.offset, this.color);

  final double radius;
  final double blur;
  final Offset offset;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final panel = RRect.fromRectAndRadius(
      Offset.zero & size,
      Radius.circular(radius),
    );
    canvas
      ..save()
      ..clipPath(
        Path()
          ..fillType = PathFillType.evenOdd
          ..addRect((Offset.zero & size).inflate(blur * 3 + offset.distance))
          ..addRRect(panel),
      )
      ..drawRRect(
        panel.shift(offset),
        Paint()
          ..color = color
          ..maskFilter = MaskFilter.blur(
            BlurStyle.normal,
            Shadow.convertRadiusToSigma(blur),
          ),
      )
      ..restore();
  }

  @override
  bool shouldRepaint(_OuterShadowPainter old) =>
      old.radius != radius ||
      old.blur != blur ||
      old.offset != offset ||
      old.color != color;
}
