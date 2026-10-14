// Screenshots (plan/reference/screenshot-ui.md): the thumbnail after every capture, the
// ⇧⌘4 crosshair (with Space: a window) and the ⇧⌘5 bar.
//
// - Thumbnail: at the bottom right 12 pt from the edges, 264 × 152 (the 256 × 144 image with
//   a 4 pt white frame), corners of radius ~4 and a soft symmetric black shadow (≥ 26 pt);
//   it appears already in place with a fade (~400 ms, slowing down), stays ~5.1 s, then
//   leaves to the right in ~220 ms accelerating (K-05). The file goes to the Desktop when the
//   thumbnail disappears; a click saves it at once and opens it.
// - Crosshair: a thin black cross of ~20 pt with a translucent grey disc in the center and,
//   at the bottom right, the coordinates in points on two lines. Dragging picks the region;
//   Space switches to the window under the pointer (a camera) and veils it light blue
//   (`#88bae7` at 51 %) over its rectangle, corners included and shadow excluded; over the
//   desktop the whole screen is veiled (K-05). Esc cancels.
// - Bar: glass ~545 × 52, 4 pt above the Dock area: Close, full screen, window, selection,
//   Options (menu above the button: where to save, delay, thumbnail) and Capture (blue).
//   There is no recording.
//
// Ossido captures (`screenshot.screen`, `screenshot.region`, `screenshot.window_at`), writes
// a temporary PNG and announces it with the broadcast "screenshot" = "<path> <w> <h>".

import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import '../system/layer_arbiter.dart';
import '../system/shell_channel.dart';

const Size thumbnailImage = Size(256, 144);
const double thumbnailBorder = 4;
const double thumbnailMargin = 12;
const Duration thumbnailSettle = Duration(milliseconds: 400);
const Duration thumbnailHold = Duration(milliseconds: 5100);
const Duration thumbnailLeave = Duration(milliseconds: 220);
const double thumbnailRadius = 4;

/// The veil of window mode: `#88bae7` (sRGB) at opacity ~0.51 (K-05).
const Color windowVeil = Color(0x8288BAE7);

const Size toolbarSize = Size(545, 52);

/// From the Dock area (84 pt exclusive zone) to the bar: 4 pt.
const double toolbarAboveDock = 84 + 4;

/// Where screenshots go: the xdg-user-dirs Desktop, or ~/Desktop.
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
    // Try the usual names.
  }
  for (final name in ['Scrivania', 'Desktop']) {
    if (Directory('$home/$name').existsSync()) return '$home/$name';
  }
  return home;
}

/// The file name: "Screenshot 2026-10-14 at 04.12.33.png" in the user's language (a number if
/// it already exists).
String screenshotName(
  DateTime t,
  bool Function(String name) exists, {
  AppLocalizations? texts,
}) {
  String two(int v) => v.toString().padLeft(2, '0');
  final base = (texts ?? englishTexts).screenshotFileName(
    '${t.year}-${two(t.month)}-${two(t.day)}',
    '${two(t.hour)}.${two(t.minute)}.${two(t.second)}',
  );
  var name = '$base.png';
  for (var i = 2; exists(name); i++) {
    name = '$base ($i).png';
  }
  return name;
}

enum _Mode { none, selection, window, toolbar }

enum ToolbarChoice { screen, window, selection }

/// The bar colors (over the glass). Dark measured over grey `#7a7b80` (K-06): glyphs and
/// "Options" `#c7c8cd`, separators and the pill of the chosen mode `#8f9095` (white ~15 %),
/// chosen glyph `#dbdde2`, pressed pill −5 levels and glyph `#c3c4c9`, a 1 pt dark rim at the
/// top `#595a5e` (black ~28 %). Light: measured only under the desktop veil (K-05); pill and
/// separators derived by removing the veil (out = 0.514 · C + 0.486 · background:
/// background ~`#ddd8dd`, pill ~`#b4afb4`, black ~19 %), glyphs almost black; the pressed
/// pill in light is *estimated*.
@immutable
class ShotBarColors {
  const ShotBarColors({
    required this.glyph,
    required this.closeFill,
    required this.closeGlyph,
    required this.line,
    required this.chosen,
    required this.chosenGlyph,
    required this.chosenPressed,
    required this.chosenPressedGlyph,
    required this.topLine,
  });

