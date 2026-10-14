// The shell's Dock: layout and magnification from model.dart (measurements in
// plan/reference/dock.md), lens glass background computed by Ossido, icons, separator,
// indicators and label drawn here.

import 'dart:async';
import 'dart:math' as math;
import 'dart:io'
    show
        Directory,
        File,
        FileSystemEntity,
        FileSystemException,
        Platform,
        Process,
        ProcessException;
import 'dart:ui' as ui;

import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart' show KeyDownEvent, LogicalKeyboardKey;
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import 'package:flutter/gestures.dart' show kSecondaryButton;

import '../l10n/l10n.dart';
import '../system/desktop_entries.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import 'apps.dart';
import 'icons.dart';
import 'stack.dart';
import 'stack_settings.dart';
import 'trash_alert.dart';
import 'model.dart';

/// The Dock with the real apps: installed (.desktop), pinned and open (from Ossido).
class LiveDock extends StatefulWidget {
  const LiveDock({super.key});

  @override
  State<LiveDock> createState() => _LiveDockState();
}

class _LiveDockState extends State<LiveDock> {
  late final Map<String, DesktopEntry> _entries = loadDesktopEntries(
    locale: systemLanguage(),
  );
  late final List<String> _pinned = pinnedIds(_entries);

  /// Dock options (dock.json): magnification and the recent apps section (D-08).
  bool _magnify = dockFlag('magnify', fallback: true);
  bool _recentsOn = dockFlag('recents', fallback: false);

  /// The apps open at the last change and those closed recently, from the most recent.
  Set<String> _running = {};
  final List<String> _closed = [];

  /// Windows from the most recent (focus): a click on the app raises the last one used.
  final List<String> _recent = [];

  /// The "Empty Trash" alert is open.
  bool _confirmEmpty = false;

  /// The thumbnails of minimized windows, requested from Ossido when minimizing.
  final Map<String, ui.Image> _thumbnails = {};

  /// The stack settings, by folder path.
  final Map<String, StackSettings> _stacks = loadStackSettings();

  StackSettings _stackOf(String folder) =>
      _stacks[folder] ?? const StackSettings();

  void _setStack(String folder, StackSettings settings) {
    setState(() => _stacks[folder] = settings);
    saveStackSettings(_stacks);
  }

  void _openPath(String path) =>
      ShellChannel.instance.launch('xdg-open ${shellQuote(path)}');

  /// The settings of a stack as menu items (D-04): "Sort by", "Show as",
  /// "View content as", with the check mark on the choice.
  List<MenuEntry> _stackOptions(String folder) {
    final s = _stackOf(folder);
    final l = l10nOf(context);
    MenuAction pick(String label, bool checked, StackSettings next) =>
        MenuAction(
          label,
          checked: checked,
          onSelected: () => _setStack(folder, next),
        );
    return [
      MenuSection(l.stackSortBy),
      for (final (sort, label) in [
        (StackSort.name, l.sortName),
        (StackSort.added, l.sortDateAdded),
        (StackSort.modified, l.sortDateModified),
        (StackSort.created, l.sortDateCreated),
        (StackSort.kind, l.sortKind),
      ])
        pick(label, s.sort == sort, s.copyWith(sort: sort)),
      const MenuDivider(),
      MenuSection(l.stackShowAs),
      for (final (display, label) in [
        (StackDisplay.folder, l.stackFolder),
        (StackDisplay.stack, l.stackStack),
      ])
        pick(label, s.display == display, s.copyWith(display: display)),
      const MenuDivider(),
      MenuSection(l.stackViewContentAs),
      for (final (content, label) in [
        (StackContent.fan, l.stackFan),
        (StackContent.grid, l.stackGrid),
        (StackContent.list, l.stackList),
        (StackContent.automatic, l.stackAutomatic),
      ])
        pick(label, s.content == content, s.copyWith(content: content)),
    ];
  }

  /// The menu of a folder: the stack settings, Options, Open.
  List<MenuEntry> _folderMenu(Item item, String folder) {
    final l = l10nOf(context);
    return [
      ..._stackOptions(folder),
      const MenuDivider(),
      MenuAction(
        l.options,
        submenu: [
          MenuAction(
            l.showInEnclosingFolder,
            onSelected: () => _openPath(File(folder).parent.path),
          ),
        ],
      ),
      const MenuDivider(),
      MenuAction(l.openNamed(item.label), onSelected: () => _openPath(folder)),
    ];
  }

  /// The List view of a stack: a menu with a small icon and the name of every file, then
  /// Options (the stack settings) and Open (dock-stacks.md › List).
  List<MenuEntry> _stackListMenu(Item item, List<FileSystemEntity> entries) {
    final folder = item.folder;
    if (folder == null) return const [];
    return [
      for (final entry in entries)
        MenuAction(
          fileName(entry.path),
          icon: (color) => SizedBox(
            width: 16,
            height: 16,
            child: switch (fileIcon(entry)) {
              final source? => SilicaIcon(source, side: 16),
              null => null,
            },
          ),
          onSelected: () => _openPath(entry.path),
        ),
      if (entries.isNotEmpty) const MenuDivider(),
      MenuAction(l10nOf(context).options, submenu: _stackOptions(folder)),
      MenuAction(
        l10nOf(context).openNamed(item.label),
        onSelected: () => _openPath(folder),
      ),
    ];
  }

  @override
  void initState() {
    super.initState();
    ShellChannel.instance.toplevels.addListener(_changed);
    ShellChannel.instance.focused.addListener(_focused);
    ShellChannel.instance.minimized.addListener(_minimized);
    ShellChannel.instance.refresh();
  }

  void _minimized() {
    final ids = ShellChannel.instance.minimized.value;
    _thumbnails.removeWhere((id, image) {
      final gone = !ids.contains(id);
      if (gone) image.dispose();
      return gone;
    });
    for (final id in ids) {
      if (_thumbnails.containsKey(id)) continue;
      ShellChannel.instance.thumbnail(id).then((image) {
        if (!mounted || image == null) return;
        if (ShellChannel.instance.minimized.value.contains(id)) {
          setState(() => _thumbnails[id] = image);
        } else {
          image.dispose();
        }
      });
    }
    setState(() {});
  }

  @override
  void dispose() {
    ShellChannel.instance.toplevels.removeListener(_changed);
    ShellChannel.instance.focused.removeListener(_focused);
    ShellChannel.instance.minimized.removeListener(_minimized);
    for (final image in _thumbnails.values) {
      image.dispose();
    }
    super.dispose();
  }

