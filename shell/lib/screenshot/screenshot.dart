// Le istantanee (plan/reference/screenshot-ui.md): la miniatura dopo ogni cattura, il mirino
// di ⇧⌘4 (con Spazio: una finestra) e la barra di ⇧⌘5.
//
// - Miniatura: in basso a destra a 12 pt dai bordi, 264 × 152 (l'immagine 256 × 144 con una
//   cornice bianca di 4 pt) e un'ombra morbida di ~20 pt; si assesta in ~400 ms, resta ~5,1 s,
//   poi esce a destra in ~250 ms. Il file va sulla Scrivania quando la miniatura sparisce; un
//   clic lo salva subito e lo apre.
// - Mirino: una croce sottile nera di ~20 pt con un disco grigio traslucido al centro e, in
//   basso a destra, le coordinate in punti su due righe. Trascinando si sceglie la regione;
//   Spazio passa alla finestra sotto il puntatore (una macchina fotografica); Esc annulla.
// - Barra: vetro ~545 × 52, 4 pt sopra la zona del Dock: Chiudi, schermo intero, finestra,
//   selezione, Opzioni (dove salvare) e Acquisisci (blu). La registrazione non c'è.
//
// Ossido cattura (`screenshot.screen`, `screenshot.region`, `screenshot.window_at`), scrive un
// PNG temporaneo e lo annuncia con broadcast "screenshot" = "<percorso> <l> <a>".

import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/layer_arbiter.dart';
import '../system/shell_channel.dart';

const Size thumbnailImage = Size(256, 144);
const double thumbnailBorder = 4;
const double thumbnailMargin = 12;
const Duration thumbnailSettle = Duration(milliseconds: 400);
const Duration thumbnailHold = Duration(milliseconds: 5100);
const Duration thumbnailLeave = Duration(milliseconds: 250);

const Size toolbarSize = Size(545, 52);

/// Dalla zona del Dock (zona esclusiva di 84 pt) alla barra: 4 pt.
const double toolbarAboveDock = 84 + 4;

/// Dove vanno le istantanee: la Scrivania di xdg-user-dirs, o ~/Desktop.
String screenshotFolder({Map<String, String>? env}) {
  final e = env ?? Platform.environment;
  final home = e['HOME'] ?? '/tmp';
  final config = e['XDG_CONFIG_HOME'] ?? '$home/.config';
  try {
    final dirs = File('$config/user-dirs.dirs');
    if (dirs.existsSync()) {
      for (final line in dirs.readAsLinesSync()) {
        if (line.startsWith('XDG_DESKTOP_DIR=')) {
          final value = line
              .substring('XDG_DESKTOP_DIR='.length)
              .replaceAll('"', '')
              .replaceAll(r'$HOME', home);
          if (Directory(value).existsSync()) return value;
        }
      }
    }
  } on FileSystemException {
    // Si prova con i nomi soliti.
  }
  for (final name in ['Scrivania', 'Desktop']) {
    if (Directory('$home/$name').existsSync()) return '$home/$name';
  }
  return home;
}

/// Il nome del file: "Schermata del 2026-10-14 alle 04.12.33.png" (un numero se esiste già).
String screenshotName(DateTime t, bool Function(String name) exists) {
  String two(int v) => v.toString().padLeft(2, '0');
  final base =
      'Schermata del ${t.year}-${two(t.month)}-${two(t.day)} alle '
      '${two(t.hour)}.${two(t.minute)}.${two(t.second)}';
  var name = '$base.png';
  for (var i = 2; exists(name); i++) {
    name = '$base ($i).png';
  }
  return name;
}

enum _Mode { none, selection, window, toolbar }

enum ToolbarChoice { screen, window, selection }

