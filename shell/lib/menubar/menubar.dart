// The menu bar, from plan/reference/menubar.md (identical in light and dark): 30 pt tall,
// with no background (the gradient that compresses the lights under it is done by Ossido),
// white text in Inter 13 pt with the baseline at y 19, app name in bold, status area and
// clock in white at 90 %.

import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import 'package:intl/date_symbol_data_local.dart';
import 'package:intl/intl.dart';

import '../dock/dock.dart' show shellQuote;
import '../faro/launcher.dart' show RecentApps;
import '../l10n/l10n.dart';
import '../system/desktop_entries.dart';
import '../system/shell_channel.dart';
import '../system/layer_control.dart';
import '../system/system_status.dart';
import 'desktop_menu.dart';
import 'logo_menu.dart';
import 'menus.dart';
import 'status_icons.dart';
import 'window_menu.dart';

const double barHeight = 30;
const double fontSize = 13;

/// Text baseline: y 19.0.
const double baseline = 19;

/// Logo box: x 10, 34 pt wide; boxes overlap by 1 pt.
const double logoLeft = 10;
const double logoCell = 34;

/// Space around the text of an item: box = text + ~21.5 pt (~10.75 per side).
const double itemPadding = 10.75;

/// Clock: the text ends 20 pt before the right edge; 16 pt between the Control Center and
/// the clock; 7 pt between the icon boxes.
const double clockRightMargin = 20;
const double clockGap = 16;
const double statusGap = 7;

const Color itemColor = Color(0xFFFFFFFF);

/// Open menu (menubar.md): body 5 pt left of the item's box, 1 pt below the bar; the item
/// has a capsule 24 tall, 3.5 pt beyond the box per side (`#7e7f84` over `#6f7075`: white
/// at ~10 %). Moving onto another item the menu changes after ~85 ms.
const double menuLeftOfItem = 5;
const double capsuleHeight = 24;
const double capsuleOutset = 3.5;
const Color capsuleFill = Color(0x1AFFFFFF);
const Duration menuSwitchDelay = Duration(milliseconds: 85);

/// Room below the menu for its shadow, when the surface grows.
const double menuShadowRoom = 40;

/// White at 90 % (status icons and clock: 241 over 112).
const Color statusColor = Color(0xE6FFFFFF);

/// The app menu items until the global menu arrives (M7): generic ids, shown with
/// `menuTitle`.
const List<String> genericMenus = ['file', 'edit', 'view', 'window', 'help'];

/// Date and time as in the bar: "Lun 12 ott  22:15" (double space between date and time).
/// The capsule on the icon of the open Control Center: 46 × 24, just lighter than the bar
/// (`#727478` over `#6f7075`, controlcenter.md).
const Size controlsCapsule = Size(46, 24);
const Color controlsCapsuleFill = Color(0x05FFFFFF);

String clockText(DateTime t, [String locale = 'en']) {
  _dates ??= initializeDateFormatting();
  final date = DateFormat('EEE d MMM', locale).format(t);
  final hh = t.hour.toString().padLeft(2, '0');
  final mm = t.minute.toString().padLeft(2, '0');
  // The weekday starts with a capital, as in the bar ("Mon", "Lun").
  final shown = date.isEmpty ? date : date[0].toUpperCase() + date.substring(1);
  return '$shown  $hh:$mm';
}

/// The date symbols of every intl locale, loaded once (synchronously, from the package).
Future<void>? _dates;

class MenuBar extends StatefulWidget {
  const MenuBar({
    super.key,
    this.entries,
    this.onSearch,
    this.onClock,
    this.onControls,
    this.system,
    this.now,
  });

  /// The installed apps, for the name of the focused app.
  final Map<String, DesktopEntry>? entries;

  /// Click on the Faro icon.
  final VoidCallback? onSearch;

  /// Click on the clock: the Notification Center.
  final VoidCallback? onClock;

  /// Click on the controls icon: the Control Center.
  final VoidCallback? onControls;

