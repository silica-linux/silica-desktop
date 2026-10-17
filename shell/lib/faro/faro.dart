// Faro, the search panel, on the Amalfi kit (`Apps/Faro/*`; docs/design/amalfi/components.md ›
// Faro). Thick glass, centred, its top at 30 % of the screen and never wider than 640 pt: a
// 640 × 56 capsule (radius 28) with the magnifier at x 20 and the field at x 56; after the
// pointer enters the empty bar it is 384 pt wide with four 56 pt round buttons beside it;
// with results a panel 640 wide, header 56, filter row 40, rows 56, at most 440 pt tall; the
// Files and Actions scopes are 544 pt tall.
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
import '../system/glass_profile.dart';
import '../system/open_app.dart';
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shadow_image.dart';
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

/// The top edge of the panel: 30 % of the screen height in logical points (spotlight.md),
/// snapped to a device pixel so it is the same for any scale and resolution.
double panelTop(Size screen, double devicePixelRatio) =>
    (screen.height * topFraction * devicePixelRatio).roundToDouble() /
    devicePixelRatio;

/// The left edge of a panel of `width` centred on the screen, snapped to a device pixel.
double panelLeft(Size screen, double width, double devicePixelRatio) =>
    ((screen.width - width) / 2 * devicePixelRatio).roundToDouble() /
    devicePixelRatio;
const double headerHeight = 56;
const double rowHeight = 56;

/// App icon in the row: 32 pt at x 20.
const double rowIconSide = 32;
const double maxHeight = 440;
const double radius = AmalfiRadii.faroAndLauncher;

/// The Files and Actions scopes: 640 x 544 pt (the header and 488 pt below it).
const double filesHeight = 544;
const int filesColumns = 5;
const double gridInset = 10;
const double gridCell = 104;

/// The heading above the first action of an app in the actions scope.
const double sectionHeight = 28;

/// The actions scope's footer (a separator at 504 and a caption).
const double scopeFooterHeight = 40;

/// Round buttons of the empty state: diameter 56, pitch 64, 8 pt from the bar.
const double circle = 56;
const double circlePitch = 64;
const double circleGap = 8;

/// The panel's drop shadow: the kit's `Shadow/3`, drawn outside the glass.
class PanelShadow extends StatelessWidget {
  const PanelShadow({super.key, required this.radius, required this.child});

  final double radius;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final shadow = FaroColors.of(context).shadow;
    return OuterShadow(
      radius: radius,
      // The kit's CSS blur has σ = blur / 2; OuterShadow takes a Skia blur radius.
      blurRadius: (shadow.blurRadius / 2 - 0.5) / 0.57735,
      offset: shadow.offset,
      color: shadow.color,
      child: child,
    );
  }
}

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

/// Answer card (`Apps/Faro/Calculation`): 20 pt from the panel sides, 8 under the header,
/// 84 tall, radius 16, `Surface/Sunken`; 4 pt of space under it.
const double cardInset = 20;
const double cardBefore = 8;
const double cardHeight = 84;
const double cardRadius = 16;
const double cardBorder = 2;
const double cardAfter = 4;

/// Copy button in the card: the 28 pt round icon button.
const double copySide = 28;

/// The result list: rows start right under the header or the filter row, and end with 8 pt.
const double listPadding = 8;

/// The icon of the app handling the chosen row, at the right of the field.
const double fieldIconSide = 28;
const double fieldIconRight = 20;

/// The filter row over the results: 40 pt (pills 24 tall from 8 under the header).
const double filterHeight = 40;

class _Pill {
  const _Pill(this.label, {required this.onTap, this.selected = false});
  final String label;
  final VoidCallback onTap;
  final bool selected;
}

/// Distance a press on a file row travels before it becomes a drag (PROVISIONAL: the
/// reference's list threshold, 5 pt in the sidebar).
const double dragThreshold = 5;

/// "Tab" key capsule on the chosen folder row: 20 tall, radius 10, 20 pt from the row's end.
const double keycapHeight = 20;
const double keycapRadius = 10;