/// I colori della barra (sul vetro normale). Lo scuro non è osservato (screenshot-ui.md,
/// "Non osservato"): *stimato* come i testi chiari sul vetro scuro (glifi e testo `#ebebeb`,
/// i fondi neri traslucidi diventano bianchi traslucidi).
@immutable
class ShotBarColors {
  const ShotBarColors({
    required this.glyph,
    required this.closeFill,
    required this.closeGlyph,
    required this.line,
    required this.chosen,
  });

  final Color glyph;
  final Color closeFill;
  final Color closeGlyph;
  final Color line;
  final Color chosen;

  static const ShotBarColors light = ShotBarColors(
    glyph: Color(0xFF1D1D1F),
    closeFill: Color(0x33000000),
    closeGlyph: Color(0xFF3C3C3E),
    line: Color(0x33000000),
    chosen: Color(0x22000000),
  );

  static const ShotBarColors dark = ShotBarColors(
    glyph: Color(0xFFEBEBEB),
    closeFill: Color(0x33FFFFFF),
    closeGlyph: Color(0xFFC7C7C9),
    line: Color(0x33FFFFFF),
    chosen: Color(0x26FFFFFF),
  );

  static ShotBarColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

class ScreenshotLayer extends StatefulWidget {
  const ScreenshotLayer({super.key, this.folder, this.now});

  /// Dove salvare (test); null: la Scrivania.
  final String? folder;

  /// L'ora del nome del file (test).
  final DateTime Function()? now;