  /// System state: Wi-Fi or wired network, battery.
  final SystemSource? system;

  /// Fixed clock (tests).
  final DateTime? now;

  @override
  State<MenuBar> createState() => _MenuBarState();
}

class _MenuBarState extends State<MenuBar> {
  Timer? _timer;

  /// The open menu (`logo`, `app` or the item name) and the item rectangles.
  String? _open;
  final Map<String, GlobalKey> _itemKeys = {};
  final GlobalKey _rootKey = GlobalKey();
  final GlobalKey<MenuHostState> _menuKey = GlobalKey();
  Timer? _switchTimer;
  double _width = 0;

  static const List<String> _menuNames = ['logo', 'app', ...genericMenus];

  GlobalKey _keyFor(String name) => _itemKeys.putIfAbsent(name, GlobalKey.new);

  List<MenuEntry> _entries(String name) => name == 'logo'
      ? logoMenu(
          l: l10nOf(context),
          run: (id) => ShellChannel.instance.execute(id),
          launch: _launch,
          open: (path) =>
              ShellChannel.instance.launch('xdg-open ${shellQuote(path)}'),
          option: _option,
          user: _user,
          settings: _settingsApp,
          recent: _recent,
          clearRecent: () {
            RecentApps().clear();
            RecentItems.clearDocuments();
          },
        )
      : barMenu(
          name,
          _appName,
          (id) => ShellChannel.instance.execute(id),
          texts: l10nOf(context),
        );

  /// ⌥ held: the logo menu shows its alternate items (SH-MB-04).
  bool _option = false;
  RecentItems _recent = const RecentItems();
  late final String _user = userFullName();

  bool _keyChanged(KeyEvent event) {
    final option = HardwareKeyboard.instance.isAltPressed;
    if (option != _option) {
      _option = option;
      if (_open == 'logo' && mounted) setState(() {});
    }
    return false;
  }

  void _launch(DesktopEntry entry) {
    ShellChannel.instance.launch(entry.exec);
    RecentApps().add(entry.id);
  }

  /// The Settings app, if installed.
  DesktopEntry? get _settingsApp {
    for (final e in widget.entries?.values ?? const <DesktopEntry>[]) {
      if (e.id.toLowerCase().endsWith('.settings')) return e;
    }
    return null;
  }

  RecentItems _loadRecent() {
    final entries = widget.entries ?? const <String, DesktopEntry>{};
    return RecentItems(
      apps: [
        for (final id in RecentApps().load())
          if (entries[id] case final e?) e,
      ],
      documents: RecentItems.loadDocuments(),
    );
  }

  /// The rectangle of a bar item (its box), in the surface.
  Rect? _rectOf(String name) {
    final box = _itemKeys[name]?.currentContext?.findRenderObject();
    final root = _rootKey.currentContext?.findRenderObject();
    if (box is! RenderBox || root is! RenderBox || !box.hasSize) return null;
    return box.localToGlobal(Offset.zero, ancestor: root) & box.size;
  }

  String? _itemAt(Offset p) {
    if (p.dy >= barHeight) return null;
    for (final name in _menuNames) {
      if (_rectOf(name)?.contains(p) ?? false) return name;
    }
    return null;
  }

  /// Opens a menu below its item: the body 5 pt left of the box, 1 pt below the bar. The
  /// surface grows down to the menu and its shadow.
  void _openMenu(String name) {
    final entries = _entries(name);
    final rect = _rectOf(name);
    if (entries.isEmpty || rect == null) return;
    if (name == 'logo') _recent = _loadRecent();
    final size = menuSize(entries);
    final anchor = Offset(rect.left - menuLeftOfItem, barHeight + 1);
    _switchTimer?.cancel();
    setState(() => _open = name);
    LayerControl.size(0, anchor.dy + size.height + menuShadowRoom);
    LayerControl.inputRegion([
      Rect.fromLTWH(0, 0, _width, barHeight),
      (anchor & size).inflate(2),
    ]);
    LayerControl.keyboard('exclusive');
  }

