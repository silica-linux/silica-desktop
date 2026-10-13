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
import '../system/shell_icon.dart';
import '../system/silica_icons.dart';
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

class Faro extends StatefulWidget {
  const Faro({super.key, required this.entries, this.startOpen = false});

  final Map<String, DesktopEntry> entries;
  final bool startOpen;

  @override
  State<Faro> createState() => _FaroState();
}

class _FaroState extends State<Faro> with SingleTickerProviderStateMixin {
  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  late final AnimationController _open = AnimationController.unbounded(
    vsync: this,
  )..addListener(() => setState(() {}));
  StreamSubscription<String>? _commands;
  bool _visible = false;
  int _selected = 0;
  DateTime? _openedAt;

  @override
  void initState() {
    super.initState();
    _query.addListener(() => setState(() => _selected = 0));
    _commands = ShellChannel.instance.commands.listen((id) {
      if (id == 'faro.toggle') _toggle();
    });
    if (widget.startOpen) {
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
    _query.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _toggle() => _visible ? _hide() : _show();

  void _show() {
    setState(() {
      _visible = true;
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

  @override
  Widget build(BuildContext context) {
    if (!_visible) return const SizedBox.expand();
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
        final left = (screen.width - wideWidth) / 2;
        final top = screen.height * topFraction;
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
              width: width,
              height: height,
              child: Transform(
                alignment: Alignment.center,
                transform: Matrix4.diagonal3Values(1, scaleY, 1),
                child: Opacity(
                  opacity: opacity,
                  child: _panel(results, searching, listHeight),
                ),
              ),
            ),
            if (!searching)
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

  Widget _panel(List<DesktopEntry> results, bool searching, double listHeight) {
    return DecoratedBox(
      // Ombra spostata in basso (~20 pt): sotto si estende ~65 pt, sopra ~25, ai lati ~45.
      decoration: const BoxDecoration(
        borderRadius: BorderRadius.all(Radius.circular(radius)),
        boxShadow: [
          BoxShadow(
            color: Color(0x40000000),
            blurRadius: 42,
            offset: Offset(0, 20),
          ),
        ],
      ),
      child: LiquidGlass(
        radius: radius,
        material: GlassMaterial.thick,
        interactive: false,
        child: Column(
          children: [
            SizedBox(height: headerHeight, child: _field(results)),
            if (searching) ...[
              Container(
                height: 1,
                margin: const EdgeInsets.symmetric(horizontal: 20),
                color: selection,
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
          const CustomPaint(size: Size(23, 23), painter: _LensPainter()),
          const SizedBox(width: 16),
          Expanded(
            child: Stack(
              alignment: Alignment.centerLeft,
              children: [
                if (_query.text.isEmpty)
                  IgnorePointer(
                    child: Text(
                      'Cerca con Faro',
                      style: uiText(size: 26, color: placeholderColor),
                    ),
                  ),
                EditableText(
                  controller: _query,
                  focusNode: _focus,
                  style: uiText(size: 26, color: textColor),
                  cursorColor: textColor,
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
            color: selected ? selection : null,
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
                    : Center(child: ShellIcon(icon, side: rowIconSide)),
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
                        color: selected ? selectedTextColor : textColor,
                      ),
                    ),
                    Text(
                      'Applicazione',
                      style: uiText(size: 15, color: subtitleColor),
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

  Widget _circleButton(int index) => LiquidGlass(
    radius: circle / 2,
    material: GlassMaterial.thick,
    interactive: true,
    child: CustomPaint(painter: _CircleGlyphPainter(index)),
  );
}

class _LensPainter extends CustomPainter {
  const _LensPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = placeholderColor
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
  bool shouldRepaint(_LensPainter old) => false;
}

/// Glifi generici dei quattro pulsanti: app, file, azioni, appunti.
class _CircleGlyphPainter extends CustomPainter {
  const _CircleGlyphPainter(this.index);

  final int index;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final paint = Paint()
      ..color = placeholderColor
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
  bool shouldRepaint(_CircleGlyphPainter old) => old.index != index;
}