  @override
  State<ScreenshotLayer> createState() => ScreenshotLayerState();
}

class ScreenshotLayerState extends State<ScreenshotLayer>
    with TickerProviderStateMixin {
  StreamSubscription<String>? _commands;
  _Mode _mode = _Mode.none;
  Offset? _pointer;
  Offset? _dragStart;
  ToolbarChoice _choice = ToolbarChoice.screen;
  final FocusNode _focus = FocusNode();
  String? _folderChoice;

  /// La miniatura: il file temporaneo, la sua immagine e i tempi.
  _Shot? _shot;
  late final AnimationController _settle = AnimationController(
    vsync: this,
    duration: thumbnailSettle,
  )..addListener(() => setState(() {}));
  late final AnimationController _leave = AnimationController(
    vsync: this,
    duration: thumbnailLeave,
  )..addListener(() => setState(() {}));
  Timer? _hold;
  String _lastAnnounced = '';

  @override
  void initState() {
    super.initState();
    _commands = ShellChannel.instance.commands.listen((id) {
      if (id == 'screenshot.selection') _start(_Mode.selection);
      if (id == 'screenshot.toolbar') _start(_Mode.toolbar);
    });
    ShellChannel.instance.states.addListener(_statesChanged);
  }

  @override
  void dispose() {
    LayerArbiter.instance.remove(this);
    _commands?.cancel();
    ShellChannel.instance.states.removeListener(_statesChanged);
    _hold?.cancel();
    _settle.dispose();
    _leave.dispose();
    _focus.dispose();
    super.dispose();
  }

  // --- Modalità (mirino, finestra, barra) ---------------------------------------------------

  void _start(_Mode mode) {
    setState(() {
      _mode = mode;
      _dragStart = null;
    });
    _syncLayer();
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _cancel() {
    setState(() {
      _mode = _Mode.none;
      _dragStart = null;
    });
    _syncLayer();
  }

  /// La regione d'input: tutto lo schermo con il mirino o la barra; solo la miniatura se c'è;
  /// altrimenti niente (la superficie è trasparente ai clic).
  void _syncLayer() {
    final thumb = _shot == null ? null : _thumbRect(_lastSize);
    LayerArbiter.instance.set(
      this,
      _mode != _Mode.none
          ? const LayerWish.all()
          : thumb == null
          ? const LayerWish.none()
          : LayerWish.rects([thumb]),
    );
  }

  Size _lastSize = Size.zero;

  /// Nasconde il mirino, aspetta che Ossido ridisegni lo schermo senza, poi cattura.
  Future<void> _capture(String id, [Map<String, Object?>? params]) async {
    _cancel();
    await SchedulerBinding.instance.endOfFrame;
    await Future<void>.delayed(const Duration(milliseconds: 60));
    await ShellChannel.instance.execute(id, params);
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent) return KeyEventResult.ignored;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      _cancel();
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.space &&
        (_mode == _Mode.selection || _mode == _Mode.window)) {
      setState(() {
        _mode = _mode == _Mode.selection ? _Mode.window : _Mode.selection;
        _dragStart = null;
      });
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  void _pointerUp(Offset at) {
    switch (_mode) {
      case _Mode.selection:
        final start = _dragStart;
        if (start == null) return;
        final rect = Rect.fromPoints(start, at);
        // Un clic senza trascinare non cattura niente.
        if (rect.width < 4 || rect.height < 4) {
          setState(() => _dragStart = null);
          return;
        }
        _capture('screenshot.region', {
          'x': rect.left,
          'y': rect.top,
          'width': rect.width,
          'height': rect.height,
        });
      case _Mode.window:
        _capture('screenshot.window_at', {'x': at.dx, 'y': at.dy});
      case _Mode.toolbar || _Mode.none:
        break;
    }
  }

  void _acquire() {
    switch (_choice) {
      case ToolbarChoice.screen:
        _capture('screenshot.screen');
      case ToolbarChoice.window:
        _start(_Mode.window);
      case ToolbarChoice.selection:
        _start(_Mode.selection);
    }
  }

  // --- Miniatura ------------------------------------------------------------------------

  void _statesChanged() {
    final value = ShellChannel.instance.states.value['screenshot'] ?? '';
    if (value.isEmpty || value == _lastAnnounced) return;
    _lastAnnounced = value;
    final parts = value.split(' ');
    if (parts.length < 3) return;
    final height = double.tryParse(parts.removeLast());
    final width = double.tryParse(parts.removeLast());
    final path = parts.join(' ');
    if (width == null || height == null || width <= 0 || height <= 0) return;
    // Una miniatura ancora in vista: il suo file va subito al suo posto.
    final previous = _shot;
    if (previous != null) _save(previous);
    _hold?.cancel();
    setState(() => _shot = _Shot(path, Size(width, height)));
    _leave.value = 0;
    _settle.forward(from: 0);
    _hold = Timer(thumbnailSettle + thumbnailHold, _dismiss);
    _syncLayer();
  }

  Future<void> _dismiss() async {
    final shot = _shot;
    if (shot == null) return;
    await _leave.forward(from: 0);
    if (!mounted || _shot != shot) return;
    _save(shot);
    setState(() => _shot = null);
    _syncLayer();
  }

  /// Sposta il file temporaneo nella cartella delle istantanee; il percorso finale.
  String? _save(_Shot shot) {
    if (shot.saved != null) return shot.saved;
    final folder = _folderChoice ?? widget.folder ?? screenshotFolder();
    final name = screenshotName(
      widget.now?.call() ?? DateTime.now(),
      (n) => File('$folder/$n').existsSync(),
    );
    final target = '$folder/$name';
    try {
      final source = File(shot.path);
      try {
        source.renameSync(target);
      } on FileSystemException {
        // Su un altro disco: copia e cancella.
        source.copySync(target);
        source.deleteSync();
      }
      shot.saved = target;
      return target;
    } on FileSystemException {
      return null;
    }
  }

  void _openShot() {
    final shot = _shot;
    if (shot == null) return;
    _hold?.cancel();
    final saved = _save(shot);
    setState(() => _shot = null);
    _syncLayer();
    if (saved != null) {
      ShellChannel.instance.launch(
        "xdg-open '${saved.replaceAll("'", r"'\''")}'",
      );
    }
  }

  /// Il riquadro della miniatura: l'immagine adattata a 256 × 144 più la cornice, in basso a
  /// destra a 12 pt.
  Rect _thumbRect(Size screen) {
    final shot = _shot;
    if (shot == null || screen.isEmpty) return Rect.zero;
    final fit = math.min(
      thumbnailImage.width / shot.size.width,
      thumbnailImage.height / shot.size.height,
    );
    final w = shot.size.width * fit + 2 * thumbnailBorder;
    final h = shot.size.height * fit + 2 * thumbnailBorder;
    return Rect.fromLTWH(
      screen.width - thumbnailMargin - w,
      screen.height - thumbnailMargin - h,
      w,
      h,
    );
  }

  // --- Disegno ------------------------------------------------------------------------------

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      _lastSize = box.biggest;
      final children = <Widget>[];
      if (_shot case final shot?) {
        final rect = _thumbRect(box.biggest);
        // Si assesta (da 1,04 a 1, in dissolvenza), poi esce a destra.
        final settle = Curves.easeOut.transform(_settle.value);
        final dx =
            Curves.easeIn.transform(_leave.value) *
            (thumbnailMargin + rect.width + 30);
        children.add(
          Positioned.fromRect(
            rect: rect.shift(Offset(dx, 0)),
            child: Opacity(
              opacity: settle,
              child: Transform.scale(
                scale: 1.04 - 0.04 * settle,
                child: GestureDetector(
                  key: const ValueKey('miniatura'),
                  onTap: _openShot,
                  child: Container(
                    padding: const EdgeInsets.all(thumbnailBorder),
                    decoration: const BoxDecoration(
                      color: Color(0xFFFFFFFF),
                      boxShadow: [
                        BoxShadow(color: Color(0x40000000), blurRadius: 20),
                      ],
                    ),
                    child: Image.file(
                      File(shot.path),
                      fit: BoxFit.fill,
                      gaplessPlayback: true,
                      errorBuilder: (_, _, _) => const SizedBox.expand(),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
      }
      if (_mode != _Mode.none) {
        children.add(Positioned.fill(child: _overlay(box.biggest)));
      }
      return Stack(children: children);
    },
  );

  Widget _overlay(Size screen) => Focus(
    focusNode: _focus,
    onKeyEvent: _key,
    child: MouseRegion(
      cursor: _mode == _Mode.toolbar
          ? SystemMouseCursors.basic
          : SystemMouseCursors.none,
      onHover: (e) => setState(() => _pointer = e.localPosition),
      child: Listener(
        behavior: HitTestBehavior.opaque,
        onPointerDown: (e) {
          if (_mode == _Mode.selection) {
            setState(() {
              _dragStart = e.localPosition;
              _pointer = e.localPosition;
            });
          }
        },
        onPointerMove: (e) => setState(() => _pointer = e.localPosition),
        onPointerUp: (e) => _pointerUp(e.localPosition),
        child: Stack(
          children: [
            const Positioned.fill(child: SizedBox.expand()),
            if (_mode == _Mode.selection)
              if ((_dragStart, _pointer) case (final start?, final end?))
                Positioned.fromRect(
                  rect: Rect.fromPoints(start, end),
                  child: Container(
                    key: const ValueKey('selezione'),
                    decoration: BoxDecoration(
                      color: const Color(0x33808080),
                      border: Border.all(color: const Color(0x99FFFFFF)),
                    ),
                  ),
                ),
            if (_pointer case final p? when _mode == _Mode.selection)
              ..._crosshair(p),
            if (_pointer case final p? when _mode == _Mode.window)
              Positioned(
                left: p.dx - 14,
                top: p.dy - 11,
                child: const CustomPaint(
                  key: ValueKey('macchina fotografica'),
                  size: Size(28, 22),
                  painter: _CameraPainter(),
                ),
              ),
            if (_mode == _Mode.toolbar) _toolbar(screen),
          ],
        ),
      ),
    ),
  );

  /// Il mirino e le coordinate del punto (in punti, dall'angolo in alto a sinistra).
  List<Widget> _crosshair(Offset p) {
    final label = TextStyle(
      fontFamily: uiFontFamily,
      package: 'silica_ui',
      fontSize: 10,
      height: 1.15,
      color: const Color(0xFF000000),
      shadows: const [Shadow(color: Color(0xCCFFFFFF), blurRadius: 3)],
    );
    return [
      Positioned(
        left: p.dx - 10,
        top: p.dy - 10,
        child: const CustomPaint(
          key: ValueKey('mirino'),
          size: Size(20, 20),
          painter: _CrosshairPainter(),
        ),
      ),
      Positioned(
        left: p.dx + 12,
        top: p.dy + 8,
        child: Text(
          '${p.dx.round()}\n${p.dy.round()}',
          key: const ValueKey('coordinate'),
          style: label,
        ),
      ),
    ];
  }

  Widget _toolbar(Size screen) {
    final left = (screen.width - toolbarSize.width) / 2;
    final top = screen.height - toolbarAboveDock - toolbarSize.height;
    final c = ShotBarColors.of(context);
    Widget mode(ToolbarChoice choice, _Glyph glyph, String key) => _ToolButton(
      key: ValueKey(key),
      chosen: _choice == choice,
      chosenColor: c.chosen,
      onTap: () => setState(() => _choice = choice),
      child: CustomPaint(
        size: const Size(26, 20),
        painter: _ModePainter(glyph, c.glyph),
      ),
    );
    return Positioned(
      left: left,
      top: top,
      width: toolbarSize.width,
      height: toolbarSize.height,
      child: LiquidGlass(
        radius: 18,
        material: GlassMaterial.regular,
        interactive: false,
        child: Row(
          children: [
            const SizedBox(width: 12),
            GestureDetector(
              key: const ValueKey('chiudi la barra'),
              onTap: _cancel,
              child: Container(
                width: 18,
                height: 18,
                decoration: BoxDecoration(
                  color: c.closeFill,
                  shape: BoxShape.circle,
                ),
                alignment: Alignment.center,
                child: Text('×', style: uiText(size: 13, color: c.closeGlyph)),
              ),
            ),
            const SizedBox(width: 14),
            mode(ToolbarChoice.screen, _Glyph.screen, 'schermo intero'),
            mode(ToolbarChoice.window, _Glyph.window, 'finestra'),
            mode(ToolbarChoice.selection, _Glyph.selection, 'selezione'),
            const SizedBox(width: 10),
            Container(width: 1, height: 28, color: c.line),
            const SizedBox(width: 10),
            _ToolButton(
              key: const ValueKey('opzioni'),
              width: 120,
              onTap: _toggleFolder,
              child: Text(
                'Salva in: ${_folderLabel()}',
                style: uiText(size: 12, color: c.glyph),
              ),
            ),
            const Spacer(),
            GestureDetector(
              key: const ValueKey('acquisisci'),
              onTap: _acquire,
              child: Container(
                width: 92,
                height: 32,
                decoration: BoxDecoration(
                  color: const Color(0xFF0A84FF),
                  borderRadius: BorderRadius.circular(16),
                ),
                alignment: Alignment.center,
                child: Text(
                  'Acquisisci',
                  style: uiText(
                    size: 13,
                    color: const Color(0xFFFFFFFF),
                  ).copyWith(fontWeight: FontWeight.w600),
                ),
              ),
            ),
            const SizedBox(width: 10),
          ],
        ),
      ),
    );
  }

  /// "Opzioni": dove salvare, a giro fra Scrivania, Immagini e Documenti.
  void _toggleFolder() {
    final home = Platform.environment['HOME'] ?? '/tmp';
    final choices = [
      widget.folder ?? screenshotFolder(),
      for (final name in ['Immagini', 'Pictures', 'Documenti', 'Documents'])
        if (Directory('$home/$name').existsSync()) '$home/$name',
    ];
    final current = choices.indexOf(_folderChoice ?? choices.first);
    setState(() => _folderChoice = choices[(current + 1) % choices.length]);
  }

  String _folderLabel() {
    final path = _folderChoice ?? widget.folder ?? screenshotFolder();
    final parts = path.split('/').where((p) => p.isNotEmpty);
    return parts.isEmpty ? path : parts.last;
  }
}

class _Shot {
  _Shot(this.path, this.size);
  final String path;
  final Size size;

  /// Dove è finito il file (dopo il salvataggio).
  String? saved;
}

class _ToolButton extends StatelessWidget {
  const _ToolButton({
    super.key,
    required this.child,
    required this.onTap,
    this.chosen = false,
    this.chosenColor = const Color(0x22000000),
    this.width = 44,
  });

  final Color chosenColor;

  final Widget child;
  final VoidCallback onTap;
  final bool chosen;
  final double width;

  @override
  Widget build(BuildContext context) => GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTap: onTap,
    child: Container(
      width: width,
      height: 40,
      decoration: BoxDecoration(
        color: chosen ? chosenColor : null,
        borderRadius: BorderRadius.circular(10),
      ),
      alignment: Alignment.center,
      child: child,
    ),
  );
}

enum _Glyph { screen, window, selection }

/// Le icone delle modalità: lo schermo pieno, una finestra in uno schermo, una selezione
/// tratteggiata.
class _ModePainter extends CustomPainter {
  const _ModePainter(this.glyph, this.color);
  final _Glyph glyph;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final stroke = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    final outer = RRect.fromRectAndRadius(
      (Offset.zero & size).deflate(1),
      const Radius.circular(3),
    );
    switch (glyph) {
      case _Glyph.screen:
        canvas.drawRRect(outer, stroke);
        canvas.drawRRect(outer.deflate(3), Paint()..color = color);
      case _Glyph.window:
        canvas.drawRRect(outer, stroke);
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTWH(6, 5, size.width - 12, size.height - 10),
            const Radius.circular(2),
          ),
          Paint()..color = color,
        );
      case _Glyph.selection:
        const dash = 3.0;
        final rect = (Offset.zero & size).deflate(2);
        final path = Path()..addRect(rect);
        for (final metric in path.computeMetrics()) {
          for (var d = 0.0; d < metric.length; d += 2 * dash) {
            canvas.drawPath(metric.extractPath(d, d + dash), stroke);
          }
        }
    }
  }

  @override
  bool shouldRepaint(_ModePainter old) =>
      old.glyph != glyph || old.color != color;
}

/// La croce sottile nera con il disco grigio traslucido al centro.
class _CrosshairPainter extends CustomPainter {
  const _CrosshairPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final line = Paint()
      ..color = const Color(0xFF000000)
      ..strokeWidth = 1;
    canvas
      ..drawLine(Offset(c.dx, 0), Offset(c.dx, size.height), line)
      ..drawLine(Offset(0, c.dy), Offset(size.width, c.dy), line)
      ..drawCircle(c, 3, Paint()..color = const Color(0x66808080));
  }

  @override
  bool shouldRepaint(_CrosshairPainter old) => false;
}

/// La macchina fotografica della modalità finestra.
class _CameraPainter extends CustomPainter {
  const _CameraPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final body = Paint()..color = const Color(0xFF3A3A3C);
    final w = size.width;
    final h = size.height;
    canvas
      ..drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(0, h * 0.2, w, h * 0.8),
          const Radius.circular(4),
        ),
        body,
      )
      ..drawRect(Rect.fromLTWH(w * 0.35, 0, w * 0.3, h * 0.25), body)
      ..drawCircle(
        Offset(w / 2, h * 0.6),
        h * 0.25,
        Paint()..color = const Color(0xFFE5E5EA),
      )
      ..drawCircle(Offset(w / 2, h * 0.6), h * 0.15, body);
  }

  @override
  bool shouldRepaint(_CameraPainter old) => false;
}