  void _closeMenu() {
    _switchTimer?.cancel();
    if (_open == null) return;
    setState(() => _open = null);
    LayerControl.keyboard('none');
    LayerControl.inputRegion(null);
    LayerControl.size(0, barHeight);
  }

  /// → and ←: the neighbouring menu (only those with items).
  void _neighbour(int delta) {
    final withMenu = [
      for (final name in _menuNames)
        if (_entries(name).isNotEmpty) name,
    ];
    final at = withMenu.indexOf(_open ?? '');
    if (at < 0 || withMenu.isEmpty) return;
    _openMenu(withMenu[(at + delta) % withMenu.length]);
  }

  void _pointerDown(PointerDownEvent e) {
    final item = _itemAt(e.localPosition);
    if (item != null && _entries(item).isNotEmpty) {
      // The menu opens on press; on the item already open it closes.
      item == _open ? _menuKey.currentState?.close() : _openMenu(item);
      return;
    }
    if (_open != null) _menuKey.currentState?.pointerDown(e.localPosition);
  }

  void _pointerMoved(Offset p) {
    if (_open == null) return;
    // With a menu open, moving onto another item the menu changes after ~85 ms.
    final item = _itemAt(p);
    if (item != null && item != _open && _entries(item).isNotEmpty) {
      _switchTimer?.cancel();
      _switchTimer = Timer(menuSwitchDelay, () {
        if (_open != null) _openMenu(item);
      });
    } else if (item == null || item == _open) {
      _switchTimer?.cancel();
    }
    _menuKey.currentState?.pointerMoved(p);
  }

  @override
  void initState() {
    super.initState();
    ShellChannel.instance.toplevels.addListener(_changed);
    ShellChannel.instance.focused.addListener(_changed);
    ShellChannel.instance.states.addListener(_changed);
    ShellChannel.instance.states.addListener(_desktopMenuChanged);
    widget.system?.addListener(_changed);
    ShellChannel.instance.refresh();
    HardwareKeyboard.instance.addHandler(_keyChanged);
    _scheduleTick();
    _windowMenus = ShellChannel.instance.windowMenus.listen(
      (menu) => _openWindowMenu(menu.tip),
    );
  }

  /// The arrangement popover of a window (long press on the green button): the tip at the
  /// point given by Ossido. While it is open the surface covers the screen and takes all
  /// clicks: one outside closes it (as in the reference, in the document).
  StreamSubscription<({String id, Offset tip})>? _windowMenus;
  Offset? _windowMenuTip;

  Future<void> _openWindowMenu(Offset tip) async {
    _closeMenu();
    final screen = await LayerControl.outputSize();
    if (!mounted) return;
    setState(() => _windowMenuTip = tip);
    LayerControl.size(0, screen?.height ?? tip.dy + 400);
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
  }

  /// The desktop context menu: the wallpaper announces `desktopmenu` = "x y".
  Offset? _desktopMenuAt;
  String _desktopMenuState = '';

  void _desktopMenuChanged() {
    final value = ShellChannel.instance.states.value['desktopmenu'] ?? '';
    if (value == _desktopMenuState) return;
    _desktopMenuState = value;
    final parts = value.split(' ');
    final x = parts.length == 2 ? double.tryParse(parts[0]) : null;
    final y = parts.length == 2 ? double.tryParse(parts[1]) : null;
    if (x == null || y == null) {
      _closeDesktopMenu(announce: false);
    } else {
      _openDesktopMenu(Offset(x, y));
    }
  }

  Future<void> _openDesktopMenu(Offset at) async {
    _closeMenu();
    _closeWindowMenu();
    final screen = await LayerControl.outputSize();
    if (!mounted) return;
    setState(() => _desktopMenuAt = at);
    LayerControl.size(0, screen?.height ?? at.dy + 400);
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
  }