class _FaroState extends State<Faro>
    with TickerProviderStateMixin, WidgetsBindingObserver {
  /// This component's layer surface (several share one engine, #201).
  late final LayerControl _layer;

  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  StreamSubscription<String>? _commands;
  StreamSubscription<void>? _dismissed;
  DateTime? _dismissedAt;

  /// What was last given to Ossido as the panel's rectangles (only when it changes).
  String _regionKey = '';
  bool _visible = false;
  bool _launcher = false;

  /// The Launcher's list mode (the "…" menu's choice), remembered in the state folder.
  bool _launcherList = false;

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
          reverseDuration: closeDuration,
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

  /// The pointer entered the empty capsule and waited [engageDelay]: the field narrows and the
  /// discs come out, and they stay while Faro is open (or until something is typed).
  bool _engaged = false;
  Timer? _engageTimer;

  /// A scope was just chosen: the panel's next height change overshoots by 5 pt.
  bool _scopeChanged = false;

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

  /// The width and the buttons follow the state: narrow with the buttons out once the pointer
  /// has entered the empty panel (the field settles ~690 ms after it entered, the discs
  /// ~800 ms, and they stay); the field first and the discs 80 ms later. Typing or a scope
  /// widens it again: the buttons return first and the panel widens 80 ms later.
  void _retarget() {
    final narrow =
        _engaged && !_searching && !_launcher && _scope == FaroScope.all;
    _spreadDelay?.cancel();
    if (narrow) {
      _springTo(_width, narrowWidth, engageWidthSpring);
      if (_spread.value < 0.5) {
        _spreadDelay = Timer(buttonsDelay, () {
          if (mounted) _springTo(_spread, 1, engageButtonsSpring);
        });
      } else {
        _springTo(_spread, 1, engageButtonsSpring);
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

  /// The pointer on the capsule: after [engageDelay] it engages. Leaving earlier cancels it;
  /// leaving later changes nothing (the discs stay).
  void _setHovered(bool hovered) {
    if (hovered == _hovered) return;
    _hovered = hovered;
    if (!hovered) {
      if (!_engaged) _engageTimer?.cancel();
      return;
    }
    if (_engaged) return;
    _engageTimer?.cancel();
    _engageTimer = Timer(engageDelay, () {
      if (!mounted || !_open || _launcher) return;
      _engaged = true;
      _retarget();
    });
  }

  /// The height follows the content with the vertical spring (results appearing, the answer
  /// card, the list growing or shrinking): the opening height's spring; a change of scope
  /// overshoots by +5 pt.
  void _followHeight(double target) {
    if (_heightTarget == target) return;
    final first = _heightTarget == null;
    _heightTarget = target;
    if (first) {
      _height.value = target;
      return;
    }
    // Starting a simulation notifies nobody synchronously: safe during build.
    _springTo(_height, target, _scopeChanged ? scopeSpring : verticalSpring);
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
      _scopeChanged = true;
    });
    _loadScope(scope);
    _retarget();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _scopeChanged = false;
      _focus.requestFocus();
    });
  }

  /// What a scope lists is read when it opens.
  void _loadScope(FaroScope scope) {
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
  late final AnimationController _back =
      AnimationController(
        vsync: this,
        upperBound: backDuration,
        value: backDuration,
        duration: const Duration(microseconds: 213300),
      )..addListener(() {
        // The measured return drives the same morph as the way in: the Launcher-ness runs 1 → 0
        // along the measured height curve.
        _mix.value = 1 - backProgress(_back.value);
      });

  /// How much of the Launcher the one container is: 0 = the Faro bar, 1 = the Launcher
  /// (position, size and radius of the single glass region interpolate with it).
  late final AnimationController _mix = AnimationController.unbounded(
    vsync: this,
  )..addListener(_tick);

  /// Grid (0) to list (1) in the Launcher: the container morphs between 844 x 580 and 640 x 560.
  late final AnimationController _listMix = AnimationController.unbounded(
    vsync: this,
    value: _launcherList ? 1 : 0,
  )..addListener(_tick);
  late final RecentApps _recents = widget.recents ?? RecentApps();
  late final FolderIndex _folders = widget.folders ?? FolderIndex.home();
  List<String> _recentIds = const [];
  String _lastText = '';

  @override
  void initState() {
    super.initState();
    _layer = LayerControl.of(context);
    _launcherList = _recents.loadList();
    WidgetsBinding.instance.addObserver(this);
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
    _dismissed = _layer.transientDismissed.listen((_) => _transientDismissed());
    if (widget.startLauncher) {
      _showLauncher();
    } else if (widget.startOpen) {
      _show();
    } else {
      _layer.inputRegion(const []);
      _layer.keyboard('none');
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _commands?.cancel();
    _dismissed?.cancel();
    _spreadDelay?.cancel();
    _engageTimer?.cancel();
    _fade.dispose();
    _scaleY.dispose();
    _width.dispose();
    _height.dispose();
    _spread.dispose();
    _launcherClock.dispose();
    _back.dispose();
    _mix.dispose();
    _listMix.dispose();
    _clipboard.dispose();
    _query.dispose();
    _focus.dispose();
    super.dispose();
  }

  bool get _open => _visible && !_closing;

  /// Opens Faro as ⌘Space does (tests: reopening while it closes).
  @visibleForTesting
  void reopenForTest() => _show();

  void _toggle() {
    if (_justDismissed) return;
    _open && !_launcher ? _hide() : _show();
  }

  /// A press outside the panel ended it (Ossido, #149).
  void _transientDismissed() {
    if (!_open) return;
    _dismissedAt = DateTime.now();
    _hide();
  }

  /// The shortcut pressed while the panel is open: Ossido first ends it (transientDismissed),
  /// then the command arrives, which must not open it again.
  bool get _justDismissed {
    final at = _dismissedAt;
    return !_open &&
        at != null &&
        DateTime.now().difference(at) < const Duration(milliseconds: 400);
  }

  /// The output's scale or size changed: the panel is placed again and the input region
  /// is sent again, even when the logical rectangles end up equal.
  @override
  void didChangeMetrics() {
    _regionKey = '';
    if (mounted) setState(() {});
  }

  /// The panel's rectangles are the only place Faro takes the pointer; a press outside
  /// arrives from Ossido (#149), so the windows below keep their hover. Without that support
  /// Faro covers the screen.
  void _syncRegion(List<Rect> rects) {
    if (!_open) return;
    final key = rects
        .map(
          (r) =>
              '${r.left.round()},${r.top.round()},${r.width.round()},${r.height.round()}',
        )
        .join(' ');
    if (key == _regionKey) return;
    _regionKey = key;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted || !_open) return;
      final supported = await _layer.transient(
        rects: rects,
        policy: TransientPolicy.panel,
      );
      if (!mounted || !_open) return;
      _layer.inputRegion(supported ? rects : null);
    });
  }

  /// Esc from the launcher: no fade, the panel turns into Faro (the measured return).
  void _backToFaro() {
    setState(() {
      _launcher = false;
      _query.clear();
      _hovered = false;
      _engaged = false;
    });
    _spreadDelay?.cancel();
    _engageTimer?.cancel();
    _width.value = wideWidth;
    _spread.value = 0;
    _scaleY.value = 1;
    _fade.value = fadeMs;
    _heightTarget = null;
    _mix.value = 1;
    _back.forward(from: 0);
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  /// Cmd+2..4 in the Launcher go straight to that scope of Faro; Cmd+1 is where it is.
  KeyEventResult _launcherKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent || !HardwareKeyboard.instance.isMetaPressed) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.digit1) {
      return KeyEventResult.handled;
    }
    final scope = scopeOfDigit(event.logicalKey);
    if (scope == null) return KeyEventResult.ignored;
    _backToFaro();
    _setScope(scope);
    return KeyEventResult.handled;
  }

  void _toggleLauncher() {
    if (_justDismissed) return;
    _open && _launcher ? _hide() : _showLauncher();
  }

  void _showLauncher() {
    if (_open && !_launcher) {
      // Faro is open: the bar morphs into the Launcher (one container, one glass region).
      final mix = _mix.value;
      setState(() {
        _launcher = true;
        _hovered = false;
        _recentIds = _recents.load();
      });
      _spreadDelay?.cancel();
      _engageTimer?.cancel();
      _back.value = backDuration;
      _mix.value = mix;
      _springTo(_mix, 1, launcherMorphSpring);
      _layer.keyboard('exclusive');
      return;
    }
    setState(() {
      _visible = true;
      _closing = false;
      _launcher = true;
      _hovered = false;
      _engaged = false;
      _recentIds = _recents.load();
    });
    _spreadDelay?.cancel();
    _engageTimer?.cancel();
    _width.value = wideWidth;
    _spread.value = 0;
    _scaleY.value = 1;
    _back.value = backDuration;
    _mix.value = 1;
    _listMix.value = _launcherList ? 1 : 0;
    _fade.forward(from: 0);
    _launcherClock.forward(from: 0);
    // The region follows the panel from the first frame (_syncRegion); until then nothing.
    _regionKey = '';
    _layer.keyboard('exclusive');
  }

  /// Grid or list: the container morphs between the two sizes.
  void _setLauncherList(bool list) {
    setState(() => _launcherList = list);
    _recents.saveList(list);
    _springTo(_listMix, list ? 1 : 0, launcherMorphSpring);
  }

  void _show() {
    final reopening = _visible && _closing;
    setState(() {
      _visible = true;
      _closing = false;
      _launcher = false;
      _hovered = false;
      _engaged = false;
      // The scope stays as it was left (also the folder of the files scope).
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
    _loadScope(_scope);
    _regionKey = '';
    _layer.keyboard('exclusive');
    _spreadDelay?.cancel();
    _engageTimer?.cancel();
    _spread.value = 0;
    _back.value = backDuration;
    _mix.value = 0;
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

  /// Closing takes ~100 ms: the opening backwards. The fade runs back while the height
  /// returns to 0.966; input goes away at once, so a click on what is behind already
  /// reaches it.
  void _hide() {
    if (!_open) return;
    setState(() => _closing = true);
    _spreadDelay?.cancel();
    _engageTimer?.cancel();
    _layer.keyboard('none');
    _layer.clearTransient();
    _layer.inputRegion(const []);
    _regionKey = '';
    _fade.reverse();
    if (!_launcher) {
      _scaleY.animateTo(
        openScaleY,
        duration: closeDuration * (_fade.value / fadeMs),
      );
    }
  }

  void _launch(DesktopEntry entry) {
    openApp(entry);
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
    // Cmd+1..4 open the scopes of the round buttons, also straight from one to another.
    if (command && event is KeyDownEvent) {
      if (event.logicalKey == LogicalKeyboardKey.digit1) {
        _showLauncher();
        return KeyEventResult.handled;
      }
      if (scopeOfDigit(event.logicalKey) case final scope?) {
        _setScope(scope);
        return KeyEventResult.handled;
      }
    }
    final grid = _scope == FaroScope.files;
    switch (event.logicalKey) {
      case LogicalKeyboardKey.escape:
        // In a scope: the text first, then out of the scope; then closed.
        if (_scope != FaroScope.all) {
          if (_query.text.isNotEmpty) {
            _query.clear();
          } else {
            _setScope(FaroScope.all);
          }
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
          final step = grid ? filesColumns : 1;
          setState(() => _selected = (_selected + step).clamp(0, count - 1));
        }
      case LogicalKeyboardKey.arrowUp:
        if (count > 0) {
          final step = grid ? filesColumns : 1;
          setState(() => _selected = (_selected - step).clamp(0, count - 1));
        }
      case LogicalKeyboardKey.arrowRight when grid && _query.text.isEmpty:
        if (count > 0) {
          setState(() => _selected = (_selected + 1).clamp(0, count - 1));
        }
      case LogicalKeyboardKey.arrowLeft when grid && _query.text.isEmpty:
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
    // An overlay of its own (the field's selection handles and toolbar). Faro's menus and
    // popovers declare Ossido glass regions like the panel itself (not silica_ui's in-Flutter
    // approximation).
    return AmalfiGlassBackendScope(
      backend: AmalfiGlassBackend.compositor,
      child: Overlay.wrap(child: _faroView(context)),
    );
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
            systemSection: l10nOf(context).faroSectionSystem,
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
    final cardBlock = answer == null
        ? 0.0
        : cardBefore + cardHeight + cardAfter;
    final grid = _scope == FaroScope.files;
    final actionsScope = _scope == FaroScope.actions;
    final headings = actionsScope ? _headings(items) : const <int, String>{};
    // Files and Actions are 544 tall, the list scrolling inside; the rest is as tall as its
    // rows (at most 440).
    final listHeight = grid
        ? filesHeight - headerHeight
        : actionsScope
        ? filesHeight - headerHeight - scopeFooterHeight
        : items.isEmpty
        ? 0.0
        : (items.length * rowHeight +
                  headings.length * sectionHeight +
                  listPadding)
              .clamp(0.0, maxHeight - headerHeight - cardBlock - pillsBlock);
    final shows =
        grid ||
        actionsScope ||
        searching && (items.isNotEmpty || answer != null);
    _followHeight(
      headerHeight +
          (shows
              ? cardBlock +
                    pillsBlock +
                    listHeight +
                    (actionsScope ? scopeFooterHeight : 0)
              : 0),
    );
    final height = _height.value;
    final opacity = _fade.value <= 0 && _closing ? 0.0 : opacityAt(_fade.value);
    final scaleY = _scaleY.value;
    // The Launcher opened from nothing starts ~8 % larger and shrinks (measured).
    final openScale = _launcher ? launcherScaleAt(_launcherClock.value) : 1.0;
    final width = _width.value;
    final slide = _spread.value;
    final buttons = slide > 0.001;

    return LayoutBuilder(
      builder: (context, constraints) {
        final screen = constraints.biggest;
        final dpr = MediaQuery.devicePixelRatioOf(context);
        final frameLeft = panelLeft(screen, wideWidth, dpr);
        // Wider than the 640 pt frame (the first frames of an opening) it stays centred;
        // narrower, its left edge stays on the frame and the right edge moves.
        var left = width > wideWidth
            ? frameLeft - (width - wideWidth) / 2
            : frameLeft;
        final top = panelTop(screen, dpr);
        // One container: the Faro bar at mix 0, the Launcher (grid or list) at mix 1. Left,
        // width, height and radius interpolate; the glass region follows every frame.
        final mix = _mix.value;
        final big = _launcherBox();
        var panelWidth = width;
        var panelHeight = height;
        var panelRadius = radius;
        if (mix > 0) {
          double lerp(double a, double b) => a + (b - a) * mix;
          left = lerp(left, panelLeft(screen, big.width, dpr));
          panelWidth = lerp(width, big.width);
          panelHeight = lerp(height, big.height);
          panelRadius = lerp(radius, launcherRadius);
        }
        final morphing = mix > 0 || _launcher;
        _syncRegion([
          Rect.fromLTWH(left, top, panelWidth, panelHeight).inflate(2),
          if (buttons && !morphing)
            Rect.fromLTWH(
              left + width,
              top + (headerHeight - circle) / 2 - 2,
              (frameLeft + wideWidth + 40 - (left + width)).clamp(
                0.0,
                double.infinity,
              ),
              circle + 4,
            ),
        ]);
        return Stack(
          children: [
            // A click outside closes Faro (without transient support).
            Positioned.fill(
              child: GestureDetector(
                onTap: _hide,
                behavior: HitTestBehavior.opaque,
              ),
            ),
            // The round buttons come out from behind the panel: the panel itself is glass
            // drawn by the compositor under the surface, so what is "behind" it is cut away
            // here, left of its right edge (their glass merges with the panel's, as drops).
            if (buttons && !morphing)
              Positioned(
                // Keyed, like the panel: the buttons coming and going must not shift the
                // panel to another element (its field would lose focus and the typed text).
                key: const ValueKey('faro buttons'),
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
                                      circleGap +
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
              key: const ValueKey('faro panel slot'),
              left: left,
              top: top,
              width: panelWidth,
              height: panelHeight,
              child: Transform(
                key: const ValueKey('faro panel'),
                alignment: Alignment.center,
                transform: Matrix4.diagonal3Values(
                  openScale,
                  scaleY * openScale,
                  1,
                ),
                child: Opacity(
                  opacity: opacity,
                  child: _panel(
                    answer,
                    items,
                    selected,
                    listHeight,
                    pills: pills,
                    headings: headings,
                    panelRadius: panelRadius,
                    mix: mix,
                  ),
                ),
              ),
            ),
            // The hover zone is the whole 640 pt frame, buttons included: the panel narrowing
            // under the pointer must not count as leaving it.
            if (!morphing)
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
    Map<int, String> headings = const {},
    double panelRadius = radius,
    double mix = 0,
  }) {
    final offset = answer == null ? 0 : 1;
    final grid = _scope == FaroScope.files;
    final actionsScope = _scope == FaroScope.actions;
    final l = l10nOf(context);
    // The separator under the header (kit: y 55, 20 pt inset) when something shows below.
    final body = items.isNotEmpty || answer != null || grid || actionsScope;
    return PanelShadow(
      radius: panelRadius,
      child: ProfiledGlass(
        profile: ShellGlassProfile.panel,
        radius: panelRadius,
        material: GlassMaterial.thick,
        child: _morphStack(
          mix,
          ClipRect(
            child: OverflowBox(
              alignment: Alignment.topLeft,
              maxHeight: double.infinity,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  SizedBox(
                    height: headerHeight,
                    child: Stack(
                      children: [
                        Positioned.fill(child: _field(answer, items, selected)),
                        if (body)
                          Positioned(
                            left: 20,
                            right: 20,
                            bottom: 0,
                            height: 1,
                            child: ColoredBox(color: _c.separator),
                          ),
                      ],
                    ),
                  ),
                  if (answer != null) ...[
                    const SizedBox(height: cardBefore),
                    _card(answer, selected == 0),
                    const SizedBox(height: cardAfter),
                  ],
                  if (pills.isNotEmpty) _filterRow(pills),
                  if (grid)
                    SizedBox(
                      height: listHeight,
                      child: AmalfiScrollbar(
                        child: GridView.builder(
                          key: const ValueKey('faro files grid'),
                          padding: const EdgeInsets.all(gridInset),
                          gridDelegate:
                              const SliverGridDelegateWithFixedCrossAxisCount(
                                crossAxisCount: filesColumns,
                                mainAxisExtent: gridCell,
                              ),
                          itemCount: items.length,
                          itemBuilder: (context, i) =>
                              _cell(items[i], i + offset == selected),
                        ),
                      ),
                    )
                  else if (items.isNotEmpty)
                    SizedBox(
                      height: listHeight,
                      child: AmalfiScrollbar(
                        child: ListView.builder(
                          padding: const EdgeInsets.only(bottom: listPadding),
                          itemExtentBuilder: (i, _) =>
                              rowHeight +
                              (headings.containsKey(i) ? sectionHeight : 0),
                          itemCount: items.length,
                          itemBuilder: (context, i) {
                            final row = SizedBox(
                              height: rowHeight,
                              child: _row(items[i], i + offset == selected),
                            );
                            final heading = headings[i];
                            return heading == null
                                ? row
                                : Column(children: [_heading(heading), row]);
                          },
                        ),
                      ),
                    ),
                  if (actionsScope)
                    SizedBox(
                      height: scopeFooterHeight,
                      child: Stack(
                        children: [
                          Positioned(
                            left: 20,
                            right: 20,
                            top: 0,
                            height: 1,
                            child: ColoredBox(color: _c.separator),
                          ),
                          Positioned(
                            left: 20,
                            right: 20,
                            top: 1,
                            bottom: 0,
                            child: Align(
                              alignment: Alignment.centerLeft,
                              child: Text(
                                key: const ValueKey('faro actions footer'),
                                items.length <= _actionsVisible
                                    ? l.faroActionsFooter(items.length)
                                    : l.faroActionsFooterMore(
                                        _actionsVisible,
                                        items.length,
                                      ),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: AmalfiType.caption(color: _c.subtitle),
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// The size of the Launcher's box now: grid and list morph into each other.
  Size _launcherBox() {
    final grid = launcherSize(list: false);
    final list = launcherSize(list: true);
    final t = _listMix.value;
    return Size(
      grid.width + (list.width - grid.width) * t,
      grid.height + (list.height - grid.height) * t,
    );
  }

  /// The panel's content: Faro's (bar, results) fading out as the Launcher's fades in, in the
  /// same container. The Launcher lays out at its final size and is cut by the container.
  /// The fade split is an assumption (not measured): the content arrives in the last 60 % of
  /// the morph and leaves in the first 40 %.
  Widget _morphStack(double mix, Widget faro) {
    final faroOpacity = (1 - mix / 0.4).clamp(0.0, 1.0);
    final launcherOpacity = ((mix - 0.4) / 0.6).clamp(0.0, 1.0);
    final size = launcherSize(list: _launcherList);
    return Stack(
      fit: StackFit.expand,
      children: [
        if (faroOpacity > 0)
          ExcludeFocus(
            excluding: _launcher,
            child: IgnorePointer(
              ignoring: _launcher,
              child: Opacity(opacity: faroOpacity, child: faro),
            ),
          ),
        if (_launcher || mix > 0)
          IgnorePointer(
            ignoring: !_launcher,
            child: Opacity(
              opacity: launcherOpacity,
              child: ClipRect(
                child: OverflowBox(
                  alignment: Alignment.topCenter,
                  minWidth: size.width,
                  maxWidth: size.width,
                  minHeight: size.height,
                  maxHeight: size.height,
                  child: Focus(
                    canRequestFocus: false,
                    skipTraversal: true,
                    onKeyEvent: _launcherKey,
                    child: Launcher(
                      entries: widget.entries,
                      recents: _recentIds,
                      list: _launcherList,
                      onListChanged: _setLauncherList,
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
  }

  /// Rows that fit in the actions scope's list (448 pt of 56).
  static const int _actionsVisible =
      (filesHeight - headerHeight - scopeFooterHeight) ~/ rowHeight;

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

  /// What the field says in a scope: its name, or "Search in <folder>" inside a folder.
  String get _scopeLabel => _scope == FaroScope.files && _underName != null
      ? l10nOf(context).faroSearchIn(_underName!)
      : _scopeName(_scope);

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

  /// The Icon Kit glyph of a scope (the round buttons and the field's leading glyph).
  static String _scopeSymbol(FaroScope scope) => switch (scope) {
    FaroScope.all => 'search',
    FaroScope.files => 'folder',
    FaroScope.actions => 'bolt',
    FaroScope.clipboard => 'clipboard',
  };

  Widget _field(FaroAnswer? answer, List<FaroItem> items, int selected) {
    final (completion, preview) = _completion(answer, items, selected);
    // While the reopened query is selected the completion waits for typing.
    final hasSelection =
        _query.selection.isValid && !_query.selection.isCollapsed;
    final shown = hasSelection ? null : completion;
    final fieldStyle = AmalfiType.field(color: _c.text);
    final typed = shown == null
        ? 0.0
        : (TextPainter(
            text: TextSpan(text: _query.text, style: fieldStyle),
            textDirection: TextDirection.ltr,
          )..layout()).width;
    final icon = _query.text.isEmpty ? null : preview;
    final scoped = _scope != FaroScope.all;
    return Focus(
      onKeyEvent: (node, event) => _key(node, event, answer, items),
      child: Stack(
        children: [
          // The magnifier; inside a scope it is the scope's glyph and goes back to all.
          Positioned(
            left: 20,
            top: 16,
            width: 24,
            height: 24,
            child: GestureDetector(
              key: scoped ? const ValueKey('faro scope token') : null,
              behavior: HitTestBehavior.opaque,
              onTap: scoped ? () => _setScope(FaroScope.all) : null,
              child: AmalfiSymbol(
                _scopeSymbol(_scope),
                size: 24,
                color: _c.subtitle,
              ),
            ),
          ),
          Positioned(
            left: 56,
            top: 0,
            bottom: 0,
            right: icon == null ? 20 : fieldIconSide + 2 * fieldIconRight,
            child: Stack(
              alignment: Alignment.centerLeft,
              children: [
                if (_query.text.isEmpty)
                  IgnorePointer(
                    child: Text(
                      // In a scope the placeholder is the scope's name.
                      scoped ? _scopeLabel : _placeholder,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AmalfiType.field(
                        color: scoped ? _c.text : _c.placeholder,
                      ),
                    ),
                  ),
                if (shown != null)
                  Positioned(
                    left: typed - 2,
                    top: 0,
                    bottom: 0,
                    child: IgnorePointer(
                      child: Center(
                        child: Container(
                          height: completionHeight,
                          padding: const EdgeInsets.only(left: 4, right: 1),
                          alignment: Alignment.center,
                          decoration: BoxDecoration(
                            color: _c.completion,
                            borderRadius: BorderRadius.circular(
                              completionRadius,
                            ),
                          ),
                          child: Text(shown, maxLines: 1, style: fieldStyle),
                        ),
                      ),
                    ),
                  ),
                EditableText(
                  controller: _query,
                  focusNode: _focus,
                  style: fieldStyle,
                  cursorColor: _c.text,
                  cursorWidth: 2,
                  cursorHeight: 28,
                  selectionColor: _c.completion,
                  backgroundCursorColor: const Color(0x00000000),
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

  /// The answer card (`Apps/Faro/Calculation`): the expression small on top (Callout,
  /// secondary), the result below (Title 1), the copy button on the right.
  Widget _card(FaroAnswer answer, bool selected) => Container(
    height: cardHeight,
    margin: const EdgeInsets.symmetric(horizontal: cardInset),
    padding: const EdgeInsets.only(left: 20, right: 20),
    decoration: BoxDecoration(
      color: _c.sunken,
      border: Border.all(
        color: selected ? _c.selection : const Color(0x00000000),
        width: cardBorder,
      ),
      borderRadius: BorderRadius.circular(cardRadius),
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
                style: AmalfiType.callout(color: _c.subtitle),
              ),
              const SizedBox(height: 6),
              Text(
                answer.result,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AmalfiType.title1(color: _c.text),
              ),
            ],
          ),
        ),
        AmalfiIconButton(
          semanticLabel: l10nOf(context).faroCopy,
          onPressed: () => _copy(answer),
          icon: const AmalfiSymbol('copy', size: 16),
        ),
      ],
    ),
  );

  /// A 32 pt tile with a 20 pt Icon Kit glyph (`Apps/Faro/Row * Glyph`).
  Widget _glyphTile(String symbol) => DecoratedBox(
    decoration: BoxDecoration(
      color: _c.sunken,
      borderRadius: BorderRadius.circular(8),
    ),
    child: Center(child: AmalfiSymbol(symbol, size: 20, color: _c.subtitle)),
  );

  /// The Icon Kit glyph of a system action.
  static String _actionSymbol(String id) => switch (id) {
    'screenshot.selection' => 'crop',
    final i when i.startsWith('screenshot.') => 'camera',
    'system.dark_toggle' => 'moon',
    'system.mute_toggle' => 'volume-slash',
    'system.volume_up' => 'volume-2',
    'system.volume_down' => 'volume-1',
    'system.brightness_up' => 'sun',
    'system.brightness_down' => 'sun-min',
    final i when i.startsWith('panorama.') => 'window-stack',
    'desktop.show' => 'desktop',
    final i when i.startsWith('space.') => 'square-stack',
    'launcher.toggle' => 'square-grid-2x2',
    'notifications.toggle' => 'bell',
    'controlcenter.toggle' => 'sliders',
    final i when i.startsWith('window.arrange') => 'window-split-3',
    final i when i.startsWith('window.tile') => 'window-split',
    final i when i.startsWith('window.') => 'window',
    'app.force_quit_panel' => 'xmark',
    'session.quit' => 'power',
    _ => 'bolt',
  };

  Widget _row(FaroItem item, bool selected) {
    final l = l10nOf(context);
    final subtitleStyle = AmalfiType.caption(color: _c.subtitle);
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
        _glyphTile(kind == FallbackKind.files ? 'folder' : 'globe'),
        _fallbackLabel(kind),
        null,
      ),
      ActionItem(:final action) => (
        null,
        _glyphTile(_actionSymbol(action.id)),
        _actionLabel(action),
        null,
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
            borderRadius: const BorderRadius.all(Radius.circular(8)),
            child: Image.file(
              File(clip.image!),
              width: rowIconSide,
              height: rowIconSide,
              fit: BoxFit.cover,
            ),
          ),
          _ => _glyphTile(
            clip.kind == ClipKind.image
                ? 'photo'
                : clip.title.startsWith('http')
                ? 'link'
                : 'doc-text',
          ),
        },
        clip.kind == ClipKind.image ? l.faroImage : clip.title,
        Text(l.faroCopied(_when(clip.time)), style: subtitleStyle),
      ),
      ClearClipboardItem() => (
        null,
        _glyphTile('trash'),
        l.faroClearHistory,
        null,
      ),
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
      // The whole row, also where it is empty or only an image icon.
      behavior: HitTestBehavior.opaque,
      onTap: () => _activate(item),
      child: Padding(
        // The selection is inset 8 from the panel's sides.
        padding: const EdgeInsets.symmetric(horizontal: 8),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: selected ? _c.selection : null,
            borderRadius: BorderRadius.circular(16),
          ),
          child: Row(
            children: [
              const SizedBox(width: 12),
              SizedBox(
                width: rowIconSide,
                height: rowIconSide,
                child: switch (icon) {
                  final i? => Center(child: SilicaIcon(i, side: rowIconSide)),
                  null => leading,
                },
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: AmalfiType.bodyEmphasis(color: _c.text),
                    ),
                    ?subtitle,
                  ],
                ),
              ),
              if (shortcut != null) ...[
                const SizedBox(width: 8),
                Text(
                  shortcutGlyphs(shortcut),
                  style: AmalfiType.caption(color: _c.subtitle),
                ),
              ],
              if (selected && item is FolderItem) ...[
                const SizedBox(width: 8),
                Text(
                  l.faroSearchIn(item.folder.name),
                  maxLines: 1,
                  style: AmalfiType.caption(color: _c.subtitle),
                ),
                const SizedBox(width: 8),
                // "Tab" key capsule: 20 tall.
                Container(
                  height: keycapHeight,
                  constraints: const BoxConstraints(minWidth: 40),
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  alignment: Alignment.center,
                  decoration: BoxDecoration(
                    color: _c.key,
                    borderRadius: BorderRadius.circular(keycapRadius),
                  ),
                  child: Text(
                    l.faroTabKey,
                    style: AmalfiType.caption(color: _c.text),
                  ),
                ),
              ],
              // Rows end 20 pt from the panel's edge (the selection ends 8 from it).
              const SizedBox(width: 12),
            ],
          ),
        ),
      ),
    );
    // A file or a folder can be dragged out of Faro, onto the Dock, a window or the
    // desktop: past the drag threshold Faro closes and the drag continues as a system drag.
    return path == null ? row : _draggable(path, row);
  }

  Widget _draggable(String path, Widget child) => Listener(
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
    child: child,
  );

  /// A cell of the files grid: the icon above the name.
  Widget _cell(FaroItem item, bool selected) {
    final (IconSource? icon, String name, String path) = switch (item) {
      FolderItem(:final folder) => (
        fileIcon(Directory(folder.path)),
        folder.name,
        folder.path,
      ),
      FileItem(:final file) => (
        fileIcon(File(file.path)),
        file.name,
        file.path,
      ),
      _ => (null, '', ''),
    };
    return _draggable(
      path,
      GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: () => _activate(item),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: selected ? _c.selection : null,
            borderRadius: const BorderRadius.all(Radius.circular(16)),
          ),
          child: Column(
            children: [
              const SizedBox(height: 8),
              SizedBox(
                width: 56,
                height: 56,
                child: icon == null ? null : SilicaIcon(icon, side: 56),
              ),
              const SizedBox(height: 6),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6),
                child: Text(
                  name,
                  maxLines: 2,
                  textAlign: TextAlign.center,
                  overflow: TextOverflow.ellipsis,
                  style: AmalfiType.callout(color: _c.text),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// The app name over its first action in the actions scope.
  Widget _heading(String title) => SizedBox(
    height: sectionHeight,
    child: Align(
      alignment: Alignment.bottomLeft,
      child: Padding(
        padding: const EdgeInsets.only(left: 20, bottom: 4),
        child: Text(
          title,
          maxLines: 1,
          key: ValueKey('faro section $title'),
          style: AmalfiType.caption(color: _c.subtitle),
        ),
      ),
    ),
  );

  /// Where a new app starts in the actions scope: row index to app name.
  Map<int, String> _headings(List<FaroItem> items) {
    final l = l10nOf(context);
    final result = <int, String>{};
    String? last;
    for (var i = 0; i < items.length; i++) {
      final title = switch (items[i]) {
        ActionItem() => l.faroSectionSystem,
        AppActionItem(:final entry) => entry.name,
        _ => null,
      };
      if (title != null && title != last) result[i] = title;
      last = title;
    }
    return result;
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
      _Pill(
        l.faroScopeFiles,
        // The pill becomes a token in the field; what was typed stays after it.
        onTap: () => _setScope(FaroScope.files, keepQuery: true),
      ),
    ];
  }

  /// The filter row (kit: 40 pt, pills 24 tall from 8 under the header, 8 apart, 20 pt
  /// inset, radius 12, Callout), scrolling sideways and cut by the panel.
  Widget _filterRow(List<_Pill> pills) => SizedBox(
    height: filterHeight,
    child: ListView.separated(
      key: const ValueKey('faro pills'),
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.fromLTRB(20, 8, 20, 8),
      itemCount: pills.length,
      separatorBuilder: (_, _) => const SizedBox(width: 8),
      itemBuilder: (context, i) {
        final pill = pills[i];
        return GestureDetector(
          onTap: pill.onTap,
          child: Container(
            height: 24,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: pill.selected ? _c.selection : _c.pill,
              borderRadius: BorderRadius.circular(12),
            ),
            child: Text(pill.label, style: AmalfiType.callout(color: _c.text)),
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
    final style = AmalfiType.caption(color: _c.subtitle);
    return Row(
      children: [
        Text('$date · ', style: style),
        AmalfiSymbol('folder', size: 12, color: _c.subtitle),
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

  /// A round button: Applications (the Launcher), Files, Actions, Clipboard (Cmd+1..4). A
  /// plain disc: no hover, no pressed state, no tooltip.
  Widget _circleButton(int index) {
    final l = l10nOf(context);
    final (name, onTap) = switch (index) {
      0 => (l.faroScopeApps, _showLauncher),
      1 => (l.faroScopeFiles, () => _setScope(FaroScope.files)),
      2 => (l.faroScopeActions, () => _setScope(FaroScope.actions)),
      _ => (l.faroScopeClipboard, () => _setScope(FaroScope.clipboard)),
    };
    return Semantics(
      button: true,
      label: name,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: onTap,
        child: LiquidGlass(
          radius: circle / 2,
          material: GlassMaterial.thick,
          interactive: false,
          child: Center(
            child: AmalfiSymbol(_buttonSymbol(index), size: 24, color: _c.text),
          ),
        ),
      ),
    );
  }

  /// The glyph of a round button (Applications, Files, Actions, Clipboard).
  static String _buttonSymbol(int index) => switch (index) {
    0 => 'square-grid-2x2',
    1 => 'folder',
    2 => 'bolt',
    _ => 'clipboard',
  };
}

/// The shadow of a glass panel drawn only outside the panel: under the glass, which Ossido
/// composes behind the surface, it must darken nothing (a BoxShadow would). Drawn through an
/// image ([CachedShadow]): it fades with an Opacity ancestor under Impeller.
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
  Widget build(BuildContext context) => CachedShadow(
    cacheKey: Object.hash(radius, blurRadius, offset, color),
    bleed: blurRadius * 3 + offset.distance,
    paintShadow: _OuterShadowPainter(radius, blurRadius, offset, color).paint,
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
