// Faro, the app search: from plan/reference/spotlight.md (light appearance). A thick glass
// panel, centered, with its top at 30 % of the screen; a 56 pt header with the lens and the
// field; empty: 384 pt wide with four 54 pt round buttons (64 pitch); with results: 640 pt
// wide, 57 pt rows, at most 467 pt tall.
//
// The component always runs on a full-screen overlay layer: when closed it draws nothing
// and takes neither pointer nor keyboard (silica/layer).

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/physics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:intl/date_symbol_data_local.dart';
import 'package:intl/intl.dart' show DateFormat;
import 'package:silica_ui/silica_ui.dart';

import '../dock/dock.dart' show shellQuote;
import '../dock/stack.dart' show fileIcon;
import '../l10n/l10n.dart';
import '../system/dbus.dart';
import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import '../system/silica_icons.dart';
import 'answers.dart';
import 'faro_motion.dart';
import 'folders.dart';
import 'launcher.dart';
import 'rates.dart';
import 'search.dart';

export 'faro_motion.dart';

part 'faro_scopes.dart';

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

class Faro extends StatefulWidget {
  const Faro({
    super.key,
    required this.entries,
    this.startOpen = false,
    this.startLauncher = false,
    this.recents,
    this.folders,
    this.actions,
    this.clipboard,
  });

  final Map<String, DesktopEntry> entries;
  final bool startOpen;

  /// Opens the launcher right away (tests and recordings).
  final bool startLauncher;

  /// Where recent apps are remembered (default: ~/.local/state/silica).
  final RecentApps? recents;

  /// The folders Faro finds (default: the home folder, indexed when Faro opens).
  final FolderIndex? folders;

  /// The system actions (default: from silica-daemon).
  final SystemActions? actions;

  /// The clipboard history (default: from silica-daemon).
  final ClipboardHistory? clipboard;

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
    for (final f in folders.search(query, files: true))
      f.isFolder ? FolderItem(f) : FileItem(f),
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

/// The filter row over the results: 47 pt (10 above the pills, 26, 11 below).
const double filterHeight = 47;

class _Pill {
  const _Pill(this.label, {required this.onTap, this.selected = false});
  final String label;
  final VoidCallback onTap;
  final bool selected;
}

/// Distance a press on a file row travels before it becomes a drag (PROVISIONAL: the
/// reference's list threshold, 5 pt in the sidebar).
const double dragThreshold = 5;

/// "Tab" key capsule on the chosen folder row: 18 tall, radius 5.5, 10 pt from the row's
/// right edge; the hint before it at ~11 pt.
const double keycapHeight = 18;
const double keycapRadius = 5.5;

class _FaroState extends State<Faro> with TickerProviderStateMixin {
  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  StreamSubscription<String>? _commands;
  bool _visible = false;
  bool _launcher = false;

  /// The pointer is on the 640 pt frame of the empty panel: the round buttons come out.
  bool _hovered = false;

  /// Closing: the fade plays backwards, then the panel is gone.
  bool _closing = false;

  // All the motion of the panel (faro_motion.dart). Each controller is one quantity,
  // driven by the measured table or spring, from wherever it is when something changes.

  /// The opening fade, in ms from the first frame (0 → [fadeMs]; backwards when closing).
  late final AnimationController _fade =
      AnimationController(
          vsync: this,
          upperBound: fadeMs,
          duration: Duration(microseconds: (fadeMs * 1000).round()),
        )
        ..addListener(_tick)
        ..addStatusListener(_fadeStatus);

  /// Vertical scale of the panel, centred (0.966 → 1 when opening, back when closing).
  late final AnimationController _scaleY = AnimationController.unbounded(
    vsync: this,
    value: 1,
  )..addListener(_tick);

  /// Width of the panel, in pt.
  late final AnimationController _width = AnimationController.unbounded(
    vsync: this,
    value: wideWidth,
  )..addListener(_tick);

  /// Height of the panel, in pt (the header alone, or with the results).
  late final AnimationController _height = AnimationController.unbounded(
    vsync: this,
    value: headerHeight,
  )..addListener(_tick);
  double? _heightTarget;