  void _changed() {
    // An app with no more windows joins the recently closed ones.
    final now = {
      for (final t in ShellChannel.instance.toplevels.value)
        if (_entries.values.where((e) => e.matchesAppId(t.appId)).firstOrNull
            case final entry?)
          entry.id,
    };
    for (final id in _running.difference(now)) {
      _closed
        ..remove(id)
        ..insert(0, id);
    }
    _closed.removeWhere(now.contains);
    if (_closed.length > 10) _closed.removeRange(10, _closed.length);
    _running = now;
    setState(() {});
  }

  void _setOption(String key, bool value) {
    setState(() {
      if (key == 'magnify') _magnify = value;
      if (key == 'recents') _recentsOn = value;
    });
    saveDockOption(key, value);
  }

  /// The separator menu (D-04): the options the Dock has. Hide, position and the minimize
  /// effect arrive with those features.
  List<MenuEntry> _separatorMenu() => [
    MenuAction(
      _magnify
          ? l10nOf(context).magnificationOff
          : l10nOf(context).magnificationOn,
      onSelected: () => _setOption('magnify', !_magnify),
    ),
    MenuAction(
      l10nOf(context).showRecentApps,
      checked: _recentsOn,
      onSelected: () => _setOption('recents', !_recentsOn),
    ),
  ];

  void _focused() {
    final id = ShellChannel.instance.focused.value;
    if (id.isEmpty) return;
    _recent
      ..remove(id)
      ..insert(0, id);
  }

  /// The window of the app used last.
  String _latest(Item item) {
    for (final id in _recent) {
      if (item.windows.contains(id)) return id;
    }
    return item.windows.first;
  }

  void _activate(Item item) {
    // A minimized window comes back itself; an app with all windows minimized brings back
    // the last minimized (D-03); an open app comes to the front; otherwise it launches.
    final minimized = ShellChannel.instance.minimized.value;
    if (item.action case final action?) {
      ShellChannel.instance.execute(action);
    } else if (item.minimizedWindow case final window?) {
      ShellChannel.instance.activate(window);
    } else if (item.windows.isNotEmpty &&
        item.windows.every(minimized.contains)) {
      ShellChannel.instance.activate(
        minimized.lastWhere(item.windows.contains),
      );
    } else if (item.windows.isNotEmpty) {
      ShellChannel.instance.activate(_latest(item));
    } else if (item.command case final command?) {
      ShellChannel.instance.launch(command);
    }
  }

  /// Files dropped from another app (dock.md D-05, MU-40): on the Trash they end up there, on a
  /// stack they move into the folder, on an app they open with it.
  Future<void> _dropFiles(Item item, List<String> paths) async {
    if (item.trash) {
      await moveToTrash(paths);
    } else if (item.folder case final folder?) {
      try {
        await Process.run('mv', ['--', ...paths, folder]);
      } on ProcessException {
        // No mv: the files stay where they are.
      }
    } else if (item.openFiles case final exec?) {
      final command = execWithFiles(exec, paths);
      if (command != null) ShellChannel.instance.launch(command);
    }
    if (mounted) setState(() {});
  }

  void _setPinned(Item item, bool pinned) {
    final id = item.id;
    if (id == null) return;
    setState(() {
      _pinned.remove(id);
      if (pinned) _pinned.add(id);
    });
    savePinned(_pinned);
  }

  /// The menu of an icon (dock.md, D-04): the open windows, Options, then the actions.
  List<MenuEntry> _menu(Item item) {
    final shell = ShellChannel.instance;
    final l = l10nOf(context);
    final toplevels = shell.toplevels.value;
    if (item.minimizedWindow case final window?) {
      return [
        MenuAction(
          l.openNamed(item.label),
          onSelected: () => shell.activate(window),
        ),
      ];
    }
    if (item.kind == ItemKind.separator) return _separatorMenu();
    if (item.folder case final folder?) return _folderMenu(item, folder);
    if (item.action case final action?) {
      return [
        MenuAction(
          l.options,
          submenu: [
            MenuAction(
              l.removeFromDock,
              onSelected: () => _setPinned(item, false),
            ),
          ],
        ),
        const MenuDivider(),
        MenuAction(l.open, onSelected: () => shell.execute(action)),
      ];
    }
    if (item.fixed) return _fileManagerMenu(item);
    // Trash: Open; when full also Empty (D-07).
    if (item.id == null) {
      return [
        if (item.command case final command?)
          MenuAction(l.open, onSelected: () => shell.launch(command)),
        if (item.trash &&
            trashHasFiles(Platform.environment['HOME'] ?? '')) ...[
          const MenuDivider(),
          MenuAction(
            l.emptyTrashEllipsis,
            onSelected: () => setState(() => _confirmEmpty = true),
          ),
        ],
      ];
    }
    return [
      for (final id in item.windows)
        MenuAction(
          toplevels.where((t) => t.id == id).firstOrNull?.title ?? item.label,
          onSelected: () => shell.activate(id),
        ),
      if (item.windows.isNotEmpty) const MenuDivider(),
      MenuAction(
        l.options,
        submenu: [
          if (item.pinned)
            MenuAction(
              l.removeFromDock,
              onSelected: () => _setPinned(item, false),
            )
          else
            MenuAction(l.keepInDock, onSelected: () => _setPinned(item, true)),
        ],
      ),
      const MenuDivider(),
      if (item.running) ...[
        MenuAction(
          l.showAllWindows,
          onSelected: () {
            shell.activate(_latest(item));
            shell.execute('panorama.app');
          },
        ),
        MenuAction(
          l.quit,
          onSelected: () {
            shell.activate(_latest(item));
            shell.execute('app.quit');
          },
        ),
      ] else
        MenuAction(l.open, onSelected: () => _activate(item)),
    ];
  }

  /// The file manager's menu (#141): its windows, then New Window; it can be neither
  /// removed nor quit. Provisional: the reference's file manager menu is not measured.
  List<MenuEntry> _fileManagerMenu(Item item) {
    final shell = ShellChannel.instance;
    final l = l10nOf(context);
    final toplevels = shell.toplevels.value;
    return [
      for (final id in item.windows)
        MenuAction(
          toplevels.where((t) => t.id == id).firstOrNull?.title ?? item.label,
          onSelected: () => shell.activate(id),
        ),
      if (item.windows.isNotEmpty) const MenuDivider(),
      if (item.command case final command?)
        MenuAction(l.newWindow, onSelected: () => shell.launch(command)),
      if (item.running)
        MenuAction(
          l.showAllWindows,
          onSelected: () {
            shell.activate(_latest(item));
            shell.execute('panorama.app');
          },
        ),
    ];
  }

