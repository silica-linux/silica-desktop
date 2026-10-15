// The menu bar, from plan/reference/menubar.md (identical in light and dark): 30 pt tall,
// with no background (the gradient that compresses the lights under it is done by Ossido),
// white text in Inter 13 pt with the baseline at y 19, app name in bold, status area and
// clock in white at 90 %.

import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import 'package:intl/date_symbol_data_local.dart';
import 'package:intl/intl.dart';

import '../dock/dock.dart' show shellQuote;
import '../faro/launcher.dart' show RecentApps;
import '../l10n/l10n.dart';
import '../system/desktop_entries.dart';
import '../system/open_app.dart';
import '../system/app_menu_model.dart';
import '../system/shell_channel.dart';
import '../system/layer_control.dart';
import '../system/media.dart';
import '../system/system_status.dart';
import 'app_menus.dart';
import 'desktop_menu.dart';
import 'logo_menu.dart';
import 'menubar_config.dart';
import 'power.dart';
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

/// The menus of an app that published none (silica-menu-v1, #25): generic ids, shown with
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

/// The rules and owner of what the bar has open (INPUT-01): the app menus belong to the
/// frontmost app, the logo menu to the system, the desktop menu to the desktop's owner;
/// the battery menu is a status menu (a system panel) and the arrangement popover a
/// popover of the frontmost window.
({TransientPolicy policy, String owner}) menuBarTransient({
  String? open,
  bool windowMenu = false,
  bool desktopMenu = false,
  String frontmost = '',
}) {
  if (open == 'logo') return (policy: TransientPolicy.menu, owner: systemOwner);
  if (open == 'battery') {
    return (policy: TransientPolicy.panel, owner: systemOwner);
  }
  if (open != null) return (policy: TransientPolicy.menu, owner: frontmost);
  if (windowMenu) return (policy: TransientPolicy.popover, owner: frontmost);
  if (desktopMenu) return (policy: TransientPolicy.menu, owner: desktopOwner);
  return (policy: TransientPolicy.menu, owner: systemOwner);
}

class MenuBar extends StatefulWidget {
  const MenuBar({
    super.key,
    this.entries,
    this.onSearch,
    this.onClock,
    this.onControls,
    this.system,
    this.now,
    this.media,
    this.config,
  });

  /// Which optional status items to show (Settings › Menu Bar); null: the file, watched.
  final ValueListenable<MenubarConfig>? config;

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

  /// The media players, for the Now Playing item.
  final MediaPlayers? media;

  /// Fixed clock (tests).
  final DateTime? now;

  @override
  State<MenuBar> createState() => _MenuBarState();
}

class _MenuBarState extends State<MenuBar> {
  Timer? _timer;

  /// The open menu (`logo`, `app` or the item name) and the item rectangles.
  String? _open;

  /// The entries of the open menu, taken when it opens: a rebuild while it is open (the
  /// frontmost app changing under a click that passes, the clock) must not hand the menu a
  /// new list, which restarts it and stops its closing fade halfway (#149).
  List<MenuEntry>? _openEntries;
  final Map<String, GlobalKey> _itemKeys = {};
  final GlobalKey _rootKey = GlobalKey();
  final GlobalKey<MenuHostState> _menuKey = GlobalKey();
  Timer? _switchTimer;

  /// The bar items with a menu: the logo, the app menu, then the app's own menus
  /// (`menu0`, `menu1`… from its model) or the generic ones.
  List<String> get _menuNames {
    final own = _appMenus();
    return [
      'logo',
      'app',
      if (own == null)
        ...genericMenus
      else
        for (var i = 1; i < own.length; i++) 'menu$i',
    ];
  }

  /// The frontmost app's own menus (the app menu first), null if it published none. Built
  /// again only when the model, the windows, the focus or the language change.
  List<BarMenu>? _appMenus() {
    final shell = ShellChannel.instance;
    final state = shell.appMenu.value;
    final key = (
      state,
      shell.toplevels.value,
      shell.focused.value,
      l10nOf(context).localeName,
      shell.states.value['app.frontmost'],
    );
    if (key == _appMenusKey) return _appMenusCache;
    _appMenusKey = key;
    return _appMenusCache = _composeAppMenus(state);
  }