  final Color glyph;
  final Color closeFill;
  final Color closeGlyph;
  final Color line;
  final Color chosen;
  final Color chosenGlyph;
  final Color chosenPressed;
  final Color chosenPressedGlyph;

  /// The rim at the top of the bar (null: none).
  final Color? topLine;

  static const ShotBarColors light = ShotBarColors(
    glyph: Color(0xFF1D1D1F),
    closeFill: Color(0x33000000),
    closeGlyph: Color(0xFF3C3C3E),
    line: Color(0x30000000),
    chosen: Color(0x30000000),
    chosenGlyph: Color(0xFF1D1D1F),
    chosenPressed: Color(0x3A000000),
    chosenPressedGlyph: Color(0xFF3C3C3E),
    topLine: null,
  );

  static const ShotBarColors dark = ShotBarColors(
    glyph: Color(0xFFC7C8CD),
    closeFill: Color(0x33FFFFFF),
    closeGlyph: Color(0xFFC7C8CD),
    line: Color(0x26FFFFFF),
    chosen: Color(0x26FFFFFF),
    chosenGlyph: Color(0xFFDBDDE2),
    chosenPressed: Color(0x1EFFFFFF),
    chosenPressedGlyph: Color(0xFFC3C4C9),
    topLine: Color(0x47000000),
  );

  static ShotBarColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

class ScreenshotLayer extends StatefulWidget {
  const ScreenshotLayer({super.key, this.folder, this.now, this.windowAt});

  /// The rectangle of the window under a point (frame without shadow), for the window mode
  /// veil; null or no window: the whole screen is veiled (the desktop).
  final Rect? Function(Offset point)? windowAt;

  /// Where to save (tests); null: the Desktop.
  final String? folder;

  /// The time in the file name (tests).
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

  /// The thumbnail: the temporary file, its image and the timings.
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

  /// The "Options" menu of the bar: open, the capture delay and the thumbnail.
  bool _optionsOpen = false;
  Duration _delay = Duration.zero;
  bool _showThumbnail = true;

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