  @override
  Widget build(BuildContext context) => Dock(
    items: dockItems(
      _entries,
      _pinned,
      ShellChannel.instance.toplevels.value,
      minimized: ShellChannel.instance.minimized.value,
      thumbnails: _thumbnails,
      recents: _recentsOn ? _closed : null,
      trashLabel: l10nOf(context).trash,
      appsLabel: l10nOf(context).dockApps,
    ),
    magnification: _magnify ? defaultMagnification : 1,
    onActivate: _activate,
    onDropFiles: _dropFiles,
    menuFor: _menu,
    onOpenPath: _openPath,
    stackSettingsFor: _stackOf,
    stackListMenu: _stackListMenu,
    alert: _confirmEmpty
        ? TrashAlert(
            onEmpty: () async {
              setState(() => _confirmEmpty = false);
              await emptyTrash();
              if (mounted) setState(() {});
            },
            onCancel: () => setState(() => _confirmEmpty = false),
          )
        : null,
    onReorder: (order, dropped) {
      // The pinned ones in the new order; an open app dropped among them becomes pinned.
      final ids = [
        for (final item in order)
          if (item.id case final id?
              when item.pinned || identical(item, dropped))
            id,
      ];
      setState(() {
        _pinned
          ..clear()
          ..addAll(ids);
      });
      savePinned(_pinned);
    },
  );
}

/// Colors of the light appearance (dock.md).
const Color separatorColor = Color(0xFF484950);
const Color indicatorColor = Color(0xFF03040B);
const Color labelBody = Color(0xFFBDBEC3);

/// Icon menu (D-04): it appears after ~575 ms of pressing (550–600 measured), it drags
/// beyond 5 pt; the tip touches ~11 pt above the bottom of the Dock.
const Duration longPressMenu = Duration(milliseconds: 575);
const double dragThreshold = 5;
const double menuTipAboveDock = 11;

/// Room above the menu for its shadow when the surface grows.
const double menuShadowRoom = 30;

/// The Dock input region: the background, plus what magnification and hysteresis need.
const double dockInputMargin = 60;
const double dockInputAbove = 60;

/// Dragging (D-05): the empty slot, the neighbours sliding (~265 ms, S-shaped), "Remove"
/// at ~130 pt above the grab point, fade of the removed item (~250 ms). The pointer counts
/// as "in the Dock" up to 20 pt above the bottom.
const Duration gapSlide = Duration(milliseconds: 265);
const double removeAbove = 130;
const Duration removeFade = Duration(milliseconds: 250);
const double dragInsideAbove = 20;

/// The empty slot where the dragged icon would fall.
const Item gapItem = Item.app('');

/// A drag: the item taken, the grab point in the icon, where the pointer is.
class _DockDrag {
  _DockDrag({
    required this.index,
    required this.item,
    required this.grab,
    required this.side,
    required this.downY,
    required this.pointer,
  });

  final int index;
  final Item item;
  final Offset grab;
  final double side;
  final double downY;
  Offset pointer;
  bool started = false;
  bool inside = true;
  bool remove = false;
}

/// The removed item fading out: icon, position, side and start.
class _Ghost {
  _Ghost(this.item, this.at, this.side);
  final Item item;
  final Offset at;
  final double side;
  Duration elapsed = Duration.zero;
}

/// An argument for `sh -c`, in single quotes.
String shellQuote(String text) => "'${text.replaceAll("'", r"'\''")}'";

/// Minimized windows in the Dock (D-08): 57 × 47 thumbnail in the 63 cell, an 18 pt app icon
/// sticking out 4 pt below the thumbnail.
const double minimizedThumbWidth = 57;
const double minimizedThumbHeight = 47;
const double minimizedBadge = 18;
const double minimizedBadgeBelow = 4;

/// Pressed icon: darkened (estimate, the value is not measured).
const Color pressedShade = Color(0x40000000);
const Color labelBodyTop = Color(0xFFD7D8DE);
const Color labelBodyBottom = Color(0xFFC1C2C8);
const Color labelBorder = Color(0xFF3C3D41);
const Color labelText = Color(0xFF1C1C1D);

/// The Dock colors drawn by the shell, light and dark (dock.md: separator `#a9aaaf`,
/// indicator `#e5e6eb`, label `#7f8086` with the first row `#8e8f96` and text `#f9f9f9`;
/// the `#3c3d41` border is the same). The bottom of the label background in dark is
/// *estimated* equal to the body.
@immutable
class DockColors {
  const DockColors({
    required this.separator,
    required this.indicator,
    required this.labelTop,
    required this.labelBody,
    required this.labelBottom,
    required this.labelBorder,
    required this.labelText,
  });

  final Color separator;
  final Color indicator;
  final Color labelTop;
  final Color labelBody;
  final Color labelBottom;
  final Color labelBorder;
  final Color labelText;

  static const DockColors light = DockColors(
    separator: Color(0xFF484950),
    indicator: Color(0xFF03040B),
    labelTop: Color(0xFFD7D8DE),
    labelBody: Color(0xFFBDBEC3),
    labelBottom: Color(0xFFC1C2C8),
    labelBorder: Color(0xFF3C3D41),
    labelText: Color(0xFF1C1C1D),
  );

  static const DockColors dark = DockColors(
    separator: Color(0xFFA9AAAF),
    indicator: Color(0xFFE5E6EB),
    labelTop: Color(0xFF8E8F96),
    labelBody: Color(0xFF7F8086),
    labelBottom: Color(0xFF7F8086),
    labelBorder: Color(0xFF3C3D41),
    labelText: Color(0xFFF9F9F9),
  );

  static DockColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

/// Label: body 27 pt tall (y 1325–1351), tip drawn from TIP-01 (`labelTipOutline`; the
/// layout keeps the earlier 3 pt for the box), with its top
/// ~13 pt above the icon (icon from 1367, tip at 1354), horizontal margin ~15.5 pt, text
/// of ~14 pt (from the 10 pt capitals in Inter).
const double labelHeight = 27;
const double labelTip = 3;
const double labelGap = 13;
const double labelPadding = 15.5;
const double labelFontSize = 14;

class Dock extends StatefulWidget {
  const Dock({
    super.key,
    required this.items,
    this.magnification = defaultMagnification,
    this.onActivate,
    this.onDropFiles,
    this.menuFor,
    this.onReorder,
    this.alert,
    this.onOpenPath,
    this.stackSettingsFor,
    this.stackListMenu,
  });

  final List<Item> items;
  final double magnification;

  /// Click on an app.
  final void Function(Item item)? onActivate;

  /// Files dropped from another app on an item that accepts them ([Item.acceptsFiles]).
  final void Function(Item item, List<String> paths)? onDropFiles;

  /// A modal alert above the screen (Svuota il Cestino), [trashAlertSize] large.
  final Widget? alert;

