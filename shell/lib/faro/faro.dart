// Faro, the app search: from plan/reference/spotlight.md (light appearance). A thick glass
// panel, centered, with its top at 30 % of the screen; a 56 pt header with the lens and the
// field; empty: 384 pt wide with four 54 pt round buttons (64 pitch); with results: 640 pt
// wide, 57 pt rows, at most 467 pt tall.
//
// The component always runs on a full-screen overlay layer: when closed it draws nothing
// and takes neither pointer nor keyboard (silica/layer).

import 'dart:async';
import 'dart:io';

import 'package:flutter/physics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:intl/intl.dart' show DateFormat;
import 'package:silica_ui/silica_ui.dart';

import '../dock/dock.dart' show shellQuote;
import '../dock/stack.dart' show fileIcon;
import '../l10n/l10n.dart';
import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import '../system/silica_icons.dart';
import 'answers.dart';
import 'folders.dart';
import 'launcher.dart';
import 'search.dart';

const double wideWidth = 640;
const double narrowWidth = 384;
const double topFraction = 0.30;
const double headerHeight = 56;
const double rowHeight = 57;

/// App icon in the row: ~38–39 pt visible (spotlight.md).
const double rowIconSide = 38;
const double maxHeight = 467;
const double radius = 28.5;

/// Round buttons of the empty state: diameter 54, pitch 64, 10 pt from the panel.
const double circle = 54;
const double circlePitch = 64;

const Color placeholderColor = Color(0xFF494A4F);
const Color textColor = Color(0xFF16171C);
const Color selectedTextColor = Color(0xFF000003);
const Color subtitleColor = Color(0xFF88898E);

/// Selected row and separator: `#afb0b5` over the `#c8c9ce` fill (−25 levels): black at
/// ~12 %, so it follows the material over different backgrounds.
const Color selection = Color(0x20000000);

/// Opacity while opening (0, 13, 27, 53, 67 ms from the first frame).
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

/// Launcher opening: it starts ~8 % larger and shrinks (residual overshoot measured at 0,
/// 13, 40, 53, 67, 93, 107, 120 and 147 ms), with the same fade as Faro.
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

/// Esc in the launcher: the panel shrinks towards Faro (measured height, S curve, 215 ms);
/// width, position and radius follow the same progress.
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

/// Progress 0–1 of the return to Faro after `ms`.
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
    this.folders,
  });

  final Map<String, DesktopEntry> entries;
  final bool startOpen;

  /// Opens the launcher right away (tests and recordings).
  final bool startLauncher;

  /// Where recent apps are remembered (default: ~/.local/state/silica).
  final RecentApps? recents;

  /// The folders Faro finds (default: the home folder, indexed when Faro opens).
  final FolderIndex? folders;

  @override
  State<Faro> createState() => _FaroState();
}

/// A result row: an app, a folder or one of the fallback searches at the bottom.
sealed class FaroItem {
  const FaroItem();
}

class AppItem extends FaroItem {
  const AppItem(this.entry);
  final DesktopEntry entry;
}

class FolderItem extends FaroItem {
  const FolderItem(this.folder);
  final FolderHit folder;
}

enum FallbackKind { files, web }

/// "Search in <file manager>" and "Search the Web": always last, also with no results
/// (SH-SP-18: there is no "no results" message).
class FallbackItem extends FaroItem {
  const FallbackItem(this.kind);
  final FallbackKind kind;
}

/// The address Faro's web search opens (provisional: the search engine is a setting to
/// come).
const String webSearchUrl = 'https://duckduckgo.com/?q=';

/// The rows for `query`: apps, then folders, then the fallback searches.
List<FaroItem> faroItems(
  Iterable<DesktopEntry> entries,
  FolderIndex folders,
  String query,
) {
  if (query.trim().isEmpty) return const [];
  return [
    for (final e in searchApps(entries, query)) AppItem(e),
    for (final f in folders.search(query)) FolderItem(f),
    const FallbackItem(FallbackKind.files),
    const FallbackItem(FallbackKind.web),
  ];
}

