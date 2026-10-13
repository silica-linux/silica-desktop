// Il Dock della shell: disposizione e ingrandimento da model.dart (misure in
// plan/reference/dock.md), fondo di Liquid Glass calcolato da Ossido, icone, separatore,
// indicatori ed etichetta disegnati qui.

import 'dart:async';

import 'package:flutter/scheduler.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/shell_channel.dart';
import '../system/shell_icon.dart';
import 'apps.dart';
import 'icons.dart';
import 'model.dart';

/// Il Dock con le app vere: installate (.desktop), fissate e aperte (da Ossido).
class LiveDock extends StatefulWidget {
  const LiveDock({super.key});

  @override
  State<LiveDock> createState() => _LiveDockState();
}

class _LiveDockState extends State<LiveDock> {
  late final Map<String, DesktopEntry> _entries = loadDesktopEntries();
  late final List<String> _pinned = pinnedIds(_entries);

  @override
  void initState() {
    super.initState();
    ShellChannel.instance.toplevels.addListener(_changed);
    ShellChannel.instance.refresh();
  }

  @override
  void dispose() {
    ShellChannel.instance.toplevels.removeListener(_changed);
    super.dispose();
  }

  void _changed() => setState(() {});

  @override
  Widget build(BuildContext context) => Dock(
    items: dockItems(_entries, _pinned, ShellChannel.instance.toplevels.value),
    onActivate: (item) {
      // Un'app aperta torna in primo piano; altrimenti si avvia.
      if (item.windows.isNotEmpty) {
        ShellChannel.instance.activate(item.windows.first);
      } else if (item.command case final command?) {
        ShellChannel.instance.launch(command);
      }
    },
  );
}

/// Colori dell'aspetto chiaro (dock.md).
const Color separatorColor = Color(0xFF484950);
const Color indicatorColor = Color(0xFF03040B);
const Color labelBody = Color(0xFFBDBEC3);

/// Icona premuta: scurita (stima, il valore non è misurato).
const Color pressedShade = Color(0x40000000);
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
  const Dock({
    super.key,
    required this.items,
    this.magnification = defaultMagnification,
    this.onActivate,
  });

  final List<Item> items;
  final double magnification;

  /// Clic su un'app.
  final void Function(Item item)? onActivate;

  @override
  State<Dock> createState() => _DockState();
}

class _DockState extends State<Dock> with TickerProviderStateMixin {
  late final Ticker _ticker = createTicker(_tick);

  /// L'etichetta: l'elemento a cui appartiene e la sua opacità.
  int? _labelIndex;
  late final AnimationController _labelAnim = AnimationController(vsync: this)
    ..addListener(() => setState(() {}));
  Timer? _labelTimer;

  /// L'app premuta (icona scurita) e quelle che rimbalzano, con l'istante d'inizio.
  int? _pressed;
  final Map<String, DateTime> _bounces = {};
  late final Ticker _bounceTicker = createTicker((_) {
    _endBounces();
    setState(() {});
  });
  double? _pointerX;
  double _amount = 0;
  bool _inside = false;
  // Animazione dell'entrata o dell'uscita: da dove parte e quando.
  double _from = 0;
  Duration? _start;

  @override
  void dispose() {
    _ticker.dispose();
    _bounceTicker.dispose();
    _labelAnim.dispose();
    _labelTimer?.cancel();
    super.dispose();
  }

  static String _key(Item item) => '${item.label}\u0000${item.command}';

  /// Un salto in corso finisce a terra quando l'app ha una finestra, o dopo il limite.
  void _endBounces() {
    final now = DateTime.now();
    _bounces.removeWhere((key, start) {
      final ms = now.difference(start).inMicroseconds / 1000;
      final item = widget.items.where((i) => _key(i) == key).firstOrNull;
      final landed = ms % bouncePeriod < 30 && ms > bouncePeriod / 2;
      return ms > bounceLimit.inMilliseconds ||
          item == null ||
          (item.running && landed);
    });
    if (_bounces.isEmpty && _bounceTicker.isActive) _bounceTicker.stop();
  }

  double _bounce(int index) {
    final start = _bounces[_key(widget.items[index])];
    if (start == null) return 0;
    return bounceHeight(DateTime.now().difference(start).inMicroseconds / 1000);
  }