  /// A file of the open stack, or the whole folder ("Open folder").
  final ValueChanged<String>? onOpenPath;

  /// The menu of an item (right click, long press); null or empty: no menu.
  final List<MenuEntry> Function(Item item)? menuFor;

  /// The stack settings of a folder (order, icon, view); null: the defaults.
  final StackSettings Function(String folder)? stackSettingsFor;

  /// The menu of a stack's List view: the files, Options, Open (dock-stacks.md › List).
  final List<MenuEntry> Function(Item item, List<FileSystemEntity> entries)?
  stackListMenu;

  /// Dragging (D-05): the new order of the apps left of the separator and the item dropped
  /// (null if it was removed with "Remove", and then it is missing from the order).
  /// Null: the icons do not drag.
  final void Function(List<Item> order, Item? dropped)? onReorder;

  @override
  State<Dock> createState() => _DockState();
}

class _DockState extends State<Dock> with TickerProviderStateMixin {
  late final Ticker _ticker = createTicker(_tick);

  /// The label: the item it belongs to and its opacity.
  int? _labelIndex;
  late final AnimationController _labelAnim = AnimationController(vsync: this)
    ..addListener(() => setState(() {}));
  Timer? _labelTimer;

  /// The open menu: the item, the entries, whether it came from a long press.
  int? _menuIndex;
  List<MenuEntry> _menuEntries = const [];

  /// The open submenus (outside the main menu rectangle), for the input region.
  List<Rect> _submenuRects = const [];
  bool _menuByLongPress = false;
  final GlobalKey<MenuHostState> _menuKey = GlobalKey();
  Timer? _longPress;
  Offset? _downAt;
  int? _downIndex;

  /// The drag in progress and the removed icon fading out in place.
  _DockDrag? _drag;
  _Ghost? _ghost;
  DateTime? _settleUntil;
  late final Ticker _ghostTicker = createTicker((elapsed) {
    final ghost = _ghost;
    if (ghost == null || elapsed >= removeFade) {
      _ghostTicker.stop();
      setState(() => _ghost = null);
    } else {
      setState(() => ghost.elapsed = elapsed);
    }
  });

  /// The open stack (D-06): the item, the files, the view and the animation (0 closed, 1 open).
  int? _stackIndex;
  List<FileSystemEntity> _stackEntries = const [];
  StackView _stackView = StackView.fan;
  bool _stackClosing = false;
  late final AnimationController _stackAnim = AnimationController(
    vsync: this,
    duration: stackOpen,
  )..addListener(() => setState(() {}));
  final FocusNode _stackFocus = FocusNode();

  /// The starting height of the surface and the screen (to center the alert).
  double? _baseHeight;
  Size? _screen;

  /// Where the open stack is in the surface, for the input region and clicks.
  Rect? _stackRect;

  /// What was last asked of the layer (input region, height, keyboard).
  String _layerState = '';

  /// The pressed app (darkened icon) and those bouncing, with their start time.
  int? _pressed;
  final Map<String, DateTime> _bounces = {};
  late final Ticker _bounceTicker = createTicker((_) {
    _endBounces();
    setState(() {});
  });
  double? _pointerX;
  double _amount = 0;
  bool _inside = false;
  // Entrance or exit animation: where it starts and when.
  double _from = 0;
  Duration? _start;

  /// A drag of files from another app: the cell under it, darkened as when pressed.
  StreamSubscription<SilicaDragEvent>? _fileDrags;
  int? _dropIndex;

  void _fileDrag(SilicaDragEvent event) {
    final box = context.findRenderObject();
    if (!mounted || box is! RenderBox || !box.hasSize) return;
    final size = box.size;
    switch (event) {
      case SilicaDragOver(:final position):
        final local = box.globalToLocal(position);
        _hover(local, size);
        final current = layout(
          widget.items,
          width: size.width,
          height: size.height,
          pointerX: _pointerX,
          amount: _amount,
          magnification: widget.magnification,
        );
        final index = _inside ? itemAt(current, widget.items, local.dx) : null;
        final target = index != null && widget.items[index].acceptsFiles
            ? index
            : null;
        if (target != _dropIndex) {
          setState(() {
            if (_pressed == _dropIndex) _pressed = target;
            _dropIndex = target;
          });
        }
      case SilicaDragLeave():
        _endFileDrag();
      case SilicaDrop(:final paths):
        final index = _dropIndex;
        _endFileDrag();
        if (index != null && index < widget.items.length && paths.isNotEmpty) {
          widget.onDropFiles?.call(widget.items[index], paths);
        }
      case SilicaDragFinished():
        break;
    }
  }

  void _endFileDrag() {
    setState(() {
      if (_pressed == _dropIndex) _pressed = null;
      _dropIndex = null;
    });
    _setInside(false);
    _showLabel(null);
  }

  @override
  void initState() {
    super.initState();
    _fileDrags = SilicaDrag.events.listen(_fileDrag);
    // The screen is needed by the first menu already (the surface covers it to catch the click outside).
    LayerControl.outputSize().then((size) {
      if (mounted && size != null && _screen == null) {
        setState(() => _screen = size);
      }
    });
  }

  @override
  void dispose() {
    _fileDrags?.cancel();
    _ticker.dispose();
    _bounceTicker.dispose();
    _labelAnim.dispose();
    _labelTimer?.cancel();
    _longPress?.cancel();
    _ghostTicker.dispose();
    _stackAnim.dispose();
    _stackFocus.dispose();
    super.dispose();
  }

  /// The last item left of the separator (where one can drag).
  int get _leftEnd {
    final separator = widget.items.indexWhere(
      (i) => i.kind == ItemKind.separator,
    );
    return separator < 0 ? widget.items.length : separator;
  }

  /// The items to lay out: while dragging without the one taken and, if the pointer is in
  /// the Dock, with an empty slot where it would fall.
  List<Item> _displayItems(Size size) {
    final drag = _drag;
    if (drag == null || !drag.started) return widget.items;
    final rest = [...widget.items]..removeAt(drag.index);
    if (!drag.inside || drag.remove) return rest;
    final slot = _dropSlot(rest, size, drag.pointer.dx);
    return [...rest]..insert(slot, gapItem);
  }

  /// Where the icon would fall: the first item on the left whose center is past the pointer.
  int _dropSlot(List<Item> rest, Size size, double x) {
    final l = layout(rest, width: size.width, height: size.height);
    final end = rest.indexWhere((i) => i.kind == ItemKind.separator);
    final limit = end < 0 ? rest.length : end;
    // Nothing goes before the fixed items (the file manager, #141).
    final first = rest.takeWhile((i) => i.fixed).length;
    for (var i = first; i < limit; i++) {
      if (x < l.center(i)) return i;
    }
    return limit;
  }

