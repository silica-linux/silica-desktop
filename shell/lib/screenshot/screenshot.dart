// Screenshots (screenshot-ui.md, docs/spec/screenshot-ui.md): the thumbnail after every
// capture, the ⇧⌘4 crosshair (with Space: a window) and the ⇧⌘5 bar with its Options menu.
//
// - Ossido captures (`screenshot.screen`, `screenshot.region`, `screenshot.window_at`; ⇧⌘3
//   and ⌃⇧⌘3 run there directly), writes a temporary PNG and announces it with the broadcast
//   "screenshot" (JSON: path, width, height, clipboard, seq). The shell plays the shutter
//   sound (silica-daemon), then shows the thumbnail or sends the capture to its place.
// - Where captures go, the timer, the thumbnail, the pointer, the bar's mode, selection and
//   position: `settings.dart` ($XDG_CONFIG_HOME/silica/screenshot.json). ⇧⌘3 and ⇧⌘4 follow
//   the same choices.
// - The file is named "Screenshot <date> at <time>.png" (localized) from the capture time,
//   and lands in its place when the thumbnail leaves, or at once when a drag of the
//   thumbnail starts (a real Wayland drag and drop of the file).
// - Clicking the thumbnail opens the image with the default app: there is no Markup editor
//   yet (#154), so the image viewer stands in for it. Mail is not a place yet (#155).

import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../dock/apps.dart' show openPathCommand;
import '../dock/dock.dart' show shellQuote;
import '../l10n/l10n.dart';
import '../menubar/menubar.dart' show barHeight;
import '../system/desktop_entries.dart';
import '../system/layer_arbiter.dart';
import '../system/shell_channel.dart';
import 'files.dart';
import 'selection.dart';
import 'services.dart';
import 'settings.dart';
import 'thumbnail.dart';
import 'toolbar.dart';

export 'files.dart' show screenshotFolder, screenshotName, documentsFolder;
export 'settings.dart';
export 'thumbnail.dart'
    show
        thumbnailImage,
        thumbnailBorder,
        thumbnailMargin,
        thumbnailRadius,
        thumbnailSettle,
        thumbnailHold,
        thumbnailLeave;
export 'toolbar.dart' show toolbarSize, toolbarAboveDock, ShotBarColors;

/// The veil of window mode: `#88bae7` (sRGB) at opacity ~0.51 (K-05).
const Color windowVeil = Color(0x8288BAE7);

/// The countdown of the timer (PROVISIONAL: not measured): a dark rounded square with the
/// seconds left, at the center of what will be captured.
const Size countdownSize = Size(96, 96);

enum _Mode { none, selection, window, toolbar, countdown }

class ScreenshotLayer extends StatefulWidget {
  const ScreenshotLayer({
    super.key,
    this.folder,
    this.documents,
    this.cache,
    this.now,
    this.windowAt,
    this.services,
    this.settingsFile,
    this.entries = const {},
    this.env,
  });

  /// The rectangle of the window under a point (frame without shadow), for the window mode
  /// veil; null or no window: the whole screen is veiled (the desktop). Without it Ossido
  /// draws the veil.
  final Rect? Function(Offset point)? windowAt;

  /// The Desktop and Documents folders (tests); null: the user's.
  final String? folder;
  final String? documents;

  /// Where captures for the clipboard or an app get a file (tests).
  final String? cache;

  /// The capture time (tests).
  final DateTime Function()? now;

  /// The system (tests replace it).
  final ScreenshotServices? services;

  /// The settings file (tests); null: $XDG_CONFIG_HOME/silica/screenshot.json.
  final File? settingsFile;

  /// The installed apps: the image viewer, the file manager.
  final Map<String, DesktopEntry> entries;

  /// The environment for the default image viewer (tests).
  final Map<String, String>? env;

  @override
  State<ScreenshotLayer> createState() => ScreenshotLayerState();
}

/// A capture announced by Ossido, until it reaches its place.
class _Shot {
  _Shot(this.temp, this.size, this.time, this.destination);

  final String temp;
  final Size size;
  final DateTime time;
  final ShotDestination destination;

  /// Where the file is now, once placed (a drag places it early).
  String? placed;

  /// It reached its place, was dropped somewhere or deleted: nothing more to do.
  bool done = false;

  /// Being dragged out: the thumbnail is hidden.
  bool dragging = false;

  /// The image of the drag, decoded when the thumbnail appears.
  ui.Image? dragImage;

  String get file => placed ?? temp;
}