  void _activate(int index) {
    final item = widget.items[index];
    if (!item.running && item.command != null) {
      _bounces.putIfAbsent(_key(item), DateTime.now);
      if (!_bounceTicker.isActive) _bounceTicker.start();
    }
    widget.onActivate?.call(item);
  }

  /// L'etichetta segue la cella sotto il puntatore: subito da un'icona all'altra.
  void _showLabel(int? index) {
    if (index != null) {
      _labelTimer?.cancel();
      _labelTimer = null;
      final hidden = _labelIndex == null || _labelAnim.value < 1;
      _labelIndex = index;
      if (hidden) {
        _labelAnim.animateTo(1, duration: labelAppear);
      }
      return;
    }
    if (_labelIndex == null || _labelTimer != null) return;
    _labelTimer = Timer(labelHold, () {
      _labelTimer = null;
      _labelAnim.animateTo(0, duration: labelVanish).whenComplete(() {
        if (_labelAnim.value == 0 && mounted)
          setState(() => _labelIndex = null);
      });
    });
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
    final current = layout(
      widget.items,
      width: size.width,
      height: size.height,
      pointerX: _pointerX,
      amount: _amount,
      magnification: widget.magnification,
    );
    // Si entra dalla soglia fissa; si esce solo lasciando la cella ingrandita (isteresi).
    final inside = _inside
        ? staysInActiveZone(
            rest,
            current,
            widget.items,
            position.dx,
            position.dy,
            size.height,
          )
        : inActiveZone(rest, position.dx, position.dy, size.height);
    setState(() {
      // L'ingrandimento segue il puntatore senza inerzia; uscendo resta l'ultima x.
      if (inside) _pointerX = position.dx;
    });
    _setInside(inside);
    _showLabel(inside ? itemAt(current, widget.items, position.dx) : null);
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
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
        final label = _labelIndex;
        final hovered = label != null && label < widget.items.length
            ? label
            : null;
        return MouseRegion(
          onHover: (event) => _hover(event.localPosition, size),
          onExit: (_) {
            _setInside(false);
            _showLabel(null);
          },
          child: GestureDetector(
            behavior: HitTestBehavior.translucent,
            onTapDown: (details) => setState(
              () =>
                  _pressed = itemAt(l, widget.items, details.localPosition.dx),
            ),
            onTapCancel: () => setState(() => _pressed = null),
            onTapUp: (details) {
              final index = itemAt(l, widget.items, details.localPosition.dx);
              setState(() => _pressed = null);
              if (index != null) _activate(index);
            },
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
                for (var i = 0; i < widget.items.length; i++)
                  if (widget.items[i].icon case final source?)
                    _icon(l, i, size.height, source),
                if (hovered != null)
                  _label(
                    l,
                    hovered,
                    size.height,
                    _labelAnim.value.clamp(0.0, 1.0),
                  ),
              ],
            ),
          ),
        );
      },
    );
  }

  /// L'icona di un'app: a livelli da silica-icons, o dal tema del sistema.
  Widget _icon(Layout l, int index, double height, IconSource source) {
    final (x, y, side) = iconRect(l, index, height);
    final icon = ShellIcon(source, side: side);
    return Positioned(
      left: x,
      top: y - _bounce(index),
      width: side,
      height: side,
      // Premuta: l'icona si scurisce (D-02).
      child: _pressed == index
          ? ColorFiltered(
              colorFilter: const ColorFilter.mode(
                pressedShade,
                BlendMode.srcATop,
              ),
              child: icon,
            )
          : icon,
    );
  }

  Widget _label(Layout l, int index, double height, double opacity) {
    final (_, iconTop, _) = iconRect(l, index, height);
    // Durante il rimbalzo l'etichetta sale con l'icona.
    final iconTopY = iconTop - _bounce(index);
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
      child: Opacity(
        opacity: opacity,
        child: CustomPaint(
          painter: const _LabelPainter(),
          child: Padding(
            padding: const EdgeInsets.only(bottom: labelTip),
            child: Center(child: Text(widget.items[index].label, style: style)),
          ),
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
      if (item.icon == null) {
        paintIcon(canvas, Rect.fromLTWH(ix, iy, side, side), item.label, i);
      }
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