  Object? _appMenusKey;
  List<BarMenu>? _appMenusCache;

  List<BarMenu>? _composeAppMenus(AppMenuState state) {
    final shell = ShellChannel.instance;
    final menus = state.menus;
    if (!state.hasMenus || menus == null) return null;
    final focused = shell.focusedToplevel;
    final appId = focused?.appId ?? state.appId;
    return composeAppMenus(
      menus,
      appName: _appNameFromEntries,
      texts: l10nOf(context),
      windows: [
        for (final t in shell.toplevels.value)
          if (t.appId == appId && appId.isNotEmpty) t,
      ],
      focusedWindow: shell.focused.value,
      actions: AppMenuActions(
        activate: (id) => shell.activateMenuItem(state.serial, id),
        run: (id) => shell.execute(id),
        activateWindow: (id) => shell.activate(id),
        openUrl: (url) => shell.launch('xdg-open ${shellQuote(url)}'),
        // Ossido hides apps (app.hide, app.hide_others, app.show_all).
        systemHide: true,
      ),
    );
  }

  GlobalKey _keyFor(String name) => _itemKeys.putIfAbsent(name, GlobalKey.new);

  List<MenuEntry> _entries(String name) {
    final own = name == 'app' || name.startsWith('menu') ? _appMenus() : null;
    if (own != null) {
      final index = name == 'app' ? 0 : int.tryParse(name.substring(4)) ?? -1;
      return index >= 0 && index < own.length ? own[index].entries : const [];
    }
    return _systemEntries(name);
  }