  void _closeDesktopMenu({bool announce = true}) {
    if (_desktopMenuAt == null) return;
    setState(() => _desktopMenuAt = null);
    if (announce) {
      _desktopMenuState = '';
      ShellChannel.instance.broadcast('desktopmenu', '');
    }
    LayerControl.keyboard('none');
    LayerControl.inputRegion(null);
    LayerControl.size(0, barHeight);
  }

  List<MenuEntry> _desktopEntries() {
    final settings = _settingsApp;
    return desktopMenu(
      l: l10nOf(context),
      onNewFolder: () =>
          newFolder(desktopFolder(), l10nOf(context).desktopUntitledFolder),
      onChangeWallpaper: settings == null ? null : () => _launch(settings),
    );
  }

  void _closeWindowMenu() {
    if (_windowMenuTip == null) return;
    setState(() => _windowMenuTip = null);
    LayerControl.keyboard('none');
    LayerControl.inputRegion(null);
    LayerControl.size(0, barHeight);
  }

  // The next minute, aligned with the clock.
  void _scheduleTick() {
    if (widget.now != null) return;
    final now = DateTime.now();
    final next = DateTime(
      now.year,
      now.month,
      now.day,
      now.hour,
      now.minute + 1,
    );
    _timer = Timer(next.difference(now), () {
      _changed();
      _scheduleTick();
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    _switchTimer?.cancel();
    ShellChannel.instance.toplevels.removeListener(_changed);
    ShellChannel.instance.focused.removeListener(_changed);
    ShellChannel.instance.states.removeListener(_changed);
    ShellChannel.instance.states.removeListener(_desktopMenuChanged);
    widget.system?.removeListener(_changed);
    _windowMenus?.cancel();
    HardwareKeyboard.instance.removeHandler(_keyChanged);
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  String get _appName {
    final window = ShellChannel.instance.focusedToplevel;
    if (window == null) return 'Silica';
    final entry = widget.entries?.values
        .where((e) => e.matchesAppId(window.appId))
        .firstOrNull;
    return entry?.name ??
        (window.appId.isNotEmpty ? window.appId : window.title);
  }

  @override
  Widget build(BuildContext context) {
    final regular = uiText(size: fontSize, color: itemColor);
    final bold = TextStyle(
      fontFamily: uiFontFamily,
      package: 'silica_ui',
      fontSize: fontSize,
      fontWeight: FontWeight.w600,
      color: itemColor,
    );
    final status = uiText(size: fontSize, color: statusColor);
    final open = _open == null ? null : _rectOf(_open!);
    final states = ShellChannel.instance.states.value;
    // Control Center open: 46 × 24 capsule on the icon, just lighter than the bar
    // (controlcenter.md: +3 levels). Notification Center open: the clock with the capsule
    // of the open items (notifications.md, N-03).
    final controls = states['controlcenter.open'] == '1'
        ? _rectOf('controls')
        : null;
    final clock = states['notifications.open'] == '1' ? _rectOf('clock') : null;
    // A status menu open: its icon with the capsule of the open items.
    final statusMenu = (states['statusmenu'] ?? '').startsWith('wifi')
        ? _rectOf('wifi')
        : null;
    final bar = SizedBox(
      height: barHeight,
      child: Stack(
        children: [
          if (controls != null)
            Positioned(
              left: controls.center.dx - controlsCapsule.width / 2,
              top: (barHeight - controlsCapsule.height) / 2,
              width: controlsCapsule.width,
              height: controlsCapsule.height,
              child: const DecoratedBox(
                key: ValueKey('control center capsule'),
                decoration: BoxDecoration(
                  color: controlsCapsuleFill,
                  borderRadius: BorderRadius.all(
                    Radius.circular(capsuleHeight / 2),
                  ),
                ),
              ),
            ),
          if (statusMenu != null)
            Positioned(
              left: statusMenu.left - capsuleOutset,
              top: (barHeight - capsuleHeight) / 2,
              width: statusMenu.width + 2 * capsuleOutset,
              height: capsuleHeight,
              child: const DecoratedBox(
                key: ValueKey('status menu capsule'),
                decoration: BoxDecoration(
                  color: capsuleFill,
                  borderRadius: BorderRadius.all(
                    Radius.circular(capsuleHeight / 2),
                  ),
                ),
              ),
            ),
          if (clock != null)
            Positioned(
              left: clock.left - capsuleOutset,
              top: (barHeight - capsuleHeight) / 2,
              width: clock.width + 2 * capsuleOutset,
              height: capsuleHeight,
              child: const DecoratedBox(
                key: ValueKey('clock capsule'),
                decoration: BoxDecoration(
                  color: capsuleFill,
                  borderRadius: BorderRadius.all(
                    Radius.circular(capsuleHeight / 2),
                  ),
                ),
              ),
            ),
          // The item with the open menu: capsule 24 tall, ~3.5 pt beyond the box per side.
          if (open != null)
            Positioned(
              left: open.left - capsuleOutset,
              top: (barHeight - capsuleHeight) / 2,
              width: open.width + 2 * capsuleOutset,
              height: capsuleHeight,
              child: const DecoratedBox(
                decoration: BoxDecoration(
                  color: capsuleFill,
                  borderRadius: BorderRadius.all(
                    Radius.circular(capsuleHeight / 2),
                  ),
                ),
              ),
            ),
          Positioned(
            left: logoLeft,
            top: 0,
            width: logoCell,
            height: barHeight,
            child: SizedBox(
              key: _keyFor('logo'),
              child: const Center(child: SilicaMark(color: itemColor)),
            ),
          ),
          Positioned(
            left: logoLeft + logoCell - 1,
            top: 0,
            height: barHeight,
            child: Row(
              children: [
                _item(_appName, bold, key: _keyFor('app')),
                for (final menu in genericMenus)
                  _item(
                    menuTitle(l10nOf(context), menu),
                    regular,
                    key: _keyFor(menu),
                  ),
              ],
            ),
          ),
          Positioned(
            right: clockRightMargin,
            top: 0,
            height: barHeight,
            child: Row(
              children: [
                ..._systemIcons(),
                const SizedBox(width: statusGap),
                GestureDetector(
                  onTap: widget.onSearch,
                  child: const StatusIcon(kind: StatusKind.search, cell: 33),
                ),
                const SizedBox(width: statusGap),
                GestureDetector(
                  key: _keyFor('controls'),
                  onTap: widget.onControls,
                  behavior: HitTestBehavior.opaque,
                  child: const StatusIcon(kind: StatusKind.controls, cell: 26),
                ),
                const SizedBox(width: clockGap),
                GestureDetector(
                  key: _keyFor('clock'),
                  onTap: widget.onClock,
                  behavior: HitTestBehavior.opaque,
                  child: _baselined(
                    clockText(
                      widget.now ?? DateTime.now(),
                      l10nOf(context).localeName,
                    ),
                    status,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        _width = constraints.maxWidth;
        return Listener(
          key: _rootKey,
          behavior: HitTestBehavior.translucent,
          onPointerDown: _pointerDown,
          onPointerMove: (e) => _pointerMoved(e.localPosition),
          onPointerHover: (e) => _pointerMoved(e.localPosition),
          onPointerUp: (e) => _menuKey.currentState?.pointerUp(e.localPosition),
          child: SizedBox.expand(
            child: Stack(
              children: [
                Positioned(
                  left: 0,
                  top: 0,
                  right: 0,
                  height: barHeight,
                  child: bar,
                ),
                if (_windowMenuTip case final tip?)
                  Positioned.fill(
                    child: MenuHost(
                      key: ValueKey(tip),
                      entries: windowMenuEntries(
                        (id) => ShellChannel.instance.execute(id),
                        texts: l10nOf(context),
                      ),
                      anchor: Offset.zero,
                      tip: tip,
                      tipUp: true,
                      onDismissed: _closeWindowMenu,
                    ),
                  ),
                if (_desktopMenuAt case final at?)
                  Positioned.fill(
                    child: MenuHost(
                      key: ValueKey(at),
                      entries: _desktopEntries(),
                      anchor: Offset(at.dx, at.dy - desktopMenuAbove),
                      onDismissed: _closeDesktopMenu,
                    ),
                  ),
                if (_open case final name?)
                  Positioned.fill(
                    child: MenuHost(
                      key: _menuKey,
                      entries: _entries(name),
                      anchor: Offset(
                        (open?.left ?? 0) - menuLeftOfItem,
                        barHeight + 1,
                      ),
                      handlesPointer: false,
                      onDismissed: _closeMenu,
                      onNext: () => _neighbour(1),
                      onPrevious: () => _neighbour(-1),
                      // With a submenu open the input region includes it.
                      onRectsChanged: (rects) {
                        final bottom = rects.fold(
                          barHeight,
                          (double b, r) => math.max(b, r.bottom),
                        );
                        LayerControl.size(0, bottom + menuShadowRoom);
                        LayerControl.inputRegion([
                          Rect.fromLTWH(0, 0, _width, barHeight),
                          for (final r in rects) r.inflate(2),
                        ]);
                      },
                    ),
                  ),
              ],
            ),
          ),
        );
      },
    );
  }

  /// Battery (if any) and network, from the system state; without it, the full Wi-Fi icon.
  List<Widget> _systemIcons() {
    final s = widget.system?.value;
    if (s == null) {
      return const [StatusIcon(kind: StatusKind.network, cell: 22)];
    }
    return [
      if (s.batteryPresent) ...[
        StatusIcon(
          kind: StatusKind.battery,
          cell: 33,
          level: s.batteryPercent / 100,
          charging: s.batteryCharging,
        ),
        const SizedBox(width: statusGap),
      ],
      if (s.wired && s.wifiSsid.isEmpty)
        const StatusIcon(kind: StatusKind.wired, cell: 22)
      else if (s.wifiAvailable)
        GestureDetector(
          key: _keyFor('wifi'),
          behavior: HitTestBehavior.opaque,
          onTap: _toggleWifiMenu,
          child: StatusIcon(
            kind: StatusKind.network,
            cell: 22,
            level: s.wifiStrength / 100,
            dim: !s.wifiEnabled || s.wifiSsid.isEmpty,
          ),
        ),
    ];
  }

  /// The Wi-Fi status menu (menubar.md › Status menu): the Wi-Fi panel of the Control Center
  /// below the icon, which draws it when the bar announces `statusmenu`.
  void _toggleWifiMenu() {
    final shell = ShellChannel.instance;
    final rect = _rectOf('wifi');
    if (rect == null) return;
    final open = shell.states.value['statusmenu']?.startsWith('wifi') ?? false;
    shell.broadcast('statusmenu', open ? '' : 'wifi ${rect.right}');
  }

  /// An item: the text with ~10.75 pt per side.
  Widget _item(String text, TextStyle style, {Key? key}) => Padding(
    key: key,
    padding: const EdgeInsets.symmetric(horizontal: itemPadding),
    child: _baselined(text, style),
  );

  /// Text with the baseline at y 19.
  Widget _baselined(String text, TextStyle style) => SizedBox(
    height: barHeight,
    child: Baseline(
      baseline: baseline,
      baselineType: TextBaseline.alphabetic,
      child: Text(text, style: style),
    ),
  );
}

/// The user's full name for the log out item: the first field of the comment in
/// `/etc/passwd`, else the login name.
String userFullName({Map<String, String>? env, String? passwd}) {
  final e = env ?? Platform.environment;
  final login = e['USER'] ?? e['LOGNAME'] ?? '';
  String text;
  try {
    text = passwd ?? File('/etc/passwd').readAsStringSync();
  } on FileSystemException {
    return login;
  }
  for (final line in text.split('\n')) {
    final fields = line.split(':');
    if (fields.length > 4 && fields[0] == login) {
      final name = fields[4].split(',').first.trim();
      return name.isEmpty ? login : name;
    }
  }
  return login;
}