  void _startDrag(Offset p, Size size) {
    final index = _downIndex;
    if (index == null || index >= _leftEnd || widget.onReorder == null) return;
    final item = widget.items[index];
    if (item.kind != ItemKind.app || item.fixed) return;
    final l = layout(
      widget.items,
      width: size.width,
      height: size.height,
      pointerX: _pointerX,
      amount: _amount,
      magnification: widget.magnification,
    );
    final (x, y, side) = iconRect(l, index, size.height);
    _longPress?.cancel();
    _longPress = null;
    _showLabel(null);
    setState(() {
      _pressed = null;
      _drag = _DockDrag(
        index: index,
        item: item,
        grab: p - Offset(x, y),
        side: side,
        downY: _downAt?.dy ?? p.dy,
        pointer: p,
      )..started = true;
    });
  }

  void _moveDrag(Offset p, Size size) {
    final drag = _drag;
    if (drag == null) return;
    final rest = layout(widget.items, width: size.width, height: size.height);
    setState(() {
      drag.pointer = p;
      drag.inside =
          inActiveZone(rest, p.dx, p.dy, size.height) ||
          p.dy >= size.height - bottomGap - backgroundHeight - dragInsideAbove;
      // "Remove" appears 125–135 pt above the grab point.
      drag.remove = p.dy < drag.downY - removeAbove;
    });
  }

  void _endDrag(Size size) {
    final drag = _drag;
    if (drag == null) return;
    final rest = [...widget.items]..removeAt(drag.index);
    final left = rest.indexWhere((i) => i.kind == ItemKind.separator);
    final leftItems = rest.sublist(0, left < 0 ? rest.length : left);
    if (drag.remove) {
      // Removed: the icon and the capsule fade out in place (~250 ms), with no other effects.
      _ghost = _Ghost(drag.item, drag.pointer - drag.grab, drag.side);
      _ghostTicker.start();
      widget.onReorder?.call(leftItems, null);
    } else if (drag.inside) {
      final slot = _dropSlot(rest, size, drag.pointer.dx);
      final order = [...leftItems]
        ..insert(slot.clamp(0, leftItems.length), drag.item);
      widget.onReorder?.call(order, drag.item);
    }
    setState(() {
      _drag = null;
      _settleUntil = DateTime.now().add(gapSlide);
    });
  }

  /// Opens the menu of an item (D-04): the icon stays pressed, the label disappears.
  void _openMenu(int index, {required bool longPress}) {
    final entries = widget.menuFor?.call(widget.items[index]) ?? const [];
    if (entries.isEmpty) return;
    setState(() {
      _menuIndex = index;
      _menuEntries = entries;
      _menuByLongPress = longPress;
      _pressed = index;
    });
    _showLabel(null);
  }

  void _closeMenu() {
    if (!mounted) return;
    setState(() {
      _menuIndex = null;
      _menuEntries = const [];
      _submenuRects = const [];
      _pressed = null;
    });
  }

  bool _menuContains(Offset p) => _menuKey.currentState?.contains(p) ?? false;

  void _pointerDown(PointerDownEvent e, Layout l, Size size) {
    final p = e.localPosition;
    // With the stack open a click outside it and its icon closes it.
    if (_stackIndex != null &&
        !(_stackRect?.contains(p) ?? false) &&
        itemAt(l, widget.items, p.dx) != _stackIndex) {
      _closeStack();
      return;
    }
    if (_menuIndex != null) {
      if (_menuContains(p)) {
        _menuKey.currentState?.pointerDown(p);
      } else {
        _menuKey.currentState?.close();
      }
      return;
    }
    final rest = layout(widget.items, width: size.width, height: size.height);
    final inZone = inActiveZone(rest, p.dx, p.dy, size.height) || _inside;
    final index = inZone ? itemAt(l, widget.items, p.dx) : null;
    // The separator has its menu (right click), but is neither pressed nor dragged.
    if (index == null && inZone && e.buttons & kSecondaryButton != 0) {
      final cell = cellAt(l, p.dx);
      if (cell != null && widget.items[cell].kind == ItemKind.separator) {
        _openMenu(cell, longPress: false);
      }
      return;
    }
    if (index == null) return;
    if (e.buttons & kSecondaryButton != 0) {
      _openMenu(index, longPress: false);
      return;
    }
    setState(() => _pressed = index);
    _downAt = p;
    _downIndex = index;
    _longPress?.cancel();
    _longPress = Timer(longPressMenu, () {
      _longPress = null;
      _openMenu(index, longPress: true);
    });
  }

  void _pointerMove(Offset p, Size size) {
    if (_drag != null) {
      _moveDrag(p, size);
      return;
    }
    if (_longPress != null &&
        _downAt != null &&
        (p - _downAt!).distance > dragThreshold) {
      // Beyond 5 pt the press becomes a drag.
      _startDrag(p, size);
      _longPress?.cancel();
      _longPress = null;
    }
    if (_menuIndex != null) _menuKey.currentState?.pointerMoved(p);
  }

  void _pointerUp(PointerUpEvent e, Layout l, Size size) {
    final p = e.localPosition;
    if (_drag != null) {
      _endDrag(size);
      return;
    }
    if (_longPress != null) {
      // A click: the item under the pointer, if it is the same as at the press.
      _longPress?.cancel();
      _longPress = null;
      final index = itemAt(l, widget.items, p.dx);
      setState(() => _pressed = null);
      if (index != null && index == _downIndex) _activate(index);
      return;
    }
    if (_menuIndex != null && _menuByLongPress) {
      // After the long press: releasing on an entry chooses it, on the icon closes.
      _menuByLongPress = false;
      if (_menuContains(p)) {
        _menuKey.currentState?.pointerUp(p);
      } else {
        _menuKey.currentState?.close();
      }
    }
  }