  List<MenuEntry> _systemEntries(String name) => name == 'battery'
      ? _batteryMenu()
      : name == 'logo'
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
          power: (action, {required now}) => _requestPower(action, now: now),
          powerEnabled: powerAvailable(),
        )
      : barMenu(
          name,
          _appName,
          (id) => ShellChannel.instance.execute(id),
          texts: l10nOf(context),
          openUrl: (url) =>
              ShellChannel.instance.launch('xdg-open ${shellQuote(url)}'),
        );

  /// The battery menu: charge, power source, Battery Settings (not measured on the
  /// reference, which has no battery: provisional).
  List<MenuEntry> _batteryMenu() {
    final l = l10nOf(context);
    final s = widget.system?.value;
    final settings = _settingsApp;
    return [
      MenuSection(l.batteryMenuTitle),
      MenuAction(
        l.batteryMenuCharge((s?.batteryPercent ?? 0).round()),
        enabled: false,
      ),
      MenuAction(
        (s?.batteryCharging ?? false)
            ? l.batteryMenuPowerAdapter
            : l.batteryMenuOnBattery,
        enabled: false,
      ),
      const MenuDivider(),
      MenuAction(
        l.batteryMenuSettings,
        enabled: settings != null,
        onSelected: settings == null ? null : () => _launch(settings),
      ),
    ];
  }

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
    openApp(entry);
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
    setState(() {
      _open = name;
      _openEntries = entries;
    });
    // The surface grows to the screen, but takes the pointer only on the bar and the menus
    // (`_menuRegion`, from the menu's rectangles): the windows below keep their hover, and a
    // press outside arrives from Ossido as transientDismissed (#149).
    LayerControl.size(0, anchor.dy + size.height + menuShadowRoom);
    LayerControl.inputRegion([_barRect]);
    LayerControl.keyboard('exclusive');
    _coverScreen();
  }

  static const Rect _barRect = Rect.fromLTWH(0, 0, 16384, barHeight);

  /// A menu of this surface is open with these rectangles (submenus included): the input
  /// region is the bar plus the menus, and Ossido tells about a press outside. Without that
  /// support (an older Ossido) the surface takes every click, as before.
  Future<void> _menuRegion(List<Rect> rects) async {
    // Already closed (a late report of a menu that went away): no transient again.
    if (_open == null && _windowMenuTip == null && _desktopMenuAt == null) {
      return;
    }
    final menus = [for (final r in rects) r.inflate(2)];
    final (:policy, :owner) = menuBarTransient(
      open: _open,
      windowMenu: _windowMenuTip != null,
      desktopMenu: _desktopMenuAt != null,
      frontmost: ShellChannel.instance.frontmostAppId,
    );
    final supported = await LayerControl.transient(
      rects: menus,
      policy: policy,
      owner: owner,
    );
    if (!mounted ||
        (_open == null && _windowMenuTip == null && _desktopMenuAt == null)) {
      return;
    }
    LayerControl.inputRegion(supported ? [_barRect, ...menus] : null);
  }

  /// A press outside the open menu ended it (Ossido, #149).
  void _transientDismissed() {
    if (_open != null) {
      final menu = _menuKey.currentState;
      menu == null ? _closeMenu() : menu.close();
    }
    _closeWindowMenu();
    _closeDesktopMenu();
  }

  Future<void> _coverScreen() async {
    final screen = await LayerControl.outputSize();
    if (!mounted || _open == null || screen == null) return;
    LayerControl.size(0, screen.height);
  }

  void _closeMenu() {
    _switchTimer?.cancel();
    if (_open == null) return;
    setState(() {
      _open = null;
      _openEntries = null;
    });
    LayerControl.clearTransient();
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

  MenubarConfigWatcher? _ownConfig;
  ValueListenable<MenubarConfig> get _config =>
      widget.config ?? (_ownConfig ??= MenubarConfigWatcher());

  @override
  void initState() {
    super.initState();
    _config.addListener(_changed);
    ShellChannel.instance.toplevels.addListener(_changed);
    ShellChannel.instance.focused.addListener(_changed);
    ShellChannel.instance.appMenu.addListener(_changed);
    ShellChannel.instance.states.addListener(_changed);
    ShellChannel.instance.states.addListener(_desktopMenuChanged);
    widget.system?.addListener(_changed);
    ShellChannel.instance.refresh();
    HardwareKeyboard.instance.addHandler(_keyChanged);
    widget.media?.start();
    widget.media?.addListener(_changed);
    _scheduleTick();
    _windowMenus = ShellChannel.instance.windowMenus.listen(
      (menu) => _openWindowMenu(menu.tip),
    );
    _dismissed = ShellChannel.instance.transientDismissed.listen(
      (_) => _transientDismissed(),
    );
  }

  StreamSubscription<void>? _dismissed;

  /// The arrangement popover of a window (long press on the green button): the tip at the
  /// point given by Ossido. While it is open the surface covers the screen but takes the
  /// pointer only on the popover; a click outside closes it (#149).
  StreamSubscription<({String id, Offset tip})>? _windowMenus;
  Offset? _windowMenuTip;

  Future<void> _openWindowMenu(Offset tip) async {
    _closeMenu();
    final screen = await LayerControl.outputSize();
    if (!mounted) return;
    setState(() => _windowMenuTip = tip);
    LayerControl.size(0, screen?.height ?? tip.dy + 400);
    LayerControl.inputRegion([_barRect]);
    LayerControl.keyboard('exclusive');
  }

  /// The desktop context menu: the wallpaper announces `desktopmenu` = "x y", the file
  /// manager's desktop "x y desktop" or "x y items" (#147).
  Offset? _desktopMenuAt;
  DesktopMenuKind _desktopMenuKind = DesktopMenuKind.wallpaper;

  /// The desktop menu's entries, taken when it opens. The chosen item makes the file manager
  /// announce its new state (Move to Trash, Use Stacks…): a new list on that rebuild would
  /// restart the menu halfway through its closing fade and leave it open.
  List<MenuEntry>? _desktopMenuEntries;
  String _desktopMenuState = '';

  /// Each action sent to the file manager's desktop carries a new serial: the broadcast is a
  /// state, and the same action twice must still arrive.
  int _desktopActionSerial = 0;

  void _desktopMenuChanged() {
    final value = ShellChannel.instance.states.value['desktopmenu'] ?? '';
    if (value == _desktopMenuState) return;
    _desktopMenuState = value;
    final request = parseDesktopMenu(value);
    if (request == null) {
      _closeDesktopMenu(announce: false);
    } else {
      _desktopMenuKind = request.$2;
      _openDesktopMenu(request.$1);
    }
  }

  void _desktopAction(String id) {
    _desktopActionSerial++;
    ShellChannel.instance.broadcast(
      'desktop.action',
      '$id $_desktopActionSerial',
    );
  }

  Future<void> _openDesktopMenu(Offset at) async {
    _closeMenu();
    _closeWindowMenu();
    final screen = await LayerControl.outputSize();
    if (!mounted) return;
    setState(() {
      _desktopMenuAt = at;
      _desktopMenuEntries = _desktopEntries();
    });
    LayerControl.size(0, screen?.height ?? at.dy + 400);
    LayerControl.inputRegion([_barRect]);
    LayerControl.keyboard('exclusive');
  }

  void _closeDesktopMenu({bool announce = true}) {
    if (_desktopMenuAt == null) return;
    setState(() {
      _desktopMenuAt = null;
      _desktopMenuEntries = null;
    });
    if (announce) {
      _desktopMenuState = '';
      ShellChannel.instance.broadcast('desktopmenu', '');
    }
    LayerControl.clearTransient();
    LayerControl.keyboard('none');
    LayerControl.inputRegion(null);
    LayerControl.size(0, barHeight);
  }

  List<MenuEntry> _desktopEntries() {
    final settings = _settingsApp;
    final owner = DesktopOwnerState.parse(
      ShellChannel.instance.states.value['desktop.state'] ?? '',
    );
    switch (_desktopMenuKind) {
      case DesktopMenuKind.desktop:
        return ownerDesktopMenu(
          l: l10nOf(context),
          state: owner,
          onAction: _desktopAction,
          onChangeWallpaper: settings == null ? null : () => _launch(settings),
        );
      case DesktopMenuKind.items:
        return ownerItemsMenu(
          l: l10nOf(context),
          state: owner,
          onAction: _desktopAction,
        );
      case DesktopMenuKind.wallpaper:
        break;
    }
    return desktopMenu(
      l: l10nOf(context),
      onNewFolder: () =>
          newFolder(desktopFolder(), l10nOf(context).desktopUntitledFolder),
      onChangeWallpaper: settings == null ? null : () => _launch(settings),
    );
  }

  /// The power confirmation on screen (SH-MB-05), above everything: the surface covers the
  /// screen while it is up.
  PowerAction? _power;

  Future<void> _requestPower(PowerAction action, {required bool now}) async {
    _closeMenu();
    if (now || action == PowerAction.sleep) {
      unawaited(_runPower(action));
      return;
    }
    final screen = await LayerControl.outputSize();
    if (!mounted) return;
    setState(() => _power = action);
    LayerControl.size(0, screen?.height ?? 800);
    LayerControl.inputRegion(null);
    LayerControl.keyboard('exclusive');
  }

  Future<void> _runPower(PowerAction action) => runPower(
    action,
    logOut: () => ShellChannel.instance.execute('session.quit'),
  );

  void _closePower() {
    if (_power == null) return;
    setState(() => _power = null);
    LayerControl.keyboard('none');
    LayerControl.inputRegion(null);
    LayerControl.size(0, barHeight);
  }

  void _closeWindowMenu() {
    if (_windowMenuTip == null) return;
    setState(() => _windowMenuTip = null);
    LayerControl.clearTransient();
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
    _config.removeListener(_changed);
    _ownConfig?.dispose();
    _timer?.cancel();
    _switchTimer?.cancel();
    ShellChannel.instance.toplevels.removeListener(_changed);
    ShellChannel.instance.focused.removeListener(_changed);
    ShellChannel.instance.appMenu.removeListener(_changed);
    ShellChannel.instance.states.removeListener(_changed);
    ShellChannel.instance.states.removeListener(_desktopMenuChanged);
    widget.system?.removeListener(_changed);
    _windowMenus?.cancel();
    _dismissed?.cancel();
    HardwareKeyboard.instance.removeHandler(_keyChanged);
    widget.media?.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    if (mounted) setState(() {});
  }

  /// The app menu's title: the name the app gives its menu, else its entry's name.
  String get _appName => _appMenus()?.first.title ?? _appNameFromEntries;

  String get _appNameFromEntries {
    final window = ShellChannel.instance.focusedToplevel;
    // An app that closed its last window stays frontmost (#142, APP-LC): Ossido announces it.
    final windowless =
        ShellChannel.instance.states.value['app.frontmost'] ?? '';
    final appId = window?.appId ?? windowless;
    if (window == null && windowless.isEmpty) return 'Silica';
    final entry = widget.entries?.values
        .where((e) => e.matchesAppId(appId))
        .firstOrNull;
    return entry?.name ??
        (appId.isNotEmpty ? appId : (window?.title ?? 'Silica'));
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
    final statusMenuKey = (states['statusmenu'] ?? '').split(' ').first;
    final statusMenu = statusMenuKey.isEmpty ? null : _rectOf(statusMenuKey);
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
                if (_appMenus() case final own?)
                  for (var i = 1; i < own.length; i++)
                    _item(own[i].title, regular, key: _keyFor('menu$i'))
                else
                  for (final menu in genericMenus)
                    if (_entries(menu).isNotEmpty)
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
                      top: barHeight,
                      onDismissed: _closeWindowMenu,
                      onRectsChanged: _menuRegion,
                    ),
                  ),
                if (_power case final action?)
                  Positioned.fill(
                    child: LayoutBuilder(
                      builder: (context, box) => Stack(
                        children: [
                          // Clicks outside the card do nothing (it is modal); the
                          // menu bar stays visible above it.
                          const Positioned.fill(
                            child: AbsorbPointer(child: SizedBox.expand()),
                          ),
                          Positioned(
                            left: (box.maxWidth - powerCardSize.width) / 2,
                            // Upper third: the measured card's top at ~20 % of the height.
                            top: box.maxHeight * 0.205,
                            child: PowerConfirmation(
                              action: action,
                              user: _user,
                              onCancel: _closePower,
                              onConfirm: () {
                                _closePower();
                                unawaited(_runPower(action));
                              },
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                if (_desktopMenuAt case final at?)
                  Positioned.fill(
                    child: MenuHost(
                      key: ValueKey(at),
                      entries: _desktopMenuEntries ?? _desktopEntries(),
                      anchor: Offset(at.dx, at.dy - desktopMenuAbove),
                      top: barHeight,
                      onDismissed: _closeDesktopMenu,
                      onRectsChanged: _menuRegion,
                    ),
                  ),
                if (_open case final name?)
                  Positioned.fill(
                    child: MenuHost(
                      key: _menuKey,
                      entries: _openEntries ?? _entries(name),
                      anchor: Offset(
                        (open?.left ?? 0) - menuLeftOfItem,
                        barHeight + 1,
                      ),
                      handlesPointer: false,
                      top: barHeight,
                      onDismissed: _closeMenu,
                      onNext: () => _neighbour(1),
                      onPrevious: () => _neighbour(-1),
                      // With a submenu open the input region includes it.
                      onRectsChanged: _menuRegion,
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
    final config = _config.value;
    if (s == null) {
      return const [StatusIcon(kind: StatusKind.network, cell: 22)];
    }
    return [
      // While media plays (SH-MB-33): the play glyph, 17 pt; the menu is the Now Playing
      // panel.
      if (widget.media?.now?.playing ?? false) ...[
        GestureDetector(
          key: _keyFor('media'),
          behavior: HitTestBehavior.opaque,
          onTap: () => _toggleStatusMenu('media'),
          child: const StatusIcon(kind: StatusKind.nowPlaying, cell: 17),
        ),
        const SizedBox(width: statusGap),
      ],
      // While a Focus is on, its moon (SH-MB-26); the menu is the Focus panel.
      if (s.doNotDisturb) ...[
        GestureDetector(
          key: _keyFor('dnd'),
          behavior: HitTestBehavior.opaque,
          onTap: () => _toggleStatusMenu('dnd'),
          child: const StatusIcon(kind: StatusKind.focus, cell: 22),
        ),
        const SizedBox(width: statusGap),
      ],
      // Bluetooth and Sound when Settings › Menu Bar asks for them (SH-MB-24, SH-MB-25):
      // their menus are the Control Center's detail panels, like Wi-Fi. Sound "when
      // active" shows while media plays (provisional reading of the setting).
      if (config.bluetooth && s.bluetoothAvailable) ...[
        GestureDetector(
          key: _keyFor('bluetooth'),
          behavior: HitTestBehavior.opaque,
          onTap: () => _toggleStatusMenu('bluetooth'),
          child: StatusIcon(
            kind: StatusKind.bluetooth,
            cell: 22,
            dim: !s.bluetoothPowered,
          ),
        ),
        const SizedBox(width: statusGap),
      ],
      if (s.audioAvailable &&
          switch (config.sound) {
            SoundItem.always => true,
            SoundItem.active => widget.media?.now?.playing ?? false,
            SoundItem.never => false,
          }) ...[
        GestureDetector(
          key: _keyFor('sound'),
          behavior: HitTestBehavior.opaque,
          onTap: () => _toggleStatusMenu('sound'),
          child: StatusIcon(
            kind: StatusKind.sound,
            cell: 26,
            level: s.muted ? 0 : s.volume,
            dim: s.muted,
          ),
        ),
        const SizedBox(width: statusGap),
      ],
      if (s.batteryPresent && config.battery) ...[
        // The battery menu (SH-MB-27): a standard menu with the charge, the power source
        // and Battery Settings.
        GestureDetector(
          key: _keyFor('battery'),
          behavior: HitTestBehavior.opaque,
          onTap: () => _open == 'battery' ? _closeMenu() : _openMenu('battery'),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (config.batteryPercent) ...[
                Text(
                  '${s.batteryPercent.round()} %',
                  style: uiText(size: fontSize, color: statusColor),
                ),
                const SizedBox(width: 4),
              ],
              StatusIcon(
                kind: StatusKind.battery,
                cell: 33,
                level: s.batteryPercent / 100,
                charging: s.batteryCharging,
              ),
            ],
          ),
        ),
        const SizedBox(width: statusGap),
      ],
      if (s.wired && s.wifiSsid.isEmpty)
        const StatusIcon(kind: StatusKind.wired, cell: 22)
      else if (s.wifiAvailable)
        GestureDetector(
          key: _keyFor('wifi'),
          behavior: HitTestBehavior.opaque,
          onTap: () => _toggleStatusMenu('wifi'),
          child: StatusIcon(
            kind: StatusKind.network,
            cell: 22,
            level: s.wifiStrength / 100,
            dim: !s.wifiEnabled || s.wifiSsid.isEmpty,
          ),
        ),
    ];
  }

  /// A status menu (menubar.md › Status menu): the Control Center panel of `key` (`wifi`,
  /// `dnd`) below the icon, which the Center draws when the bar announces `statusmenu`.
  void _toggleStatusMenu(String key) {
    final shell = ShellChannel.instance;
    final rect = _rectOf(key);
    if (rect == null) return;
    final open =
        (shell.states.value['statusmenu'] ?? '').split(' ').first == key;
    shell.broadcast('statusmenu', open ? '' : '$key ${rect.right}');
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