/// Answer card (SH-SP-13/14): 18 pt from the panel sides, 1 pt under the field, 62 tall
/// with a 2 pt border, radius 18; the separator 9 pt under it.
const double cardInset = 18;
const double cardHeight = 62;
const double cardRadius = 18;
const double cardBorder = 2;
const double cardAfter = 9;

/// Copy button in the card: a 26 pt disc.
const double copySide = 26;

/// The result list has 10 pt above the first row and below the last (INV-A3).
const double listPadding = 10;

/// The icon of the app handling the chosen row, at the right of the field.
const double fieldIconSide = 26;
const double fieldIconRight = 24;

/// "Tab" key capsule on the chosen folder row: 18 tall, radius 5.5, 10 pt from the row's
/// right edge; the hint before it at ~11 pt.
const double keycapHeight = 18;
const double keycapRadius = 5.5;

class _FaroState extends State<Faro> with TickerProviderStateMixin {
  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  late final AnimationController _open = AnimationController.unbounded(
    vsync: this,
  )..addListener(() => setState(() {}));
  StreamSubscription<String>? _commands;
  bool _visible = false;
  bool _launcher = false;
  bool _hovered = false;

  /// The round buttons coming out on hover: the panel narrows from 640 to 384 pt with a
  /// damped spring (~390 ms, small undershoot) and the buttons slide out from behind it from
  /// +80 ms, overshooting ~7 % (spotlight › animation, measured on the first-run layout).
  late final AnimationController _reveal = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 670),
  )..addListener(() => setState(() {}));

  void _setHovered(bool hovered) {
    if (hovered == _hovered) return;
    _hovered = hovered;
    hovered ? _reveal.forward() : _reveal.reverse();
  }

  /// The chosen row; with an answer card, 0 is the card and the rows start at 1.
  int _selected = 0;
  DateTime? _openedAt;

  /// The return from the launcher (value = ms from the start).
  late final AnimationController _back = AnimationController(
    vsync: this,
    upperBound: backDuration,
    value: backDuration,
    duration: const Duration(microseconds: 213300),
  )..addListener(() => setState(() {}));
  late final RecentApps _recents = widget.recents ?? RecentApps();
  late final FolderIndex _folders = widget.folders ?? FolderIndex.home();
  List<String> _recentIds = const [];
  String _lastText = '';

  @override
  void initState() {
    super.initState();
    _query.addListener(() {
      // Moving the cursor or selecting keeps the chosen row.
      if (_query.text == _lastText) {
        setState(() {});
        return;
      }
      _lastText = _query.text;
      setState(() => _selected = 0);
    });
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
    _reveal.dispose();
    _query.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _toggle() => _visible && !_launcher ? _hide() : _show();

  /// Esc from the launcher: no fade, the panel turns into Faro.
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
    // The controller rebuilds every frame while the opening lasts.
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
      _hovered = false;
      _reveal.value = 0;
      // It reopens with the previous query, selected (SH-SP-04).
      _query.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _query.text.length,
      );
    });
    _folders.refresh(
      onDone: () {
        if (mounted && _visible) setState(() {});
      },
    );
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
    // Height: from 0.966 to 1 with a small overshoot (+2 pt over 447).
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

  void _activate(FaroItem item) {
    switch (item) {
      case AppItem(:final entry):
        _launch(entry);
        return;
      case FolderItem(:final folder):
        ShellChannel.instance.launch('xdg-open ${shellQuote(folder.path)}');
      case FallbackItem(kind: FallbackKind.files):
        // Provisional: the file manager opens on the home folder (it has no search
        // argument yet).
        final home = Platform.environment['HOME'] ?? '/';
        ShellChannel.instance.launch('xdg-open ${shellQuote(home)}');
      case FallbackItem(kind: FallbackKind.web):
        final url = webSearchUrl + Uri.encodeQueryComponent(_query.text.trim());
        ShellChannel.instance.launch('xdg-open ${shellQuote(url)}');
    }
    _hide();
  }

  void _copy(FaroAnswer answer) {
    Clipboard.setData(ClipboardData(text: answer.result));
    _hide();
  }

  KeyEventResult _key(
    FocusNode node,
    KeyEvent event,
    FaroAnswer? answer,
    List<FaroItem> items,
  ) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final count = items.length + (answer == null ? 0 : 1);
    switch (event.logicalKey) {
      case LogicalKeyboardKey.escape:
        _hide();
      case LogicalKeyboardKey.arrowDown:
        if (count > 0) {
          setState(() => _selected = (_selected + 1).clamp(0, count - 1));
        }
      case LogicalKeyboardKey.arrowUp:
        if (count > 0) {
          setState(() => _selected = (_selected - 1).clamp(0, count - 1));
        }
      case LogicalKeyboardKey.enter || LogicalKeyboardKey.numpadEnter:
        if (count == 0) return KeyEventResult.handled;
        final index = _selected.clamp(0, count - 1);
        if (answer != null && index == 0) {
          _copy(answer);
        } else {
          _activate(items[index - (answer == null ? 0 : 1)]);
        }
      default:
        return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  FaroColors get _c => FaroColors.of(context);

  /// The decimal separator of the shell's language.
  String get _decimal =>
      l10nOf(context).localeName.startsWith('it') ? ',' : '.';

  /// The installed app whose id contains `part` (the calculator, the file manager).
  DesktopEntry? _appWithId(String part) {
    for (final e in widget.entries.values) {
      if (e.id.toLowerCase().contains(part)) return e;
    }
    return null;
  }

  @override
  Widget build(BuildContext context) {
    if (!_visible) return const SizedBox.expand();
    if (_launcher) return _launcherView();
    final answer = answerFor(_query.text, decimal: _decimal);
    final items = faroItems(widget.entries.values, _folders, _query.text);
    final searching = _query.text.trim().isNotEmpty;
    final count = items.length + (answer == null ? 0 : 1);
    final selected = count == 0 ? 0 : _selected.clamp(0, count - 1);
    final cardBlock = answer == null ? 0.0 : 1 + cardHeight + cardAfter;
    final listHeight = items.isEmpty
        ? 0.0
        : (items.length * rowHeight + 2 * listPadding).clamp(
            0.0,
            maxHeight - headerHeight - cardBlock - 1,
          );
    final height = headerHeight + (searching ? cardBlock + 1 + listHeight : 0);
    final elapsed = _openedAt == null
        ? 1000.0
        : DateTime.now().difference(_openedAt!).inMicroseconds / 1000;
    final opacity = opacityAt(elapsed);
    final scaleY = 0.966 + 0.034 * _open.value;
    if (elapsed < 70) WidgetsBinding.instance.scheduleFrame();
    // Steady state: one 640 pt capsule, empty or with results (SH-SP-04). The round buttons
    // come out only while the pointer is on the empty panel (provisional, not measured).
    final t = searching ? 0.0 : _reveal.value * 670;
    // Panel width: spring settling in ~390 ms; buttons: from +80 ms, settling by ~670 ms.
    final narrow = Curves.easeOutBack.transform((t / 390).clamp(0.0, 1.0));
    final slide = Curves.easeOutBack.transform(
      ((t - 80) / 590).clamp(0.0, 1.0),
    );
    final buttons = t > 0;
    final width = wideWidth + (narrowWidth - wideWidth) * narrow;

    return LayoutBuilder(
      builder: (context, constraints) {
        final screen = constraints.biggest;
        var left = (screen.width - wideWidth) / 2;
        final top = screen.height * topFraction;
        // From the launcher: size and position from the big panel to the Faro one.
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
            // A click outside closes Faro.
            Positioned.fill(
              child: GestureDetector(
                onTap: _hide,
                behavior: HitTestBehavior.opaque,
              ),
            ),
            if (buttons && back >= 1)
              for (var i = 3; i >= 0; i--)
                Positioned(
                  // From behind the panel (all overlapped at its right end) out to the
                  // 64 pt pitch.
                  left:
                      left +
                      narrowWidth -
                      circle +
                      (10 + circle + i * circlePitch) * slide,
                  top: top + (headerHeight - circle) / 2,
                  width: circle,
                  height: circle,
                  child: Opacity(opacity: opacity, child: _circleButton(i)),
                ),
            Positioned(
              left: left,
              top: top,
              width: panelWidth,
              height: panelHeight,
              child: MouseRegion(
                onEnter: (_) => _setHovered(true),
                child: Transform(
                  alignment: Alignment.center,
                  transform: Matrix4.diagonal3Values(1, scaleY, 1),
                  child: Opacity(
                    opacity: opacity,
                    child: _panel(
                      answer,
                      items,
                      selected,
                      listHeight,
                      panelRadius: panelRadius,
                    ),
                  ),
                ),
              ),
            ),
            // The hover zone is the whole 640 pt frame, buttons included: the panel narrowing
            // under the pointer must not count as leaving it.
            if (!searching && back >= 1)
              Positioned(
                left: left,
                top: top,
                width: wideWidth,
                height: headerHeight,
                child: MouseRegion(
                  opaque: false,
                  hitTestBehavior: HitTestBehavior.translucent,
                  onEnter: (_) => _setHovered(true),
                  onExit: (_) => _setHovered(false),
                ),
              ),
          ],
        );
      },
    );
  }

  Widget _panel(
    FaroAnswer? answer,
    List<FaroItem> items,
    int selected,
    double listHeight, {
    double panelRadius = radius,
  }) {
    final offset = answer == null ? 0 : 1;
    return OuterShadow(
      // Shadow moved down (~20 pt): it extends ~65 pt below, ~25 above, ~45 at the sides.
      radius: panelRadius,
      blurRadius: 42,
      offset: const Offset(0, 20),
      color: _c.shadow(const Color(0x40000000)),
      child: LiquidGlass(
        radius: panelRadius,
        material: GlassMaterial.thick,
        interactive: false,
        child: ClipRect(
          child: OverflowBox(
            alignment: Alignment.topLeft,
            maxHeight: double.infinity,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                SizedBox(
                  height: headerHeight,
                  child: _field(answer, items, selected),
                ),
                if (answer != null) ...[
                  const SizedBox(height: 1),
                  _card(answer, selected == 0),
                  const SizedBox(height: cardAfter),
                ],
                if (items.isNotEmpty || answer != null)
                  Container(
                    height: 1,
                    margin: const EdgeInsets.symmetric(horizontal: 20),
                    color: _c.selection,
                  ),
                if (items.isNotEmpty)
                  SizedBox(
                    height: listHeight,
                    child: ListView.builder(
                      // Rows are 619 pt cells 2 pt from the left edge; the rest
                      // is the scroll bar's (spotlight.md › result rows).
                      padding: const EdgeInsets.fromLTRB(
                        2,
                        listPadding,
                        19,
                        listPadding,
                      ),
                      itemExtent: rowHeight,
                      itemCount: items.length,
                      itemBuilder: (context, i) =>
                          _row(items[i], i + offset == selected),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  /// The text completed inline in the field for the chosen row, and the icon at the right
  /// of the field (the app that handles it).
  (String?, IconSource?) _completion(
    FaroAnswer? answer,
    List<FaroItem> items,
    int selected,
  ) {
    final l = l10nOf(context);
    final query = _query.text.trimLeft();
    if (answer != null && selected == 0) {
      final calculator = _appWithId('calculator');
      return (
        '= ${answer.result}',
        calculator == null ? null : _icon(calculator),
      );
    }
    final index = selected - (answer == null ? 0 : 1);
    if (index < 0 || index >= items.length) return (null, null);
    String? rest(String name) =>
        name.toLowerCase().startsWith(query.toLowerCase())
        ? name.substring(query.length)
        : null;
    final files = _appWithId('mustache');
    switch (items[index]) {
      case AppItem(:final entry):
        final r = rest(entry.name);
        return (r == null ? null : l.completionOpen(r), _icon(entry));
      case FolderItem(:final folder):
        final r = rest(folder.name);
        return (
          r == null ? null : l.completionOpen(r),
          files == null ? null : _icon(files),
        );
      case FallbackItem(:final kind):
        return (
          l.faroCompletionAction(_fallbackLabel(kind)),
          kind == FallbackKind.files && files != null ? _icon(files) : null,
        );
    }
  }

  String _fallbackLabel(FallbackKind kind) {
    final l = l10nOf(context);
    return switch (kind) {
      FallbackKind.files => l.faroSearchIn(
        _appWithId('mustache')?.name ?? l.faroFiles,
      ),
      FallbackKind.web => l.faroSearchWeb,
    };
  }

  IconSource? _icon(DesktopEntry entry) {
    final layered = silicaIconFor(entry);
    if (layered != null) return LayeredSource(layered);
    final flat = findIcon(entry.icon);
    return flat == null ? null : FlatSource(flat);
  }

  Widget _field(FaroAnswer? answer, List<FaroItem> items, int selected) {
    final (completion, preview) = _completion(answer, items, selected);
    // While the reopened query is selected the completion waits for typing.
    final hasSelection =
        _query.selection.isValid && !_query.selection.isCollapsed;
    final shown = hasSelection ? null : completion;
    final typed = shown == null
        ? 0.0
        : (TextPainter(
            text: TextSpan(
              text: _query.text,
              style: uiText(size: 26, color: _c.text),
            ),
            textDirection: TextDirection.ltr,
          )..layout()).width;
    final icon = _query.text.isEmpty ? null : preview;
    return Focus(
      onKeyEvent: (node, event) => _key(node, event, answer, items),
      child: Stack(
        children: [
          Positioned.fill(
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
                            l10nOf(context).faroPlaceholder,
                            style: uiText(size: 26, color: _c.placeholder),
                          ),
                        ),
                      if (shown != null)
                        Positioned(
                          left: typed + 1,
                          top: 0,
                          bottom: 0,
                          child: IgnorePointer(
                            child: Center(
                              child: Container(
                                height: completionHeight,
                                padding: const EdgeInsets.only(
                                  left: 1,
                                  right: 6,
                                ),
                                alignment: Alignment.center,
                                decoration: BoxDecoration(
                                  color: _c.completionFill,
                                  borderRadius: const BorderRadius.all(
                                    Radius.circular(completionRadius),
                                  ),
                                ),
                                child: Text(
                                  shown,
                                  maxLines: 1,
                                  style: uiText(
                                    size: 26,
                                    color: _c.completionText,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                      EditableText(
                        controller: _query,
                        focusNode: _focus,
                        style: uiText(size: 26, color: _c.text),
                        cursorColor: _c.text,
                        cursorWidth: 2,
                        cursorHeight: 31,
                        selectionColor: _c.completionFill,
                        backgroundCursorColor: const Color(0x00000000),
                      ),
                    ],
                  ),
                ),
                SizedBox(
                  width: icon == null ? 23 : fieldIconSide + 2 * fieldIconRight,
                ),
              ],
            ),
          ),
          if (icon != null)
            Positioned(
              right: fieldIconRight,
              top: (headerHeight - fieldIconSide) / 2,
              width: fieldIconSide,
              height: fieldIconSide,
              child: SilicaIcon(icon, side: fieldIconSide),
            ),
        ],
      ),
    );
  }

  /// The answer card: the expression small on top, the result below, the copy button on
  /// the right (SH-SP-13).
  Widget _card(FaroAnswer answer, bool selected) => Container(
    height: cardHeight,
    margin: const EdgeInsets.symmetric(horizontal: cardInset),
    padding: const EdgeInsets.only(left: 16, right: 14),
    decoration: BoxDecoration(
      color: _c.selection,
      border: Border.all(
        color: selected ? _c.cardBorder : const Color(0x00000000),
        width: cardBorder,
      ),
      borderRadius: const BorderRadius.all(Radius.circular(cardRadius)),
    ),
    child: Row(
      children: [
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                answer.expression,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: uiText(size: 11, color: _c.text),
              ),
              const SizedBox(height: 2),
              Text(
                answer.result,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: uiText(size: 16.5, color: _c.selectedText),
              ),
            ],
          ),
        ),
        Semantics(
          button: true,
          label: l10nOf(context).faroCopy,
          child: GestureDetector(
            onTap: () => _copy(answer),
            child: Container(
              width: copySide,
              height: copySide,
              decoration: BoxDecoration(
                color: _c.copyFill,
                shape: BoxShape.circle,
              ),
              child: CustomPaint(painter: _CopyGlyphPainter(_c.text)),
            ),
          ),
        ),
      ],
    ),
  );

  Widget _row(FaroItem item, bool selected) {
    final l = l10nOf(context);
    final (IconSource? icon, String title, Widget? subtitle) = switch (item) {
      AppItem(:final entry) => (
        _icon(entry),
        entry.name,
        Text(l.faroApplication, style: uiText(size: 15, color: _c.subtitle)),
      ),
      FolderItem(:final folder) => (
        fileIcon(Directory(folder.path)),
        folder.name,
        _folderSubtitle(folder),
      ),
      FallbackItem(:final kind) => (
        switch (kind == FallbackKind.files ? _appWithId('mustache') : null) {
          final e? => _icon(e),
          null => null,
        },
        _fallbackLabel(kind),
        null,
      ),
    };
    return GestureDetector(
      onTap: () => _activate(item),
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
                child: switch (icon) {
                  final i? => Center(child: SilicaIcon(i, side: rowIconSide)),
                  null =>
                    item is FallbackItem
                        ? CustomPaint(
                            painter: _FallbackGlyphPainter(
                              item.kind,
                              _c.placeholder,
                            ),
                          )
                        : null,
                },
              ),
              const SizedBox(width: 15),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: uiText(
                        size: 16.5,
                        color: selected ? _c.selectedText : _c.text,
                      ),
                    ),
                    ?subtitle,
                  ],
                ),
              ),
              if (selected && item is FolderItem) ...[
                const SizedBox(width: 8),
                Text(
                  l.faroSearchIn(item.folder.name),
                  maxLines: 1,
                  style: uiText(size: 11, color: _c.hint),
                ),
                const SizedBox(width: 7),
                Container(
                  height: keycapHeight,
                  padding: const EdgeInsets.symmetric(horizontal: 6),
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: _c.keycapFill,
                    borderRadius: const BorderRadius.all(
                      Radius.circular(keycapRadius),
                    ),
                  ),
                  child: Text(
                    l.faroTabKey,
                    style: uiText(size: 11, color: _c.keycapText),
                  ),
                ),
                const SizedBox(width: 10),
              ],
            ],
          ),
        ),
      ),
    );
  }

  /// "today, 14:56 · ▢ parent" (SH-SP-08).
  Widget _folderSubtitle(FolderHit folder) {
    final l = l10nOf(context);
    final now = DateTime.now();
    final day = DateTime(now.year, now.month, now.day);
    final m = folder.modified;
    final time = DateFormat.Hm(l.localeName).format(m);
    final date = !m.isBefore(day)
        ? l.faroToday(time)
        : !m.isBefore(day.subtract(const Duration(days: 1)))
        ? l.faroYesterday(time)
        : DateFormat.yMMMd(l.localeName).format(m);
    final style = uiText(size: 15, color: _c.subtitle);
    return Row(
      children: [
        Text('$date · ', style: style),
        CustomPaint(
          size: const Size(13, 11),
          painter: _FolderGlyphPainter(_c.subtitle),
        ),
        const SizedBox(width: 4),
        Flexible(
          child: Text(
            folder.parent,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: style,
          ),
        ),
      ],
    );
  }

  Widget _circleButton(int index) => GestureDetector(
    // The first button opens the launcher.
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
                  // Shadow moved down by ~17 pt, extending ~35 pt.
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

/// Generic glyphs of the four buttons: apps, files, actions, clipboard.
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

/// Two overlapping rounded sheets: the copy glyph of the answer card.
class _CopyGlyphPainter extends CustomPainter {
  const _CopyGlyphPainter(this.color);
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final back = RRect.fromRectAndRadius(
      Rect.fromCenter(
        center: c + const Offset(-1.5, -1.5),
        width: 8,
        height: 10,
      ),
      const Radius.circular(1.5),
    );
    final front = RRect.fromRectAndRadius(
      Rect.fromCenter(center: c + const Offset(1.5, 1.5), width: 8, height: 10),
      const Radius.circular(1.5),
    );
    canvas
      ..drawRRect(
        back,
        Paint()
          ..color = color
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.3,
      )
      ..drawRRect(front, Paint()..color = color);
  }

  @override
  bool shouldRepaint(_CopyGlyphPainter old) => old.color != color;
}