  /// Input region, surface height and keyboard: only when they change.
  void _syncLayer(Rect dock, Rect? menu, Rect? alert, Size size) {
    final stack = _stackIndex != null;
    // The open stack takes a surface as tall as the screen, like the alert.
    alert ??= stack ? _stackRect ?? Rect.zero : null;
    _baseHeight ??= size.height;
    final base = _baseHeight ?? size.height;
    final screen = _screen?.height;
    // With a menu or the stack the surface covers the screen and takes the whole pointer: a
    // click outside (on the desktop, on a window) closes them instead of going through.
    final catchAll = (menu != null || stack) && screen != null;
    // With the alert, the whole screen; outside Silica, with a menu up to the top of the menu;
    // without, back to the starting height.
    final height = alert != null || catchAll
        ? screen ?? size.height
        : menu != null
        ? (size.height - menu.top + menuShadowRoom).clamp(size.height, 4096.0)
        : base;
    final submenus = menu == null ? const <Rect>[] : _submenuRects;
    final key = '$dock $menu $submenus $alert $height $catchAll';
    if (key == _layerState) return;
    _layerState = key;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (height != size.height) LayerControl.size(0, height);
      LayerControl.inputRegion(
        catchAll
            ? [Offset.zero & Size(size.width, height)]
            : [
                dock,
                if (menu != null) menu.inflate(4),
                for (final r in submenus) r.inflate(4),
                if (alert != null) alert,
              ],
      );
      LayerControl.keyboard(
        menu == null && alert == null ? 'none' : 'exclusive',
      );
    });
  }

  static String _key(Item item) => '${item.label}\u0000${item.command}';

  /// A bounce in progress lands when the app has a window, or after the limit.
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

  double _bounce(int index) => _bounceOf(widget.items[index]);

  double _bounceOf(Item item) {
    final start = _bounces[_key(item)];
    if (start == null) return 0;
    return bounceHeight(DateTime.now().difference(start).inMicroseconds / 1000);
  }

  /// The first files of a folder shown as a stack, for the icon: read again only when the
  /// folder changes (modification time) or the order changes.
  final Map<String, (DateTime, StackSort, List<IconSource>)> _piles = {};

  List<IconSource>? _pileOf(Item item) {
    final folder = item.folder;
    if (folder == null) return null;
    final settings =
        widget.stackSettingsFor?.call(folder) ?? const StackSettings();
    if (settings.display != StackDisplay.stack) return null;
    DateTime changed;
    try {
      changed = Directory(folder).statSync().modified;
    } on FileSystemException {
      return null;
    }
    final cached = _piles[folder];
    if (cached != null && cached.$1 == changed && cached.$2 == settings.sort) {
      return cached.$3;
    }
    final sources = [
      for (final entry in stackEntries(folder, sort: settings.sort).take(3))
        ?fileIcon(entry),
    ];
    _piles[folder] = (changed, settings.sort, sources);
    return sources;
  }

  void _openStack(int index) {
    final item = widget.items[index];
    final folder = item.folder;
    if (folder == null) return;
    final settings =
        widget.stackSettingsFor?.call(folder) ?? const StackSettings();
    final entries = stackEntries(folder, sort: settings.sort);
    // List view: a menu with the files; the icon stays the folder, pressed.
    if (settings.content == StackContent.list) {
      final menu = widget.stackListMenu?.call(item, entries) ?? const [];
      if (menu.isEmpty) return;
      setState(() {
        _menuIndex = index;
        _menuEntries = menu;
        _menuByLongPress = false;
        _pressed = index;
      });
      _showLabel(null);
      return;
    }
    setState(() {
      _stackIndex = index;
      _stackEntries = entries;
      _stackView = switch (settings.content) {
        StackContent.fan => StackView.fan,
        StackContent.grid => StackView.grid,
        _ => automaticView(entries.length),
      };
      _stackClosing = false;
    });
    _showLabel(null);
    _stackAnim.duration = stackOpen;
    _stackAnim.forward(from: 0);
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => _stackFocus.requestFocus(),
    );
  }

  Future<void> _closeStack() async {
    if (_stackIndex == null || _stackClosing) return;
    _stackClosing = true;
    _stackAnim.duration = _stackView == StackView.fan ? fanClose : gridClose;
    await _stackAnim.reverse();
    if (mounted) {
      setState(() {
        _stackIndex = null;
        _stackClosing = false;
      });
    }
  }

  /// A file of the stack: the stack closes at once, then the file opens.
  void _openFromStack(String path) {
    _closeStack();
    widget.onOpenPath?.call(path);
  }

  void _activate(int index) {
    final item = widget.items[index];
    if (item.folder != null && widget.onOpenPath != null) {
      _stackIndex == index ? _closeStack() : _openStack(index);
      return;
    }
    if (!item.running && item.command != null) {
      _bounces.putIfAbsent(_key(item), DateTime.now);
      if (!_bounceTicker.isActive) _bounceTicker.start();
    }
    widget.onActivate?.call(item);
  }

  /// The label follows the cell under the pointer: immediately from one icon to the next.
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
    // Entering uses the fixed threshold; exiting only by leaving the magnified cell (hysteresis).
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
      // Magnification follows the pointer without inertia; on exit the last x stays.
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
        final items = _displayItems(size);
        final dragging = _drag != null;
        final l = layout(
          items,
          width: size.width,
          height: size.height,
          pointerX: _pointerX,
          amount: _amount,
          magnification: widget.magnification,
        );
        final (bx, by, bw, bh) = l.background;
        final label = _labelIndex;
        final hovered =
            !dragging && label != null && label < widget.items.length
            ? label
            : null;
        // During and right after the drag the icons slide into place (~265 ms).
        final settle = _settleUntil;
        final sliding =
            dragging || (settle != null && DateTime.now().isBefore(settle));
        if (sliding && !dragging) {
          WidgetsBinding.instance.scheduleFrame();
        }
        // The menu tip: above the icon, ~11 pt above the bottom of the Dock (D-04).
        final menuIndex = _menuIndex;
        final tip = menuIndex == null || menuIndex >= l.cells.length
            ? null
            : Offset(l.center(menuIndex), by - menuTipAboveDock);
        final menuRect = tip == null
            ? null
            : () {
                final m = menuSize(_menuEntries);
                return Rect.fromLTWH(
                  tip.dx - MenuMetrics.tipBodyLeft,
                  tip.dy - MenuMetrics.tipHeight - m.height,
                  m.width,
                  m.height + MenuMetrics.tipHeight,
                );
              }();
        // Outside the Dock (and the menu) the pointer goes to the windows below.
        final dockRect = Rect.fromLTRB(
          bx - dockInputMargin,
          by - dockInputAbove,
          bx + bw + dockInputMargin,
          size.height,
        );
        // The alert (D-07): centered horizontally, the top at ~20 % of the screen, when
        // the surface is as tall as the screen.
        final screen = _screen;
        final alert = widget.alert;
        if (alert != null && screen == null) {
          LayerControl.outputSize().then((size) {
            if (mounted)
              setState(() => _screen = size ?? const Size(1280, 800));
          });
        }
        final alertRect = alert == null || screen == null
            ? null
            : Rect.fromLTWH(
                (size.width - trashAlertSize.width) / 2,
                screen.height * trashAlertTop - (screen.height - size.height),
                trashAlertSize.width,
                trashAlertSize.height,
              );
        // The stack: it needs the whole screen above the Dock.
        final stackIndex = _stackIndex;
        if (stackIndex != null && screen == null) {
          LayerControl.outputSize().then((value) {
            if (mounted)
              setState(() => _screen = value ?? const Size(1280, 800));
          });
        }
        final zoneTop = by - activeZoneAbove;
        Widget? stackWidget;
        if (stackIndex != null && stackIndex < l.cells.length) {
          final iconX = l.center(stackIndex);
          final item = widget.items[stackIndex];
          final progress = Curves.easeOut.transform(_stackAnim.value);
          if (_stackView == StackView.fan) {
            final count = math.min(_stackEntries.length, fanMax) + 1;
            final top = fanSlot(count - 1);
            _stackRect = Rect.fromLTRB(
              iconX - 420,
              zoneTop + top.center.dy - top.side,
              iconX + fanShift.last + 120,
              zoneTop,
            );
            stackWidget = Positioned.fill(
              child: FanStack(
                entries: _stackEntries,
                anchor: Offset(iconX, zoneTop),
                progress: progress,
                onOpen: (e) => _openFromStack(e.path),
                onOpenFolder: () => _openFromStack(item.folder ?? ''),
              ),
            );
          } else {
            final panel = gridPanelSize(_stackEntries.length, zoneTop - 40);
            final double left = (iconX - panel.width / 2)
                .clamp(8.0, math.max(8.0, size.width - panel.width - 8))
                .toDouble();
            final rect = Rect.fromLTWH(
              left,
              by - gridTipAboveDock - gridTipHeight - panel.height,
              panel.width,
              panel.height,
            );
            _stackRect = rect;
            stackWidget = GridStack(
              title: item.label,
              entries: _stackEntries,
              rect: rect,
              tipX: iconX - left,
              progress: progress,
              onOpen: (e) => _openFromStack(e.path),
              onOpenFolder: () => _openFromStack(item.folder ?? ''),
            );
          }
        } else {
          _stackRect = null;
        }
        _syncLayer(dockRect, menuRect, alertRect, size);
        return Listener(
          behavior: HitTestBehavior.translucent,
          onPointerDown: (e) => _pointerDown(e, l, size),
          onPointerMove: (e) => _pointerMove(e.localPosition, size),
          onPointerHover: (e) {
            if (_menuIndex != null)
              _menuKey.currentState?.pointerMoved(e.localPosition);
          },
          onPointerUp: (e) => _pointerUp(e, l, size),
          onPointerCancel: (_) {
            if (_drag != null) setState(() => _drag = null);
            _longPress?.cancel();
            _longPress = null;
            if (_menuIndex == null) setState(() => _pressed = null);
          },
          child: MouseRegion(
            onHover: (event) => _hover(event.localPosition, size),
            onExit: (_) {
              _setInside(false);
              _showLabel(null);
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
                    painter: _DockPainter(
                      items,
                      l,
                      size.height,
                      DockColors.of(context),
                    ),
                  ),
                ),
                for (var i = 0; i < items.length; i++)
                  if (items[i].minimizedWindow != null)
                    _windowCell(
                      l,
                      i,
                      iconRect(l, i, size.height).$1,
                      iconRect(l, i, size.height).$2,
                      iconRect(l, i, size.height).$3,
                      items[i].icon,
                      items[i],
                    )
                  else if (items[i].icon case final source?)
                    _icon(l, i, size.height, source, items[i], sliding),
                if (_drag case final drag?) ..._dragged(drag),
                if (_ghost case final ghost?) _faded(ghost),
                if (hovered != null && _menuIndex == null)
                  _label(
                    l,
                    hovered,
                    size.height,
                    _labelAnim.value.clamp(0.0, 1.0),
                  ),
                if (stackWidget != null &&
                    (_screen == null || size.height >= _screen!.height - 1))
                  Focus(
                    focusNode: _stackFocus,
                    onKeyEvent: (node, event) {
                      if (event is KeyDownEvent &&
                          event.logicalKey == LogicalKeyboardKey.escape) {
                        _closeStack();
                        return KeyEventResult.handled;
                      }
                      return KeyEventResult.ignored;
                    },
                    child: Stack(
                      clipBehavior: Clip.none,
                      children: [stackWidget],
                    ),
                  ),
                if (alertRect != null && alert != null && alertRect.top >= 0)
                  Positioned.fromRect(rect: alertRect, child: alert),
                if (tip != null)
                  Positioned.fill(
                    child: MenuHost(
                      key: _menuKey,
                      entries: _menuEntries,
                      anchor: Offset.zero,
                      tip: tip,
                      handlesPointer: false,
                      onDismissed: _closeMenu,
                      onRectsChanged: (rects) => setState(
                        () => _submenuRects = rects.skip(1).toList(),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        );
      },
    );
  }

  /// The dragged icon: full size, opaque, hooked to the grab point; above it, the "Remove"
  /// capsule when it is high enough.
  List<Widget> _dragged(_DockDrag drag) {
    final source = drag.item.icon;
    final at = drag.pointer - drag.grab;
    return [
      if (source != null)
        Positioned(
          left: at.dx,
          top: at.dy,
          width: drag.side,
          height: drag.side,
          child: SilicaIcon(source, side: drag.side, besideApps: true),
        ),
      if (drag.remove)
        _capsule(
          l10nOf(context).dockRemove,
          at.dx + drag.side / 2,
          at.dy - labelGap,
          1,
        ),
    ];
  }

  /// The removed item fades out in place with its capsule.
  Widget _faded(_Ghost ghost) {
    final t = ghost.elapsed.inMicroseconds / removeFade.inMicroseconds;
    final opacity = (1 - t).clamp(0.0, 1.0);
    final source = ghost.item.icon;
    return Positioned.fill(
      child: IgnorePointer(
        child: Opacity(
          opacity: opacity,
          child: Stack(
            children: [
              if (source != null)
                Positioned(
                  left: ghost.at.dx,
                  top: ghost.at.dy,
                  width: ghost.side,
                  height: ghost.side,
                  child: SilicaIcon(source, side: ghost.side, besideApps: true),
                ),
              _capsule(
                l10nOf(context).dockRemove,
                ghost.at.dx + ghost.side / 2,
                ghost.at.dy - labelGap,
                1,
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// A capsule like the label, with its tip at (`centerX`, `tipY`).
  Widget _capsule(String text, double centerX, double tipY, double opacity) {
    final style = uiText(
      size: labelFontSize,
      color: DockColors.of(context).labelText,
    );
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      textDirection: TextDirection.ltr,
    )..layout();
    final width = painter.width + 2 * labelPadding;
    return Positioned(
      left: centerX - width / 2,
      top: tipY - labelTip - labelHeight,
      width: width,
      height: labelHeight + labelTip,
      child: Opacity(
        opacity: opacity,
        child: CustomPaint(
          painter: _LabelPainter(DockColors.of(context)),
          child: Padding(
            padding: const EdgeInsets.only(bottom: labelTip),
            child: Center(child: Text(text, style: style)),
          ),
        ),
      ),
    );
  }

  /// A minimized window (D-08): the 57 × 47 thumbnail centered in the cell, flush with the
  /// top of the icon, with the 18 pt app icon in the bottom right corner, 4 pt lower than
  /// the thumbnail.
  Widget _windowCell(
    Layout l,
    int index,
    double x,
    double y,
    double side,
    IconSource? source,
    Item item,
  ) {
    final k = side / iconSize;
    final thumbWidth = minimizedThumbWidth * k;
    final thumbHeight = minimizedThumbHeight * k;
    final badge = minimizedBadge * k;
    final left = l.center(index) - thumbWidth / 2;
    final image = item.thumbnail;
    return Positioned(
      key: ValueKey(_key(item)),
      left: left,
      top: y,
      width: thumbWidth,
      height: thumbHeight + minimizedBadgeBelow * k,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          Positioned(
            left: 0,
            top: 0,
            width: thumbWidth,
            height: thumbHeight,
            child: image == null
                ? const DecoratedBox(
                    decoration: BoxDecoration(color: Color(0x33FFFFFF)),
                  )
                : RawImage(image: image, fit: BoxFit.contain),
          ),
          if (source != null)
            Positioned(
              left: thumbWidth - badge + 2 * k,
              top: thumbHeight - badge + minimizedBadgeBelow * k,
              width: badge,
              height: badge,
              child: SilicaIcon(source, side: badge),
            ),
        ],
      ),
    );
  }

  /// The icon of an app: layered from silica-icons, or from the system theme.
  Widget _icon(
    Layout l,
    int index,
    double height,
    IconSource source,
    Item item,
    bool sliding,
  ) {
    final (x, y, side) = iconRect(l, index, height);
    // With the stack open the folder icon becomes the button that closes it (D-06).
    final closer = _stackIndex == index ? findSilicaIcon('stack-close') : null;
    final pile = closer == null ? _pileOf(item) : null;
    final icon = closer != null
        // The close button follows the appearance (dock-stacks.md, dark: translucent black
        // over the Dock, white chevron); app icons do not.
        ? SilicaIcon(
            LayeredSource(closer),
            side: side,
            besideApps: true,
            dark:
                MediaQuery.maybePlatformBrightnessOf(context) ==
                Brightness.dark,
          )
        : pile != null && pile.isNotEmpty
        ? PileIcon(sources: pile, side: side)
        : SilicaIcon(source, side: side, besideApps: true);
    return AnimatedPositioned(
      key: ValueKey(_key(item)),
      duration: sliding ? gapSlide : Duration.zero,
      curve: Curves.easeInOut,
      left: x,
      top: y - _bounceOf(item),
      width: side,
      height: side,
      // Pressed: the icon darkens (D-02).
      child: !sliding && _pressed == index
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
    // During the bounce the label rises with the icon.
    final iconTopY = iconTop - _bounce(index);
    final tipY = iconTopY - labelGap;
    final style = uiText(
      size: labelFontSize,
      color: DockColors.of(context).labelText,
    );
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
          painter: _LabelPainter(DockColors.of(context)),
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
  _DockPainter(this.items, this.l, this.height, this.colors);

  final List<Item> items;
  final Layout l;
  final double height;
  final DockColors colors;

  @override
  void paint(Canvas canvas, Size size) {
    final (_, by, _, bh) = l.background;
    for (var i = 0; i < items.length; i++) {
      final item = items[i];
      if (item.kind == ItemKind.separator) {
        final x = l.center(i).roundToDouble() - 0.5;
        canvas.drawRect(
          Rect.fromLTWH(x, by + (bh - separatorHeight) / 2, 1, separatorHeight),
          Paint()..color = colors.separator,
        );
        continue;
      }
      if (identical(item, gapItem) || item.minimizedWindow != null) continue;
      final (ix, iy, side) = iconRect(l, i, height);
      if (item.icon == null) {
        paintIcon(canvas, Rect.fromLTWH(ix, iy, side, side), item.label, i);
      }
      if (item.running) {
        canvas.drawCircle(
          Offset(l.center(i), by + bh - indicatorAbove),
          indicatorSize / 2,
          Paint()..color = colors.indicator,
        );
      }
    }
  }

  @override
  bool shouldRepaint(_DockPainter old) => true;
}

/// The Dock label's tip (TIP-01), origin on the apex, pointing down, in pt.
Path labelTipOutline() => Path()
  ..moveTo(-9.361, -6.516)
  ..arcToPoint(const Offset(-7.461, -5.838), radius: const Radius.circular(3))
  ..lineTo(-0.583, -0.208)
  ..arcToPoint(
    const Offset(0.583, -0.208),
    radius: const Radius.circular(0.92),
    clockwise: false,
  )
  ..lineTo(7.461, -5.838)
  ..arcToPoint(const Offset(9.361, -6.516), radius: const Radius.circular(3))
  ..close();

class _LabelPainter extends CustomPainter {
  const _LabelPainter(this.colors);
  final DockColors colors;

  @override
  void paint(Canvas canvas, Size size) {
    final body = RRect.fromRectAndRadius(
      Rect.fromLTWH(0.5, 0.5, size.width - 1, labelHeight - 1),
      const Radius.circular(labelHeight / 2),
    );
    // The measured outline (TIP-01, Dock label): concave fillets of 3 pt into the body,
    // straight flanks at 39°, a 0.92 pt round apex, 6.5 pt below the body's edge. It hangs
    // below the painter's box (layout keeps the earlier 3 pt), like the reference.
    final edge = labelHeight - 0.5;
    final tip = labelTipOutline().shift(Offset(size.width / 2, edge + 6.516));
    final shape = Path.combine(
      PathOperation.union,
      Path()..addRRect(body),
      tip,
    );
    canvas.drawPath(
      shape,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          // The first light row (#d7d8de) and the one below still almost the same (215 in the
          // recording), then the body.
          colors: [
            colors.labelTop,
            colors.labelTop,
            colors.labelBody,
            colors.labelBottom,
          ],
          stops: const [0, 0.07, 0.16, 1],
        ).createShader(Offset.zero & size),
    );
    canvas.drawPath(
      shape,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1
        ..color = colors.labelBorder,
    );
  }

  @override
  bool shouldRepaint(_LabelPainter old) => old.colors != colors;
}