  /// The round buttons: 0 = all behind the panel's right edge, 1 = spread at the 64 pt pitch.
  late final AnimationController _spread = AnimationController.unbounded(
    vsync: this,
  )..addListener(_tick);
  Timer? _spreadDelay;

  /// The Launcher's opening, in ms from its first frame.
  late final AnimationController _launcherClock = AnimationController(
    vsync: this,
    upperBound: launcherOpenMs,
    value: launcherOpenMs,
    duration: Duration(microseconds: (launcherOpenMs * 1000).round()),
  )..addListener(_tick);

  void _tick() {
    if (mounted) setState(() {});
  }

  void _fadeStatus(AnimationStatus status) {
    if (status == AnimationStatus.dismissed && _closing) {
      setState(() {
        _visible = false;
        _closing = false;
      });
    }
  }

  /// Moves `controller` to `target` with `spring`, keeping its current velocity.
  static void _springTo(
    AnimationController controller,
    double target,
    SpringDescription spring,
  ) {
    if (controller.value == target && !controller.isAnimating) return;
    controller
        .animateWith(
          SpringSimulation(
            spring,
            controller.value,
            target,
            controller.velocity,
            tolerance: const Tolerance(distance: 0.01, velocity: 0.05),
          ),
        )
        // At rest it is exactly on the target (an interrupted spring never gets here).
        .then((_) => controller.value = target);
  }

  bool get _searching => _query.text.trim().isNotEmpty;

  /// The width and the buttons follow the state: narrow with the buttons out only while the
  /// pointer is on the empty panel (PROVISIONAL: the steady-state hover is not measured; it
  /// reuses the measured first-run morph, panel first and buttons 80 ms later; going back,
  /// the buttons return first and the panel widens 80 ms later, mirroring it).
  void _retarget() {
    final narrow =
        _hovered && !_searching && !_launcher && _scope == FaroScope.all;
    _spreadDelay?.cancel();
    if (narrow) {
      _springTo(_width, narrowWidth, widthSpring);
      if (_spread.value < 0.5) {
        _spreadDelay = Timer(buttonsDelay, () {
          if (mounted) _springTo(_spread, 1, buttonsSpring);
        });
      } else {
        _springTo(_spread, 1, buttonsSpring);
      }
    } else {
      _springTo(_spread, 0, buttonsSpring);
      if (_width.value < wideWidth - 1 && _spread.value > 0.5) {
        _spreadDelay = Timer(buttonsDelay, () {
          if (mounted) _springTo(_width, wideWidth, widthSpring);
        });
      } else {
        _springTo(_width, wideWidth, widthSpring);
      }
    }
  }

  void _setHovered(bool hovered) {
    if (hovered == _hovered) return;
    _hovered = hovered;
    _retarget();
  }

  /// The height follows the content with the vertical spring (results appearing, the answer
  /// card, the list growing or shrinking). Not measured (PROVISIONAL): the same spring as
  /// the opening height.
  void _followHeight(double target) {
    if (_heightTarget == target) return;
    final first = _heightTarget == null;
    _heightTarget = target;
    if (first) {
      _height.value = target;
      return;
    }
    // Starting a simulation notifies nobody synchronously: safe during build.
    _springTo(_height, target, verticalSpring);
  }

  /// The chosen row; with an answer card, 0 is the card and the rows start at 1.
  int _selected = 0;

  /// What Faro searches: everything, or the scope of a round button.
  FaroScope _scope = FaroScope.all;

  /// The folder chosen with Tab (the files scope inside it), and its name for the token.
  String? _under;
  String? _underName;
  late final SystemActions _actions = widget.actions ?? SystemActions();
  late final ClipboardHistory _clipboard =
      widget.clipboard ?? ClipboardHistory();

  /// Exchange rates for currency conversions, refreshed when Faro opens.
  final CurrencyRates _rates = CurrencyRates();