  // --- Modes (crosshair, window, bar) ------------------------------------------------------

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
      _optionsOpen = false;
    });
    _syncLayer();
  }

  /// The input region: the whole screen with the crosshair or the bar; only the thumbnail if
  /// there is one; otherwise nothing (the surface lets clicks through).
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

  /// Hides the crosshair, waits for Ossido to redraw the screen without it, then captures.
  Future<void> _capture(String id, [Map<String, Object?>? params]) async {
    _cancel();
    await SchedulerBinding.instance.endOfFrame;
    await Future<void>.delayed(const Duration(milliseconds: 60) + _delay);
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
        // A click without dragging captures nothing.
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

  // --- Thumbnail ------------------------------------------------------------------------

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
    // A thumbnail still showing: its file goes to its place right away.
    final previous = _shot;
    if (previous != null) _save(previous);
    _hold?.cancel();
    if (!_showThumbnail) {
      // Without a thumbnail the file goes to its place right away.
      _save(_Shot(path, Size(width, height)));
      if (previous != null) setState(() => _shot = null);
      _syncLayer();
      return;
    }
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

  /// Moves the temporary file into the screenshots folder; the final path.
  String? _save(_Shot shot) {
    if (shot.saved != null) return shot.saved;
    final folder = _folderChoice ?? widget.folder ?? screenshotFolder();
    final name = screenshotName(
      widget.now?.call() ?? DateTime.now(),
      (n) => File('$folder/$n').existsSync(),
      texts: l10nOf(context),
    );
    final target = '$folder/$name';
    try {
      final source = File(shot.path);
      try {
        source.renameSync(target);
      } on FileSystemException {
        // On another disk: copy and delete.
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

  /// The thumbnail box: the image fitted to 256 × 144 plus the frame, at the bottom right
  /// 12 pt from the edges.
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

  // --- Drawing ------------------------------------------------------------------------------

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      _lastSize = box.biggest;
      final children = <Widget>[];
      if (_shot case final shot?) {
        final rect = _thumbRect(box.biggest);
        // It appears in place with a fade (slowing down), then leaves to the right accelerating.
        final settle = Curves.easeOut.transform(_settle.value);
        final dx =
            Curves.easeIn.transform(_leave.value) *
            (thumbnailMargin + rect.width + 30);
        children.add(
          Positioned.fromRect(
            rect: rect.shift(Offset(dx, 0)),
            child: Opacity(
              opacity: settle,
              child: GestureDetector(
                key: const ValueKey('thumbnail'),
                onTap: _openShot,
                child: Container(
                  padding: const EdgeInsets.all(thumbnailBorder),
                  decoration: BoxDecoration(
                    color: const Color(0xFFFFFFFF),
                    borderRadius: BorderRadius.circular(thumbnailRadius),
                    // −10 levels at the edge, −5 at ~12 pt, −2 at ~25: black, the same on three sides.
                    boxShadow: const [
                      BoxShadow(color: Color(0x2E000000), blurRadius: 26),
                    ],
                  ),
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(1),
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
                    key: const ValueKey('selection'),
                    decoration: BoxDecoration(
                      color: const Color(0x33808080),
                      border: Border.all(color: const Color(0x99FFFFFF)),
                    ),
                  ),
                ),
            if (_pointer case final p? when _mode == _Mode.selection)
              ..._crosshair(p),
            if (_pointer case final p? when _mode == _Mode.window)
              Positioned.fromRect(
                rect: widget.windowAt?.call(p) ?? Offset.zero & screen,
                child: const IgnorePointer(
                  child: ColoredBox(key: ValueKey('veil'), color: windowVeil),
                ),
              ),
            if (_pointer case final p? when _mode == _Mode.window)
              Positioned(
                left: p.dx - 14,
                top: p.dy - 11,
                child: const CustomPaint(
                  key: ValueKey('camera'),
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

  /// The crosshair and the coordinates of the point (in points, from the top left corner).
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
          key: ValueKey('crosshair'),
          size: Size(20, 20),
          painter: _CrosshairPainter(),
        ),
      ),
      Positioned(
        left: p.dx + 12,
        top: p.dy + 8,
        child: Text(
          '${p.dx.round()}\n${p.dy.round()}',
          key: const ValueKey('coordinates'),
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
      colors: c,
      onTap: () => setState(() => _choice = choice),
      child: (color) => CustomPaint(
        size: const Size(26, 20),
        painter: _ModePainter(glyph, color),
      ),
    );
    final bar = Positioned(
      left: left,
      top: top,
      width: toolbarSize.width,
      height: toolbarSize.height,
      child: LiquidGlass(
        radius: 18,
        material: GlassMaterial.regular,
        interactive: false,
        child: Container(
          key: const ValueKey('screenshot bar'),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(18),
            border: c.topLine == null
                ? null
                : Border(top: BorderSide(color: c.topLine!)),
          ),
          child: _toolbarRow(c, mode),
        ),
      ),
    );
    if (!_optionsOpen) return bar;
    // The "Options" menu opens above the button, with its left edge on the button and its
    // bottom 16 pt below the top of the bar (K-05).
    return Positioned.fill(
      child: Stack(
        children: [
          bar,
          Positioned.fill(
            child: MenuHost(
              key: const ValueKey('options menu'),
              entries: _optionsMenu(),
              anchor: Offset(left + _optionsLeft, top + 16),
              upwards: true,
              onDismissed: () => setState(() => _optionsOpen = false),
            ),
          ),
        ],
      ),
    );
  }

  /// From the left edge of the bar to the "Options" button.
  static const double _optionsLeft = 12 + 18 + 14 + 3 * 44 + 10 + 1 + 10;

  /// The items of the "Options" menu: where to save, the delay, the thumbnail (our texts).
  List<MenuEntry> _optionsMenu() {
    final home = Platform.environment['HOME'] ?? '/tmp';
    final l = l10nOf(context);
    final folders = <String>{
      widget.folder ?? screenshotFolder(),
      for (final name in ['Documenti', 'Documents', 'Immagini', 'Pictures'])
        if (Directory('$home/$name').existsSync()) '$home/$name',
    };
    final current = _folderChoice ?? widget.folder ?? screenshotFolder();
    String label(String path) {
      final parts = path.split('/').where((p) => p.isNotEmpty);
      return parts.isEmpty ? path : parts.last;
    }

    return [
      MenuSection(l.saveTo),
      for (final folder in folders)
        MenuAction(
          label(folder),
          checked: folder == current,
          onSelected: () => setState(() => _folderChoice = folder),
        ),
      const MenuDivider(),
      MenuSection(l.delay),
      for (final (seconds, text) in [
        (0, l.delayNone),
        (5, l.delaySeconds(5)),
        (10, l.delaySeconds(10)),
      ])
        MenuAction(
          text,
          checked: _delay.inSeconds == seconds,
          onSelected: () => setState(() => _delay = Duration(seconds: seconds)),
        ),
      const MenuDivider(),
      MenuSection(l.options),
      MenuAction(
        l.thumbnailAfterCapture,
        checked: _showThumbnail,
        onSelected: () => setState(() => _showThumbnail = !_showThumbnail),
      ),
    ];
  }

  Widget _toolbarRow(
    ShotBarColors c,
    Widget Function(ToolbarChoice, _Glyph, String) mode,
  ) => Row(
    children: [
      const SizedBox(width: 12),
      GestureDetector(
        key: const ValueKey('close bar'),
        onTap: _cancel,
        child: Container(
          width: 18,
          height: 18,
          decoration: BoxDecoration(color: c.closeFill, shape: BoxShape.circle),
          alignment: Alignment.center,
          child: Text('×', style: uiText(size: 13, color: c.closeGlyph)),
        ),
      ),
      const SizedBox(width: 14),
      mode(ToolbarChoice.screen, _Glyph.screen, 'mode screen'),
      mode(ToolbarChoice.window, _Glyph.window, 'mode window'),
      mode(ToolbarChoice.selection, _Glyph.selection, 'mode selection'),
      const SizedBox(width: 10),
      Container(width: 1, height: 28, color: c.line),
      const SizedBox(width: 10),
      _ToolButton(
        key: const ValueKey('options'),
        width: 76,
        colors: c,
        onTap: () => setState(() => _optionsOpen = !_optionsOpen),
        child: (color) => Text(
          l10nOf(context).optionsButton,
          style: uiText(size: 12, color: color),
        ),
      ),
      const Spacer(),
      GestureDetector(
        key: const ValueKey('capture'),
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
            l10nOf(context).capture,
            style: uiText(
              size: 13,
              color: const Color(0xFFFFFFFF),
            ).copyWith(fontWeight: FontWeight.w600),
          ),
        ),
      ),
      const SizedBox(width: 10),
    ],
  );
}

class _Shot {
  _Shot(this.path, this.size);
  final String path;
  final Size size;

  /// Where the file ended up (after saving).
  String? saved;
}

/// A bar button: the pill of the chosen mode (darker when pressed, K-06) and the glyph in
/// the color of its state. The pointer over a mode that is not chosen changes nothing.
class _ToolButton extends StatefulWidget {
  const _ToolButton({
    super.key,
    required this.child,
    required this.onTap,
    required this.colors,
    this.chosen = false,
    this.width = 44,
  });

  final Widget Function(Color color) child;
  final VoidCallback onTap;
  final ShotBarColors colors;
  final bool chosen;
  final double width;

  @override
  State<_ToolButton> createState() => _ToolButtonState();
}

class _ToolButtonState extends State<_ToolButton> {
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final c = widget.colors;
    final pressedChosen = widget.chosen && _pressed;
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTapDown: (_) => setState(() => _pressed = true),
      onTapCancel: () => setState(() => _pressed = false),
      onTapUp: (_) => setState(() => _pressed = false),
      onTap: widget.onTap,
      child: Container(
        width: widget.width,
        height: 40,
        decoration: BoxDecoration(
          color: !widget.chosen
              ? null
              : pressedChosen
              ? c.chosenPressed
              : c.chosen,
          borderRadius: BorderRadius.circular(10),
        ),
        alignment: Alignment.center,
        child: widget.child(
          !widget.chosen
              ? c.glyph
              : pressedChosen
              ? c.chosenPressedGlyph
              : c.chosenGlyph,
        ),
      ),
    );
  }
}

enum _Glyph { screen, window, selection }

/// The mode icons: the full screen, a window in a screen, a dashed
/// selection.
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

/// The thin black cross with the translucent grey disc in the center.
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

/// The camera of window mode.
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