class ScreenshotLayerState extends State<ScreenshotLayer>
    with TickerProviderStateMixin {
  late final ScreenshotServices _services =
      widget.services ?? SystemScreenshotServices(entries: widget.entries);
  late final File _settingsFile =
      widget.settingsFile ?? ScreenshotSettings.file();
  late ScreenshotSettings _settings = ScreenshotSettings.load(_settingsFile);

  StreamSubscription<String>? _commands;
  StreamSubscription<String>? _drags;
  final FocusNode _focus = FocusNode();
  _Mode _mode = _Mode.none;
  Offset? _pointer;

  /// ⇧⌘4: the selection being drawn, and whether this run goes to the clipboard.
  LiveSelection? _live;
  bool _toClipboard = false;

  /// ⇧⌘5: the mode, the selection being adjusted, the bar's center, the Options menu.
  ToolbarChoice _choice = ToolbarChoice.selection;
  Rect? _selection;
  Offset? _barCenter;
  bool _optionsOpen = false;

  /// The menus' items, made when they open (a new list would reset an open menu).
  List<MenuEntry> _optionsEntries = const [];
  List<MenuEntry> _menuEntries = const [];

  /// The timer: seconds left and its ticker.
  int _countdown = 0;
  Rect? _countdownTarget;
  Timer? _ticker;

  /// The thumbnail.
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
  bool _hovered = false;

  /// The thumbnail's menu, open at this point.
  Offset? _menuAt;
  int _lastSeq = -1;
  Size _lastSize = Size.zero;

  /// The app that opens images (the default image viewer), if any.
  late final DesktopEntry? _viewer = _findViewer();

  @override
  void initState() {
    super.initState();
    _commands = ShellChannel.instance.commands.listen((id) {
      switch (id) {
        case 'screenshot.selection':
          _startCrosshair(clipboard: false);
        case 'screenshot.selection_clipboard':
          _startCrosshair(clipboard: true);
        case 'screenshot.toolbar':
          _openToolbar();
      }
    });
    _drags = _services.dragFinished.listen(_dragFinished);
    ShellChannel.instance.states.addListener(_statesChanged);
    _pushSettings();
  }

  @override
  void dispose() {
    LayerArbiter.instance.remove(this);
    _commands?.cancel();
    _drags?.cancel();
    ShellChannel.instance.states.removeListener(_statesChanged);
    _hold?.cancel();
    _ticker?.cancel();
    _settle.dispose();
    _leave.dispose();
    _focus.dispose();
    _shot?.dragImage?.dispose();
    super.dispose();
  }

  // --- Settings -------------------------------------------------------------------------

  DesktopEntry? _findViewer() {
    final entries = widget.entries;
    final id = defaultAppFor(
      'image/png',
      env: widget.env,
      exists: entries.containsKey,
    );
    if (id != null) return entries[id];
    for (final entry in entries.values) {
      if (entry.mimeTypes.contains('image/png') && entry.id != fileManagerId) {
        return entry;
      }
    }
    return null;
  }

  void _changeSettings(void Function(ScreenshotSettings s) change) {
    setState(() => change(_settings));
    _settings.save(_settingsFile);
    _pushSettings();
  }

  /// Ossido draws the pointer into ⇧⌘3 captures itself: it learns the setting.
  void _pushSettings() {
    _services.execute('screenshot.settings', {
      'pointer': _settings.showPointer,
    });
  }

  String get _desktop => widget.folder ?? screenshotFolder(env: widget.env);
  String get _documents => widget.documents ?? documentsFolder(env: widget.env);
  String get _cache => widget.cache ?? screenshotCache(env: widget.env);

  /// The folder of a destination (null: clipboard and apps have none).
  String? _folderOf(ShotDestination to) => switch (to) {
    ToDesktop() => _desktop,
    ToDocuments() => _documents,
    ToFolder(:final path) => Directory(path).existsSync() ? path : _desktop,
    ToClipboard() || ToApp() => null,
  };

  // --- Modes (crosshair, window, bar, timer) --------------------------------------------

  void _startCrosshair({required bool clipboard}) {
    _toClipboard = clipboard;
    _enter(_Mode.selection);
  }

  void _openToolbar() {
    _toClipboard = false;
    _choice = _settings.mode;
    _selection = null;
    _barCenter = _settings.barCenter;
    _enter(_Mode.toolbar);
  }

  void _enter(_Mode mode) {
    _ticker?.cancel();
    setState(() {
      _mode = mode;
      _live = null;
      _optionsOpen = false;
    });
    _syncLayer();
    _syncVeil();
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _cancel() {
    _ticker?.cancel();
    setState(() {
      _mode = _Mode.none;
      _live = null;
      _optionsOpen = false;
    });
    _syncLayer();
    _syncVeil();
  }

  bool get _windowMode =>
      _mode == _Mode.window ||
      (_mode == _Mode.toolbar && _choice == ToolbarChoice.window);

  /// In window mode Ossido veils the frame of the window under the pointer (K-05: it knows
  /// the frames, the shell does not). A shell given `windowAt` draws the veil itself.
  bool _compositorVeil = false;
  bool _veilAsked = false;

  Future<void> _syncVeil() async {
    final on = _windowMode && widget.windowAt == null;
    if (on == _veilAsked) return;
    _veilAsked = on;
    final done = await _services.execute('screenshot.window_veil', {
      'enabled': on,
    });
    if (!mounted) return;
    setState(() => _compositorVeil = on && done);
  }

  /// The input region: everything with a tool or the thumbnail's menu open; the thumbnail
  /// alone otherwise (none while it is dragged); nothing while the timer runs, so the apps
  /// can be used before the capture.
  void _syncLayer() {
    final shot = _shot;
    final thumb = shot == null || shot.dragging
        ? null
        : thumbnailRect(shot.size, _lastSize);
    final LayerWish wish;
    if ((_mode != _Mode.none && _mode != _Mode.countdown) || _menuAt != null) {
      wish = const LayerWish.all();
    } else if (thumb != null && !thumb.isEmpty) {
      wish = LayerWish.rects([thumb]);
    } else {
      wish = const LayerWish.none();
    }
    LayerArbiter.instance.set(this, wish);
  }

  /// Hides the tools, counts down the timer if asked (from the bar), waits for Ossido to
  /// redraw the screen without them, then captures.
  Future<void> _capture(
    String id,
    Map<String, Object?> params, {
    bool timed = false,
    Rect? target,
  }) async {
    final clipboard = _toClipboard;
    final seconds = timed ? _settings.timer : 0;
    if (seconds > 0) {
      _ticker?.cancel();
      setState(() {
        _mode = _Mode.countdown;
        _optionsOpen = false;
        _live = null;
        _countdown = seconds;
        _countdownTarget = target;
      });
      _syncLayer();
      _syncVeil();
      final finished = Completer<bool>();
      _ticker = Timer.periodic(const Duration(seconds: 1), (t) {
        if (!mounted || _mode != _Mode.countdown) {
          t.cancel();
          if (!finished.isCompleted) finished.complete(false);
          return;
        }
        if (_countdown <= 1) {
          t.cancel();
          if (!finished.isCompleted) finished.complete(true);
          return;
        }
        setState(() => _countdown--);
      });
      if (!await finished.future || !mounted) return;
    }
    _cancel();
    await SchedulerBinding.instance.endOfFrame;
    await Future<void>.delayed(const Duration(milliseconds: 60));
    await _services.execute(id, {
      ...params,
      'pointer': _settings.showPointer,
      if (clipboard) 'clipboard': true,
    });
  }

  KeyEventResult _key(FocusNode node, KeyEvent event) {
    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.space && _live != null) {
      // Space with the button held moves the selection.
      if (event is KeyDownEvent) _live!.moving = _pointer;
      if (event is KeyUpEvent) _live!.moving = null;
      return KeyEventResult.handled;
    }
    if (event is! KeyDownEvent) return KeyEventResult.ignored;
    if (key == LogicalKeyboardKey.escape) {
      if (_optionsOpen) {
        setState(() => _optionsOpen = false);
      } else {
        _cancel();
      }
      return KeyEventResult.handled;
    }
    if (key == LogicalKeyboardKey.space &&
        (_mode == _Mode.selection || _mode == _Mode.window)) {
      setState(() {
        _mode = _mode == _Mode.selection ? _Mode.window : _Mode.selection;
      });
      _syncVeil();
      return KeyEventResult.handled;
    }
    if ((key == LogicalKeyboardKey.enter ||
            key == LogicalKeyboardKey.numpadEnter) &&
        _mode == _Mode.toolbar) {
      _action();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  bool get _shift => HardwareKeyboard.instance.isShiftPressed;
  bool get _option => HardwareKeyboard.instance.isAltPressed;

  void _pointerDown(Offset at) {
    _pointer = at;
    if (_mode == _Mode.selection) setState(() => _live = LiveSelection(at));
  }

  void _pointerMove(Offset at) {
    setState(() {
      _pointer = at;
      _live?.update(at, shift: _shift, option: _option);
    });
  }

  void _pointerUp(Offset at) {
    switch (_mode) {
      case _Mode.selection:
        final live = _live;
        if (live == null) return;
        live.update(at, shift: _shift, option: _option);
        final rect = live.rect(option: _option);
        // A click without dragging captures nothing.
        if (rect.width < 4 || rect.height < 4) {
          setState(() => _live = null);
          return;
        }
        _captureRegion(rect);
      case _Mode.window:
        _captureWindow(at);
      case _Mode.toolbar:
      case _Mode.none:
      case _Mode.countdown:
        break;
    }
  }

  void _captureRegion(Rect rect, {bool timed = false}) => _capture(
    'screenshot.region',
    {'x': rect.left, 'y': rect.top, 'width': rect.width, 'height': rect.height},
    timed: timed,
    target: rect,
  );

  /// The window under the point with its shadow (Option: without), or the whole screen
  /// over the desktop.
  void _captureWindow(Offset at, {bool timed = false}) => _capture(
    'screenshot.window_at',
    {'x': at.dx, 'y': at.dy, 'shadow': !_option, 'or_screen': true},
    timed: timed,
  );

  /// The bar's action button (or Enter).
  void _action() {
    switch (_choice) {
      case ToolbarChoice.screen:
        _capture('screenshot.screen', {}, timed: true);
      case ToolbarChoice.window:
        if (_pointer case final p?) _captureWindow(p, timed: true);
      case ToolbarChoice.selection:
        final rect = _selection;
        if (rect == null) return;
        _changeSettings((s) => s.selection = rect);
        _captureRegion(rect, timed: true);
    }
  }

  void _choose(ToolbarChoice choice) {
    setState(() {
      _choice = choice;
      _optionsOpen = false;
    });
    _changeSettings((s) => s.mode = choice);
    _syncVeil();
  }

  // --- The capture arrives ------------------------------------------------------------------

  void _statesChanged() {
    final value = ShellChannel.instance.states.value['screenshot'] ?? '';
    if (value.isEmpty) return;
    final Object? json;
    try {
      json = jsonDecode(value);
    } on FormatException {
      return;
    }
    if (json is! Map) return;
    final path = json['path'], w = json['width'], h = json['height'];
    final seq = json['seq'];
    if (path is! String || w is! num || h is! num || w <= 0 || h <= 0) return;
    if (seq is int) {
      if (seq == _lastSeq) return;
      _lastSeq = seq;
    }
    _services.playSound(captureSound);
    if (json['clipboard'] == true) {
      // ⌃: straight to the clipboard, no thumbnail, no file.
      _toClipboardNow(path);
      return;
    }
    final shot = _Shot(
      path,
      Size(w.toDouble(), h.toDouble()),
      widget.now?.call() ?? DateTime.now(),
      _settings.destination,
    );
    // A thumbnail still showing: its capture goes to its place right away.
    final previous = _shot;
    if (previous != null) _finish(previous);
    _hold?.cancel();
    if (!_settings.showThumbnail) {
      setState(() => _shot = null);
      _syncLayer();
      _finish(shot);
      return;
    }
    setState(() {
      _shot = shot;
      _menuAt = null;
      _hovered = false;
    });
    _leave.value = 0;
    _settle.forward(from: 0);
    _restartHold(thumbnailSettle + thumbnailHold);
    _syncLayer();
    final box = thumbnailRect(shot.size, _lastSize).size;
    if (!box.isEmpty) {
      thumbnailDragImage(path, box).then((image) {
        if (!mounted || shot.done) {
          image?.dispose();
          return;
        }
        shot.dragImage = image;
      });
    }
  }

  Future<void> _toClipboardNow(String path) async {
    await _services.copyImage(path);
    try {
      File(path).deleteSync();
    } on FileSystemException {
      // Already gone.
    }
  }

  void _restartHold(Duration after) {
    _hold?.cancel();
    _hold = Timer(after, _dismiss);
  }

  /// The time is up: the thumbnail slides off to the right, then the capture goes to its
  /// place. The pointer on it or its menu keep it.
  Future<void> _dismiss() async {
    final shot = _shot;
    if (shot == null || shot.dragging || _hovered || _menuAt != null) return;
    await _slideAway(shot);
  }

  Future<void> _slideAway(_Shot shot) async {
    _hold?.cancel();
    await _leave.forward(from: 0);
    if (!mounted || _shot != shot) return;
    _removeThumbnail();
    _finish(shot);
  }

  void _removeThumbnail() {
    final shot = _shot;
    _hold?.cancel();
    setState(() {
      _shot = null;
      _menuAt = null;
      _hovered = false;
    });
    _syncLayer();
    if (shot != null && shot.done) {
      shot.dragImage?.dispose();
      shot.dragImage = null;
    }
  }

  /// Moves the capture into `folder` under its name; the new path (null if it failed).
  String? _place(_Shot shot, String folder) {
    final placed = shot.placed;
    if (placed != null && File(placed).parent.path == folder) return placed;
    try {
      Directory(folder).createSync(recursive: true);
    } on FileSystemException {
      return null;
    }
    final name = screenshotName(
      shot.time,
      (n) => File('$folder/$n').existsSync(),
      texts: mounted ? l10nOf(context) : null,
    );
    final target = '$folder/$name';
    if (!moveFile(shot.file, target)) return null;
    shot.placed = target;
    return target;
  }

  /// Sends the capture to `to` (its own destination by default); then, if asked, opens it
  /// or shows it in the file manager.
  Future<void> _finish(
    _Shot shot, {
    ShotDestination? to,
    bool open = false,
    bool reveal = false,
  }) async {
    if (shot.done) return;
    shot.done = true;
    final destination = to ?? shot.destination;
    String? path;
    switch (destination) {
      case ToClipboard():
        await _services.copyImage(shot.file);
        if (open || reveal) {
          path = _place(shot, _cache);
        } else if (shot.placed == null) {
          try {
            File(shot.temp).deleteSync();
          } on FileSystemException {
            // Already gone.
          }
        }
      case ToApp(:final id):
        path = _place(shot, _cache);
        final app = widget.entries[id] ?? _viewer;
        if (path != null && app != null && !open) {
          _services.launch(
            execWithFiles(app.rawExec ?? app.exec, [path]) ??
                '${app.exec} ${shellQuote(path)}',
          );
        }
      case ToDesktop() || ToDocuments() || ToFolder():
        final folder = _folderOf(destination) ?? _desktop;
        path = _place(shot, folder);
    }
    if (path == null) return;
    if (open) {
      final quoted = shellQuote(path);
      _services.launch('gio open $quoted 2>/dev/null || xdg-open $quoted');
    }
    if (reveal) {
      _services.launch(openPathCommand(widget.entries, File(path).parent.path));
    }
  }

  // --- The thumbnail's gestures -------------------------------------------------------------

  /// A click: the image opens (with the default app: there is no Markup editor yet, #154).
  void _openShot() {
    final shot = _shot;
    if (shot == null) return;
    _removeThumbnail();
    _finish(shot, open: true);
  }

  void _hover(bool on) {
    _hovered = on;
    final shot = _shot;
    if (shot == null || shot.dragging) return;
    if (on) {
      _hold?.cancel();
    } else if (_menuAt == null) {
      // PROVISIONAL: the full time again once the pointer leaves.
      _restartHold(thumbnailHold);
    }
  }

  /// Two fingers to the right: it goes away now, and the capture to its place.
  void _swipe() {
    final shot = _shot;
    if (shot == null || shot.dragging) return;
    _slideAway(shot);
  }

  /// A press that moves: the file is written to its place at once and dragged out as a
  /// real drag and drop, the thumbnail's image on the pointer where it was grabbed.
  Future<void> _dragOut(Offset grab) async {
    final shot = _shot;
    if (shot == null || shot.done || shot.dragging) return;
    _hold?.cancel();
    final folder = _folderOf(shot.destination) ?? _cache;
    final path = _place(shot, folder);
    if (path == null) return;
    setState(() => shot.dragging = true);
    _syncLayer();
    final started = await _services.startDrag(
      path,
      icon: shot.dragImage,
      hotspot: grab,
    );
    if (!mounted || started) return;
    _dragEnded(shot, dropped: false);
  }

  void _dragFinished(String action) {
    final shot = _shot;
    if (shot == null || !shot.dragging) return;
    _dragEnded(shot, dropped: action != 'none');
  }

  void _dragEnded(_Shot shot, {required bool dropped}) {
    if (dropped) {
      // Taken by the target (a folder, the desktop, an app): nothing more to do.
      shot.done = true;
      _removeThumbnail();
      return;
    }
    // Dropped nowhere: the thumbnail comes back, the file already in its place.
    setState(() => shot.dragging = false);
    _syncLayer();
    _restartHold(thumbnailHold);
  }

  void _openMenu(Offset at) {
    if (_shot == null) return;
    _hold?.cancel();
    setState(() {
      _menuAt = at;
      _menuEntries = _thumbnailMenu();
    });
    _syncLayer();
  }

  void _menuClosed() {
    if (_menuAt == null) return;
    setState(() => _menuAt = null);
    _syncLayer();
    if (_shot != null && !_hovered) _restartHold(thumbnailHold);
  }

  /// A choice of the thumbnail's menu: the thumbnail goes away and the capture goes `to`.
  void _menuFinish({
    ShotDestination? to,
    bool reveal = false,
    bool delete = false,
  }) {
    final shot = _shot;
    if (shot == null) return;
    _menuAt = null;
    _removeThumbnail();
    if (delete) {
      shot.done = true;
      try {
        File(shot.file).deleteSync();
      } on FileSystemException {
        // Already gone.
      }
      return;
    }
    _finish(shot, to: to, reveal: reveal);
  }

  List<MenuEntry> _thumbnailMenu() {
    final l = l10nOf(context);
    final shot = _shot;
    final current = shot?.destination ?? _settings.destination;
    final files = widget.entries[fileManagerId];
    String base(String path) => path.split('/').where((p) => p.isNotEmpty).last;
    return [
      MenuAction(
        l.shotSaveToDesktop,
        checked: current is ToDesktop,
        onSelected: () => _menuFinish(to: const ToDesktop()),
      ),
      MenuAction(
        l.shotSaveToDocuments,
        checked: current is ToDocuments,
        onSelected: () => _menuFinish(to: const ToDocuments()),
      ),
      MenuAction(
        l.shotSaveToClipboard,
        checked: current is ToClipboard,
        onSelected: () => _menuFinish(to: const ToClipboard()),
      ),
      for (final folder in _settings.recentFolders)
        MenuAction(
          l.shotSaveToFolder(base(folder)),
          checked: current == ToFolder(folder),
          onSelected: () => _menuFinish(to: ToFolder(folder)),
        ),
      if (_viewer case final viewer?) ...[
        const MenuDivider(),
        MenuAction(
          l.shotOpenWith(viewer.name),
          checked: current == ToApp(viewer.id),
          onSelected: () => _menuFinish(to: ToApp(viewer.id)),
        ),
      ],
      const MenuDivider(),
      if (files != null)
        MenuAction(
          l.shotShowIn(files.name),
          onSelected: () => _menuFinish(reveal: true),
        ),
      MenuAction(l.shotDelete, onSelected: () => _menuFinish(delete: true)),
      const MenuDivider(),
      MenuAction(l.shotClose, onSelected: () => _menuFinish()),
    ];
  }

  // --- The bar's Options menu -------------------------------------------------------------

  Future<void> _chooseOtherFolder() async {
    final l = l10nOf(context);
    // The chooser is a window: the bar steps aside while it is open, then comes back.
    _cancel();
    final current = _folderOf(_settings.destination) ?? _desktop;
    final chosen = await _services.chooseFolder(
      title: l.shotChooseFolder,
      start: current,
    );
    if (!mounted) return;
    if (chosen != null) _changeSettings((s) => s.chooseFolder(chosen));
    _openToolbar();
  }

  List<MenuEntry> _optionsMenu() {
    final l = l10nOf(context);
    final to = _settings.destination;
    String base(String path) => path.split('/').where((p) => p.isNotEmpty).last;
    void destination(ShotDestination d) =>
        _changeSettings((s) => s.destination = d);
    return [
      MenuSection(l.saveTo),
      MenuAction(
        l.shotDesktop,
        checked: to is ToDesktop,
        onSelected: () => destination(const ToDesktop()),
      ),
      MenuAction(
        l.shotDocuments,
        checked: to is ToDocuments,
        onSelected: () => destination(const ToDocuments()),
      ),
      MenuAction(
        l.shotClipboard,
        checked: to is ToClipboard,
        onSelected: () => destination(const ToClipboard()),
      ),
      if (_viewer case final viewer?)
        MenuAction(
          viewer.name,
          checked: to == ToApp(viewer.id),
          onSelected: () => destination(ToApp(viewer.id)),
        ),
      for (final folder in _settings.recentFolders)
        MenuAction(
          base(folder),
          checked: to == ToFolder(folder),
          onSelected: () => _changeSettings((s) => s.chooseFolder(folder)),
        ),
      if (widget.entries.containsKey(fileManagerId))
        MenuAction(l.shotOtherFolder, onSelected: _chooseOtherFolder),
      const MenuDivider(),
      MenuSection(l.delay),
      for (final seconds in timerChoices)
        MenuAction(
          seconds == 0 ? l.delayNone : l.delaySeconds(seconds),
          checked: _settings.timer == seconds,
          onSelected: () => _changeSettings((s) => s.timer = seconds),
        ),
      const MenuDivider(),
      MenuSection(l.options),
      MenuAction(
        l.thumbnailAfterCapture,
        checked: _settings.showThumbnail,
        onSelected: () =>
            _changeSettings((s) => s.showThumbnail = !s.showThumbnail),
      ),
      MenuAction(
        l.shotKeepSelection,
        checked: _settings.rememberSelection,
        onSelected: () =>
            _changeSettings((s) => s.rememberSelection = !s.rememberSelection),
      ),
      MenuAction(
        l.shotIncludePointer,
        checked: _settings.showPointer,
        onSelected: () =>
            _changeSettings((s) => s.showPointer = !s.showPointer),
      ),
    ];
  }

  // --- Drawing ------------------------------------------------------------------------------

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, box) {
      final screen = box.biggest;
      if (screen != _lastSize) {
        _lastSize = screen;
        // The input region depends on the size (the thumbnail's place).
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) _syncLayer();
        });
      }
      final children = <Widget>[];
      if (_shot case final shot? when !shot.dragging) {
        final rect = thumbnailRect(shot.size, screen);
        // It appears in place with a fade (slowing down), then leaves to the right
        // accelerating.
        final settle = Curves.easeOut.transform(_settle.value);
        final dx =
            Curves.easeIn.transform(_leave.value) *
            (thumbnailMargin + rect.width + 30);
        children.add(
          Positioned.fromRect(
            rect: rect.shift(Offset(dx, 0)),
            child: Opacity(
              opacity: settle,
              child: ThumbnailView(
                key: const ValueKey('thumbnail'),
                file: shot.file,
                onOpen: _openShot,
                onMenu: _openMenu,
                onDragOut: _dragOut,
                onSwipe: _swipe,
                onHover: _hover,
              ),
            ),
          ),
        );
      }
      if (_mode == _Mode.countdown) children.add(_countdownView(screen));
      if (_mode != _Mode.none && _mode != _Mode.countdown) {
        children.add(Positioned.fill(child: _overlay(screen)));
      }
      if (_menuAt case final at?) {
        children.add(
          Positioned.fill(
            child: MenuHost(
              key: const ValueKey('thumbnail menu'),
              entries: _menuEntries,
              // At the pointer: the host moves it left when the thumbnail is at the right
              // edge.
              anchor: at,
              upwards: true,
              top: barHeight,
              // Drawn by Flutter: it covers the thumbnail (Ossido's glass would sit behind
              // the whole layer, thumbnail included).
              opaque: true,
              onDismissed: _menuClosed,
            ),
          ),
        );
      }
      return Stack(children: children);
    },
  );

  Widget _countdownView(Size screen) {
    final target = _countdownTarget ?? Offset.zero & screen;
    return Positioned.fromRect(
      rect: Rect.fromCenter(
        center: target.center,
        width: countdownSize.width,
        height: countdownSize.height,
      ),
      child: IgnorePointer(
        child: Container(
          key: const ValueKey('countdown'),
          decoration: BoxDecoration(
            color: const Color(0x99000000),
            borderRadius: BorderRadius.circular(20),
          ),
          alignment: Alignment.center,
          child: Text(
            '$_countdown',
            style: uiText(
              size: 48,
              color: const Color(0xFFFFFFFF),
            ).copyWith(fontWeight: FontWeight.w300),
          ),
        ),
      ),
    );
  }

  Rect _barRect(Size screen) {
    final center =
        _barCenter ??
        Offset(
          screen.width / 2,
          screen.height - toolbarAboveDock - toolbarSize.height / 2,
        );
    final rect = Rect.fromCenter(
      center: center,
      width: toolbarSize.width,
      height: toolbarSize.height,
    );
    // Always whole on the screen.
    final dx = rect.left < 0
        ? -rect.left
        : rect.right > screen.width
        ? screen.width - rect.right
        : 0.0;
    final dy = rect.top < 0
        ? -rect.top
        : rect.bottom > screen.height
        ? screen.height - rect.bottom
        : 0.0;
    return rect.shift(Offset(dx, dy));
  }

  Widget _overlay(Size screen) {
    final toolbar = _mode == _Mode.toolbar;
    final bar = toolbar ? _barRect(screen) : null;
    final overBar = bar != null && _pointer != null && bar.contains(_pointer!);
    final camera = _windowMode || (toolbar && _choice == ToolbarChoice.screen);
    final cursor = overBar || (toolbar && _choice == ToolbarChoice.selection)
        ? MouseCursor.defer
        : SystemMouseCursors.none;
    return Focus(
      focusNode: _focus,
      onKeyEvent: _key,
      child: MouseRegion(
        cursor: cursor,
        onHover: (e) => setState(() => _pointer = e.localPosition),
        child: Listener(
          behavior: HitTestBehavior.opaque,
          onPointerDown: (e) {
            if (!toolbar) _pointerDown(e.localPosition);
          },
          onPointerMove: (e) => _pointerMove(e.localPosition),
          onPointerUp: (e) {
            if (!toolbar) _pointerUp(e.localPosition);
          },
          child: Stack(
            children: [
              // ⇧⌘5, screen or window: a click on the screen captures.
              if (toolbar && _choice != ToolbarChoice.selection)
                Positioned.fill(
                  child: GestureDetector(
                    key: const ValueKey('capture area'),
                    behavior: HitTestBehavior.opaque,
                    onTapUp: (d) => _choice == ToolbarChoice.screen
                        ? _capture('screenshot.screen', {}, timed: true)
                        : _captureWindow(d.localPosition, timed: true),
                  ),
                )
              else
                const Positioned.fill(child: SizedBox.expand()),
              if (toolbar && _choice == ToolbarChoice.selection)
                Positioned.fill(
                  child: SelectionEditor(
                    rect: _selection ??= _initialSelection(screen),
                    onChanged: (r) => setState(() => _selection = r),
                    onCommitted: (r) {
                      if (_settings.rememberSelection) {
                        _changeSettings((s) => s.selection = r);
                      }
                    },
                  ),
                ),
              if (_mode == _Mode.selection)
                if (_live case final live?)
                  Positioned.fromRect(
                    rect: live.rect(option: _option),
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
              if (_pointer case final p?
                  when _windowMode && !_compositorVeil && !overBar)
                Positioned.fromRect(
                  rect: widget.windowAt?.call(p) ?? Offset.zero & screen,
                  child: const IgnorePointer(
                    child: ColoredBox(key: ValueKey('veil'), color: windowVeil),
                  ),
                ),
              if (_pointer case final p? when camera && !overBar)
                Positioned(
                  left: p.dx - 14,
                  top: p.dy - 11,
                  child: const IgnorePointer(
                    child: CustomPaint(
                      key: ValueKey('camera'),
                      size: Size(28, 22),
                      painter: CameraPainter(),
                    ),
                  ),
                ),
              if (bar != null) ..._toolbar(bar),
            ],
          ),
        ),
      ),
    );
  }

  Rect _initialSelection(Size screen) {
    final remembered = _settings.selection;
    if (_settings.rememberSelection && remembered != null) {
      return fitSelection(remembered, screen);
    }
    return defaultSelection(screen);
  }

  /// The crosshair and, at its bottom right, the point (or the size of the selection).
  List<Widget> _crosshair(Offset p) {
    final live = _live;
    final rect = live?.rect(option: _option);
    final text = rect != null && live!.start != live.end
        ? '${rect.width.round()}\n${rect.height.round()}'
        : '${p.dx.round()}\n${p.dy.round()}';
    return [
      Positioned(
        left: p.dx - 10,
        top: p.dy - 10,
        child: const IgnorePointer(
          child: CustomPaint(
            key: ValueKey('crosshair'),
            size: Size(20, 20),
            painter: CrosshairPainter(),
          ),
        ),
      ),
      Positioned(
        left: p.dx + 12,
        top: p.dy + 8,
        child: IgnorePointer(child: CrosshairLabel(text: text)),
      ),
    ];
  }

  List<Widget> _toolbar(Rect bar) {
    final l = l10nOf(context);
    final overBar = _pointer != null && bar.contains(_pointer!);
    return [
      Positioned.fromRect(
        rect: bar,
        child: ShotToolbar(
          choice: _choice,
          colors: ShotBarColors.of(context),
          optionsLabel: l.optionsButton,
          actionLabel: l.capture,
          // In window mode the target is the window under the pointer: none on the bar.
          showAction: !(_choice == ToolbarChoice.window && overBar),
          onChoose: _choose,
          onClose: _cancel,
          onOptions: () => setState(() {
            _optionsOpen = !_optionsOpen;
            if (_optionsOpen) _optionsEntries = _optionsMenu();
          }),
          onAction: _action,
          onMove: (d) =>
              setState(() => _barCenter = _barRect(_lastSize).center + d),
          onMoved: () => _changeSettings((s) => s.barCenter = _barCenter),
        ),
      ),
      // The Options menu opens above the button, with its left edge on the button and its
      // bottom 16 pt below the top of the bar (K-05).
      if (_optionsOpen)
        Positioned.fill(
          child: MenuHost(
            key: const ValueKey('options menu'),
            entries: _optionsEntries,
            anchor: Offset(bar.left + optionsX, bar.top + 16),
            upwards: true,
            top: barHeight,
            onDismissed: () => setState(() => _optionsOpen = false),
          ),
        ),
    ];
  }
}