  /// Switches the scope (a round button, ⌘1–⌘4, Tab on a folder, Esc or ⌫ back to all).
  void _setScope(
    FaroScope scope, {
    String? under,
    String? underName,
    bool keepQuery = false,
  }) {
    // A new scope starts empty (the reopened query would filter it), except "Search in the
    // file manager", which carries the query over.
    if (!keepQuery && scope != FaroScope.all) _query.clear();
    setState(() {
      _scope = scope;
      _under = under;
      _underName = underName;
      _selected = 0;
    });
    switch (scope) {
      case FaroScope.actions:
        _actions.load(onDone: _tick);
      case FaroScope.clipboard:
        _clipboard.load(onChange: _tick);
      case FaroScope.files:
        _folders.refresh(onDone: _tick);
      case FaroScope.all:
        break;
    }
    _retarget();
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  /// Opens a folder in the file manager, anything else with its default app.
  void _openPath(String path, {required bool folder}) {
    final files = widget.entries[fileManagerId] ?? _appWithId('mustache');
    if (folder && files != null) {
      ShellChannel.instance.launch(
        execWithFiles(files.rawExec ?? files.exec, [path]) ??
            '${files.exec} ${shellQuote(path)}',
      );
    } else {
      _openWithDefault(path);
    }
  }

  /// `gio open` (always in the image); `xdg-open` where xdg-utils is installed.
  void _openWithDefault(String target) {
    final quoted = shellQuote(target);
    ShellChannel.instance.launch(
      'gio open $quoted 2>/dev/null || xdg-open $quoted',
    );
  }

  /// ⌘⏎: the folder that contains it, in the file manager.
  void _reveal(String path) {
    final parent = File(path).parent.path;
    _openPath(parent, folder: true);
    _hide();
  }

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
    // Dates of the rows ("today, 14:56") in every language of the shell.
    initializeDateFormatting();
    _query.addListener(() {
      // Moving the cursor or selecting keeps the chosen row.
      if (_query.text == _lastText) {
        setState(() {});
        return;
      }
      final wasSearching = _lastText.trim().isNotEmpty;
      _lastText = _query.text;
      setState(() => _selected = 0);
      // Typing on the narrow panel widens it again (the buttons go back first).
      if (wasSearching != _searching) _retarget();
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
    _spreadDelay?.cancel();
    _fade.dispose();
    _scaleY.dispose();
    _width.dispose();
    _height.dispose();
    _spread.dispose();
    _launcherClock.dispose();
    _back.dispose();
    _clipboard.dispose();
    _query.dispose();
    _focus.dispose();
    super.dispose();
  }

  bool get _open => _visible && !_closing;

  /// Opens Faro as ⌘Space does (tests: reopening while it closes).
  @visibleForTesting
  void reopenForTest() => _show();

  void _toggle() => _open && !_launcher ? _hide() : _show();

  /// Esc from the launcher: no fade, the panel turns into Faro (the measured return).
  void _backToFaro() {
    setState(() {
      _launcher = false;
      _query.clear();
      _hovered = false;
    });
    _spreadDelay?.cancel();
    _width.value = wideWidth;
    _spread.value = 0;
    _scaleY.value = 1;
    _fade.value = fadeMs;
    _heightTarget = null;
    _back.forward(from: 0);
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _toggleLauncher() => _open && _launcher ? _hide() : _showLauncher();

  void _showLauncher() {
    setState(() {
      _visible = true;
      _closing = false;
      _launcher = true;
      _recentIds = _recents.load();
    });
    _fade.forward(from: 0);
    _launcherClock.forward(from: 0);
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
  }

  void _show() {
    final reopening = _visible && _closing;
    setState(() {
      _visible = true;
      _closing = false;
      _launcher = false;
      _hovered = false;
      _scope = FaroScope.all;
      _under = null;
      _underName = null;
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
    _rates.load(onDone: _tick);
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
    _spreadDelay?.cancel();
    _spread.value = 0;
    _back.value = backDuration;
    if (reopening) {
      // Reopened while it was closing: it comes back from where it is, no restart.
      _fade.forward();
      _springTo(_scaleY, 1, verticalSpring);
      _springTo(_width, wideWidth, widthSpring);
    } else {
      // First frame 0.966 tall and 1.033 wide, centred; the fade from 0.22; then the
      // height and the width settle with their springs (spotlight › opening animation).
      _heightTarget = null;
      _fade.forward(from: 0);
      _scaleY.value = openScaleY;
      _springTo(_scaleY, 1, verticalSpring);
      _width.value = wideWidth * openScaleX;
      _springTo(_width, wideWidth, widthSpring);
    }
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  /// Closing (not measured, PROVISIONAL): the opening backwards. The fade runs back over
  /// 67 ms while the height returns to 0.966; input goes away at once, so a click on what is
  /// behind already reaches it.
  void _hide() {
    if (!_open) return;
    setState(() => _closing = true);
    _spreadDelay?.cancel();
    LayerControl.keyboard('none');
    LayerControl.inputRegion(const []);
    _fade.reverse();
    if (!_launcher) {
      _scaleY.animateTo(
        openScaleY,
        duration: Duration(microseconds: (_fade.value * 1000).round()),
      );
    }
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
        _openPath(folder.path, folder: true);
      case FileItem(:final file):
        _openPath(file.path, folder: false);
      case FallbackItem(kind: FallbackKind.files):
        // The same query, in the files scope.
        _setScope(FaroScope.files, keepQuery: true);
        return;
      case FallbackItem(kind: FallbackKind.web):
        final url = webSearchUrl + Uri.encodeQueryComponent(_query.text.trim());
        _openWithDefault(url);
      case ActionItem(:final action):
        // Closed first: an action on a window acts on the window in front, not on Faro.
        _hide();
        ShellChannel.instance.execute(action.id);
        return;
      case AppActionItem(:final action):
        ShellChannel.instance.launch(action.exec);
      case ClipItem(:final clip):
        _clipboard.copy(clip);
      case ClearClipboardItem():
        _clipboard.clear();
        return;
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
    final index = count == 0 ? -1 : _selected.clamp(0, count - 1);
    final item = index < 0 || (answer != null && index == 0)
        ? null
        : items[index - (answer == null ? 0 : 1)];
    final command = HardwareKeyboard.instance.isMetaPressed;
    // ⌘1–⌘4: the scopes of the round buttons (PROVISIONAL: the reference's keys are not
    // measured).
    if (command && event is KeyDownEvent) {
      final scope = switch (event.logicalKey) {
        LogicalKeyboardKey.digit1 => null,
        LogicalKeyboardKey.digit2 => FaroScope.files,
        LogicalKeyboardKey.digit3 => FaroScope.actions,
        LogicalKeyboardKey.digit4 => FaroScope.clipboard,
        _ => FaroScope.all,
      };
      if (event.logicalKey == LogicalKeyboardKey.digit1) {
        _showLauncher();
        return KeyEventResult.handled;
      }
      if (scope != FaroScope.all && scope != null) {
        _setScope(scope);
        return KeyEventResult.handled;
      }
    }
    switch (event.logicalKey) {
      case LogicalKeyboardKey.escape:
        // Out of a scope first, then closed.
        if (_scope != FaroScope.all) {
          _setScope(FaroScope.all);
        } else {
          _hide();
        }
      case LogicalKeyboardKey.backspace
          when _scope != FaroScope.all && _query.text.isEmpty:
        _setScope(FaroScope.all);
      case LogicalKeyboardKey.tab:
        // On a folder: search inside it (the "Search in …" hint, SH-SP-08).
        if (item case FolderItem(:final folder)) {
          _query.clear();
          _setScope(
            FaroScope.files,
            under: folder.path,
            underName: folder.name,
          );
        }
      case LogicalKeyboardKey.backspace when command && item is ClipItem:
        _clipboard.remove(item.clip);
      case LogicalKeyboardKey.arrowDown:
        if (count > 0) {
          setState(() => _selected = (_selected + 1).clamp(0, count - 1));
        }
      case LogicalKeyboardKey.arrowUp:
        if (count > 0) {
          setState(() => _selected = (_selected - 1).clamp(0, count - 1));
        }
      case LogicalKeyboardKey.enter || LogicalKeyboardKey.numpadEnter
          when command:
        // ⌘⏎: show it in the file manager.
        switch (item) {
          case FolderItem(:final folder):
            _reveal(folder.path);
          case FileItem(:final file):
            _reveal(file.path);
          default:
            break;
        }
      case LogicalKeyboardKey.enter || LogicalKeyboardKey.numpadEnter:
        if (count == 0) return KeyEventResult.handled;
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
    // An overlay of its own for the tooltips of the round buttons.
    return Overlay.wrap(child: _faroView(context));
  }

  Widget _faroView(BuildContext context) {
    final answer = _scope == FaroScope.all
        ? answerFor(_query.text, decimal: _decimal, rates: _rates.rates)
        : null;
    final scoped = _scope != FaroScope.all;
    final items = scoped
        ? scopeItems(
            _scope,
            query: _query.text,
            entries: widget.entries,
            folders: _folders,
            actions: _actions,
            clipboard: _clipboard,
            labelOf: _actionLabel,
            under: _under,
          )
        : [
            for (final item in faroItems(
              widget.entries.values,
              _folders,
              _query.text,
            ))
              if (!_appsOnly || item is AppItem || item is FallbackItem) item,
          ];
    // A scope lists its rows before anything is typed.
    final searching = _query.text.trim().isNotEmpty || scoped;
    final pills = scoped || !searching ? const <_Pill>[] : _pills(items);
    final pillsBlock = pills.isEmpty ? 0.0 : filterHeight;
    final count = items.length + (answer == null ? 0 : 1);
    final selected = count == 0 ? 0 : _selected.clamp(0, count - 1);
    final cardBlock = answer == null ? 0.0 : 1 + cardHeight + cardAfter;
    final listHeight = items.isEmpty
        ? 0.0
        : (items.length * rowHeight + 2 * listPadding).clamp(
            0.0,
            maxHeight - headerHeight - cardBlock - pillsBlock - 1,
          );
    final shows = searching && (items.isNotEmpty || answer != null);
    _followHeight(
      headerHeight + (shows ? cardBlock + 1 + pillsBlock + listHeight : 0),
    );
    final height = _height.value;
    final opacity = _fade.value <= 0 && _closing ? 0.0 : opacityAt(_fade.value);
    final scaleY = _scaleY.value;
    final width = _width.value;
    final slide = _spread.value;
    final buttons = slide > 0.001;

    return LayoutBuilder(
      builder: (context, constraints) {
        final screen = constraints.biggest;
        final frameLeft = (screen.width - wideWidth) / 2;
        // Wider than the 640 pt frame (the first frames of an opening) it stays centred;
        // narrower, its left edge stays on the frame and the right edge moves.
        var left = width > wideWidth
            ? frameLeft - (width - wideWidth) / 2
            : frameLeft;
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
            // The round buttons come out from behind the panel: the panel itself is glass
            // drawn by the compositor under the surface, so what is "behind" it is cut away
            // here, left of its right edge (their glass merges with the panel's, as drops).
            if (buttons && back >= 1)
              Positioned(
                left: left + width,
                top: top + (headerHeight - circle) / 2 - 2,
                width: (frameLeft + wideWidth + 40 - (left + width)).clamp(
                  0.0,
                  double.infinity,
                ),
                height: circle + 4,
                child: ClipRect(
                  child: Stack(
                    clipBehavior: Clip.none,
                    children: [
                      for (var i = 3; i >= 0; i--)
                        Positioned(
                          // All overlapped at the panel's right end at first, then out to
                          // the 64 pt pitch.
                          left:
                              -circle +
                              (narrowWidth +
                                      10 +
                                      i * circlePitch -
                                      width +
                                      circle) *
                                  slide,
                          top: 2,
                          width: circle,
                          height: circle,
                          child: Opacity(
                            key: ValueKey('faro button $i'),
                            opacity: opacity,
                            child: _circleButton(i),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
            Positioned(
              left: left,
              top: top,
              width: panelWidth,
              height: panelHeight,
              child: Transform(
                key: const ValueKey('faro panel'),
                alignment: Alignment.center,
                transform: Matrix4.diagonal3Values(1, scaleY, 1),
                child: Opacity(
                  opacity: opacity,
                  child: _panel(
                    answer,
                    items,
                    selected,
                    listHeight,
                    pills: pills,
                    panelRadius: panelRadius,
                  ),
                ),
              ),
            ),
            // The hover zone is the whole 640 pt frame, buttons included: the panel narrowing
            // under the pointer must not count as leaving it.
            if (back >= 1)
              Positioned(
                left: frameLeft,
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
    List<_Pill> pills = const [],
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
                if (pills.isNotEmpty) _filterRow(pills),
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
    // Nothing typed (a scope listing everything): nothing to complete.
    if (query.isEmpty || index < 0 || index >= items.length) {
      return (null, null);
    }
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
      case FileItem(:final file):
        final r = rest(file.name);
        return (r == null ? null : l.completionOpen(r), null);
      case ActionItem(:final action):
        final r = rest(_actionLabel(action));
        return (
          r == null ? null : l.faroCompletionAction(_actionLabel(action)),
          null,
        );
      case AppActionItem(:final entry, :final action):
        final r = rest(action.name);
        return (
          r == null ? null : l.faroCompletionAction(action.name),
          _icon(entry),
        );
      case ClipItem() || ClearClipboardItem():
        return (null, null);
    }
  }

  /// A system action's name: the shell's translation of the catalog's label, or the
  /// catalog's own for a command the shell does not know yet.
  String _actionLabel(SystemAction action) {
    final l = l10nOf(context);
    return switch (action.id) {
      'screenshot.screen' => l.faroActionScreenshotScreen,
      'screenshot.selection' => l.faroActionScreenshotSelection,
      'screenshot.toolbar' => l.faroActionScreenshotToolbar,
      'system.dark_toggle' => l.faroActionSystemDarkToggle,
      'system.mute_toggle' => l.faroActionSystemMuteToggle,
      'system.volume_up' => l.faroActionSystemVolumeUp,
      'system.volume_down' => l.faroActionSystemVolumeDown,
      'system.brightness_up' => l.faroActionSystemBrightnessUp,
      'system.brightness_down' => l.faroActionSystemBrightnessDown,
      'panorama.toggle' => l.faroActionPanoramaToggle,
      'panorama.app' => l.faroActionPanoramaApp,
      'desktop.show' => l.faroActionDesktopShow,
      'space.add' => l.faroActionSpaceAdd,
      'space.next' => l.faroActionSpaceNext,
      'space.previous' => l.faroActionSpacePrevious,
      'launcher.toggle' => l.faroActionLauncherToggle,
      'notifications.toggle' => l.faroActionNotificationsToggle,
      'controlcenter.toggle' => l.faroActionControlcenterToggle,
      'window.minimize' => l.faroActionWindowMinimize,
      'window.fullscreen' => l.faroActionWindowFullscreen,
      'window.tile_left' => l.faroActionWindowTileLeft,
      'window.tile_right' => l.faroActionWindowTileRight,
      'window.tile_top' => l.faroActionWindowTileTop,
      'window.tile_bottom' => l.faroActionWindowTileBottom,
      'window.tile_fill' => l.faroActionWindowTileFill,
      'window.arrange_2' => l.faroActionWindowArrange2,
      'window.arrange_3' => l.faroActionWindowArrange3,
      'window.arrange_4' => l.faroActionWindowArrange4,
      'app.force_quit_panel' => l.faroActionAppForceQuitPanel,
      'session.quit' => l.faroActionSessionQuit,
      _ => action.label,
    };
  }

  String get _placeholder {
    final l = l10nOf(context);
    return switch (_scope) {
      FaroScope.all => l.faroPlaceholder,
      FaroScope.files =>
        _underName == null ? l.faroSearchFiles : l.faroSearchIn(_underName!),
      FaroScope.actions => l.faroSearchActions,
      FaroScope.clipboard => l.faroSearchClipboard,
    };
  }

  String _scopeName(FaroScope scope) {
    final l = l10nOf(context);
    return switch (scope) {
      FaroScope.all => '',
      FaroScope.files => _underName ?? l.faroScopeFiles,
      FaroScope.actions => l.faroScopeActions,
      FaroScope.clipboard => l.faroScopeClipboard,
    };
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
                // The scope as a token, like the Launcher's category (PROVISIONAL size: the
                // Launcher's 31 pt token).
                if (_scope != FaroScope.all) ...[
                  GestureDetector(
                    onTap: () => _setScope(FaroScope.all),
                    child: Container(
                      key: const ValueKey('faro scope token'),
                      height: tokenHeight,
                      padding: const EdgeInsets.only(left: 6, right: 11),
                      alignment: Alignment.center,
                      decoration: BoxDecoration(
                        color: _c.selection,
                        borderRadius: const BorderRadius.all(
                          Radius.circular(tokenRadius),
                        ),
                      ),
                      child: Text(
                        _scopeName(_scope),
                        maxLines: 1,
                        style: uiText(size: 26, color: _c.tokenText),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                ],
                Expanded(
                  child: Stack(
                    alignment: Alignment.centerLeft,
                    children: [
                      if (_query.text.isEmpty)
                        IgnorePointer(
                          child: Text(
                            _placeholder,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
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
    final subtitleStyle = uiText(size: 15, color: _c.subtitle);
    Widget glyph(int index) =>
        CustomPaint(painter: _CircleGlyphPainter(index, _c.placeholder));
    final (
      IconSource? icon,
      Widget? leading,
      String title,
      Widget? subtitle,
    ) = switch (item) {
      AppItem(:final entry) => (
        _icon(entry),
        null,
        entry.name,
        Text(l.faroApplication, style: subtitleStyle),
      ),
      FolderItem(:final folder) => (
        fileIcon(Directory(folder.path)),
        null,
        folder.name,
        _folderSubtitle(folder),
      ),
      FileItem(:final file) => (
        fileIcon(File(file.path)),
        null,
        file.name,
        _folderSubtitle(file),
      ),
      FallbackItem(:final kind) => (
        switch (kind == FallbackKind.files ? _appWithId('mustache') : null) {
          final e? => _icon(e),
          null => null,
        },
        CustomPaint(painter: _FallbackGlyphPainter(kind, _c.placeholder)),
        _fallbackLabel(kind),
        null,
      ),
      ActionItem(:final action) => (
        null,
        glyph(2),
        _actionLabel(action),
        Text(l.faroSystemAction, style: subtitleStyle),
      ),
      AppActionItem(:final entry, :final action) => (
        _icon(entry),
        null,
        action.name,
        Text(entry.name, style: subtitleStyle),
      ),
      ClipItem(:final clip) => (
        clip.kind == ClipKind.files && clip.paths.isNotEmpty
            ? fileIcon(File(clip.paths.first))
            : null,
        switch (clip.kind) {
          ClipKind.image when clip.image != null => ClipRRect(
            borderRadius: const BorderRadius.all(Radius.circular(4)),
            child: Image.file(
              File(clip.image!),
              width: 38,
              height: 38,
              fit: BoxFit.cover,
            ),
          ),
          _ => glyph(3),
        },
        clip.kind == ClipKind.image ? l.faroImage : clip.title,
        Text(l.faroCopied(_when(clip.time)), style: subtitleStyle),
      ),
      ClearClipboardItem() => (null, null, l.faroClearHistory, null),
    };
    // Keyboard shortcuts as key glyphs; media keys (XF86…) have their own keys on the
    // keyboard, not a combination to remember: none shown.
    final shortcut = switch (item) {
      ActionItem(:final action)
          when !(action.shortcut ?? '').contains('XF86') =>
        action.shortcut,
      _ => null,
    };
    final path = switch (item) {
      FolderItem(:final folder) => folder.path,
      FileItem(:final file) => file.path,
      _ => null,
    };
    Widget row = GestureDetector(
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
                  null => leading,
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
              if (shortcut != null) ...[
                const SizedBox(width: 8),
                Text(
                  shortcutGlyphs(shortcut),
                  style: uiText(size: 15, color: _c.subtitle),
                ),
                const SizedBox(width: 14),
              ],
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
    // A file or a folder can be dragged out of Faro, onto the Dock, a window or the
    // desktop: past the drag threshold Faro closes and the drag continues as a system drag.
    if (path != null) {
      row = Listener(
        onPointerDown: (e) => _dragFrom = (path, e.position),
        onPointerMove: (e) {
          final from = _dragFrom;
          if (from == null || from.$1 != path) return;
          if ((e.position - from.$2).distance < dragThreshold) return;
          _dragFrom = null;
          _hide();
          SilicaDrag.start([path]);
        },
        onPointerUp: (_) => _dragFrom = null,
        onPointerCancel: (_) => _dragFrom = null,
        child: row,
      );
    }
    return row;
  }

  /// The filter pills over the results (SH-SP-12): where else the search can go. On the
  /// reference they are the apps that can search further; here (PROVISIONAL) the kinds Silica
  /// has: applications only, and the files scope (the web is the last row already).
  List<_Pill> _pills(List<FaroItem> items) {
    final l = l10nOf(context);
    final hasApps = items.any((i) => i is AppItem);
    return [
      if (hasApps)
        _Pill(
          l.faroScopeApps,
          selected: _appsOnly,
          onTap: () => setState(() {
            _appsOnly = !_appsOnly;
            _selected = 0;
          }),
        ),
      _Pill(l.faroScopeFiles, onTap: () => _setScope(FaroScope.files)),
    ];
  }

  /// Pills 26 pt tall, 8 pt apart, radius 7.5, 10 pt above and 11 below, in a row that
  /// scrolls sideways and is cut by the panel. Their fill and text are the Launcher's
  /// category capsules' (the same measured `#d9dadf` over `#c8c9ce` and `#5a5b60` in light).
  Widget _filterRow(List<_Pill> pills) => SizedBox(
    height: filterHeight,
    child: ListView.separated(
      key: const ValueKey('faro pills'),
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.fromLTRB(20, 10, 20, 11),
      itemCount: pills.length,
      separatorBuilder: (_, _) => const SizedBox(width: 8),
      itemBuilder: (context, i) {
        final pill = pills[i];
        return GestureDetector(
          onTap: pill.onTap,
          child: Container(
            height: 26,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: pill.selected ? _c.selection : _c.categoryFill,
              borderRadius: const BorderRadius.all(Radius.circular(7.5)),
            ),
            child: Text(
              pill.label,
              style: uiText(size: 15, color: _c.categoryText),
            ),
          ),
        );
      },
    ),
  );

  /// Only the applications (the Applications pill).
  bool _appsOnly = false;

  /// Where a press on a file row started (the drag out of Faro).
  (String, Offset)? _dragFrom;

  /// "today, 14:56", "yesterday, 9:02" or the date.
  String _when(DateTime m) {
    final l = l10nOf(context);
    final now = DateTime.now();
    final day = DateTime(now.year, now.month, now.day);
    final time = DateFormat.Hm(l.localeName).format(m);
    return !m.isBefore(day)
        ? l.faroToday(time)
        : !m.isBefore(day.subtract(const Duration(days: 1)))
        ? l.faroYesterday(time)
        : DateFormat.yMMMd(l.localeName).format(m);
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

  /// A round button: Applications (the Launcher), Files, Actions, Clipboard, with its name in
  /// a tooltip and ⌘1–⌘4.
  Widget _circleButton(int index) {
    final l = l10nOf(context);
    final (name, onTap) = switch (index) {
      0 => (l.faroScopeApps, _showLauncher),
      1 => (l.faroScopeFiles, () => _setScope(FaroScope.files)),
      2 => (l.faroScopeActions, () => _setScope(FaroScope.actions)),
      _ => (l.faroScopeClipboard, () => _setScope(FaroScope.clipboard)),
    };
    return SilicaTooltip(
      message: '$name  ⌘${index + 1}',
      child: Semantics(
        button: true,
        label: name,
        child: GestureDetector(
          onTap: onTap,
          child: LiquidGlass(
            radius: circle / 2,
            material: GlassMaterial.thick,
            interactive: true,
            child: CustomPaint(
              painter: _CircleGlyphPainter(index, _c.placeholder),
            ),
          ),
        ),
      ),
    );
  }

  Widget _launcherView() {
    final elapsed = _launcherClock.value;
    final opacity = _fade.value <= 0 && _closing ? 0.0 : opacityAt(_fade.value);
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
                  opacity: opacity,
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