/// The small folder before the parent's name in a folder row's subtitle.
class _FolderGlyphPainter extends CustomPainter {
  const _FolderGlyphPainter(this.color);
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final w = size.width, h = size.height;
    final path = Path()
      ..moveTo(0.5, 2)
      ..lineTo(0.5, h - 1)
      ..quadraticBezierTo(0.5, h - 0.5, 1.5, h - 0.5)
      ..lineTo(w - 1.5, h - 0.5)
      ..quadraticBezierTo(w - 0.5, h - 0.5, w - 0.5, h - 1.5)
      ..lineTo(w - 0.5, 3.5)
      ..quadraticBezierTo(w - 0.5, 2.5, w - 1.5, 2.5)
      ..lineTo(w * 0.45, 2.5)
      ..lineTo(w * 0.35, 0.5)
      ..lineTo(1.5, 0.5)
      ..quadraticBezierTo(0.5, 0.5, 0.5, 2)
      ..moveTo(0.5, 4.5)
      ..lineTo(w - 0.5, 4.5);
    canvas.drawPath(
      path,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1,
    );
  }

  @override
  bool shouldRepaint(_FolderGlyphPainter old) => old.color != color;
}

/// The glyph of a fallback row without an app icon: a globe for the web search, a
/// magnifier on a sheet for the file search.
class _FallbackGlyphPainter extends CustomPainter {
  const _FallbackGlyphPainter(this.kind, this.color);
  final FallbackKind kind;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.8;
    switch (kind) {
      case FallbackKind.web:
        canvas
          ..drawCircle(c, 14, paint)
          ..drawOval(Rect.fromCenter(center: c, width: 13, height: 28), paint)
          ..drawLine(c + const Offset(-14, 0), c + const Offset(14, 0), paint)
          ..drawLine(c + const Offset(-12, -7), c + const Offset(12, -7), paint)
          ..drawLine(c + const Offset(-12, 7), c + const Offset(12, 7), paint);
      case FallbackKind.files:
        canvas
          ..drawCircle(c + const Offset(-3, -3), 9, paint)
          ..drawLine(c + const Offset(4, 4), c + const Offset(12, 12), paint);
    }
  }

  @override
  bool shouldRepaint(_FallbackGlyphPainter old) =>
      old.kind != kind || old.color != color;
}

/// The shadow of a glass panel drawn only outside the panel: under the glass, which Ossido
/// composes behind the surface, it must darken nothing (a BoxShadow would).
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
