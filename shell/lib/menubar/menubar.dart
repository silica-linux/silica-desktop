// La barra dei menu, da plan/reference/menubar.md (identica in chiaro e in scuro): alta
// 30 pt, senza fondo (il gradiente che comprime le luci sotto di lei lo fa Ossido), testo
// bianco in Inter 13 pt con la linea di base a y 19, nome dell'app in grassetto, area di
// stato e orologio in bianco al 90 %.

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/shell_channel.dart';
import '../system/layer_control.dart';
import '../system/system_status.dart';
import 'menus.dart';
import 'status_icons.dart';

const double barHeight = 30;
const double fontSize = 13;

/// Linea di base del testo: y 19,0.
const double baseline = 19;

/// Casella del logo: x 10, larga 34 pt; le caselle si sovrappongono di 1 pt.
const double logoLeft = 10;
const double logoCell = 34;

/// Spazio attorno al testo di una voce: casella = testo + ~21,5 pt (~10,75 per lato).
const double itemPadding = 10.75;

/// Orologio: il testo finisce 20 pt prima del bordo destro; 16 pt fra il Centro di
/// controllo e l'orologio; 7 pt fra le caselle delle icone.
const double clockRightMargin = 20;
const double clockGap = 16;
const double statusGap = 7;

const Color itemColor = Color(0xFFFFFFFF);

/// Menu aperto (menubar.md): corpo 5 pt a sinistra della casella della voce, 1 pt sotto la
/// barra; la voce ha una capsula alta 24, 3,5 pt oltre la casella per lato (`#7e7f84` su
/// `#6f7075`: bianco al ~10 %). Passando su un'altra voce il menu cambia dopo ~85 ms.
const double menuLeftOfItem = 5;
const double capsuleHeight = 24;
const double capsuleOutset = 3.5;
const Color capsuleFill = Color(0x1AFFFFFF);
const Duration menuSwitchDelay = Duration(milliseconds: 85);

/// Spazio sotto il menu per la sua ombra, quando la superficie si allunga.
const double menuShadowRoom = 40;

/// Bianco al 90 % (icone di stato e orologio: 241 su 112).
const Color statusColor = Color(0xE6FFFFFF);

/// Le voci dei menu dell'app finché il menu globale non arriva (M7): generiche.
const List<String> genericMenus = [
  'File',
  'Modifica',
  'Vista',
  'Finestra',
  'Aiuto',
];

const List<String> weekdays = ['Lun', 'Mar', 'Mer', 'Gio', 'Ven', 'Sab', 'Dom'];
const List<String> months = [
  'gen',
  'feb',
  'mar',
  'apr',
  'mag',
  'giu',
  'lug',
  'ago',
  'set',
  'ott',
  'nov',
  'dic',
];

/// Data e ora come nella barra: "Lun 12 ott  22:15" (doppio spazio fra data e ora).
String clockText(DateTime t) {
  final hh = t.hour.toString().padLeft(2, '0');
  final mm = t.minute.toString().padLeft(2, '0');
  return '${weekdays[t.weekday - 1]} ${t.day} ${months[t.month - 1]}  $hh:$mm';
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
  });

  /// Le app installate, per il nome dell'app con il focus.
  final Map<String, DesktopEntry>? entries;

  /// Clic sull'icona di Faro.
  final VoidCallback? onSearch;

  /// Clic sull'orologio: il Centro notifiche.
  final VoidCallback? onClock;

  /// Clic sull'icona dei controlli: il Centro di controllo.
  final VoidCallback? onControls;

  /// Stato del sistema: Wi-Fi o rete via cavo, batteria.
  final SystemSource? system;

  /// Orologio fisso (test).
  final DateTime? now;

  @override
  State<MenuBar> createState() => _MenuBarState();
}

class _MenuBarState extends State<MenuBar> {
  Timer? _timer;

  /// Il menu aperto (`logo`, `app` o il nome della voce) e i rettangoli delle voci.
  String? _open;
  final Map<String, GlobalKey> _itemKeys = {};
  final GlobalKey _rootKey = GlobalKey();
  final GlobalKey<MenuHostState> _menuKey = GlobalKey();
  Timer? _switchTimer;
  double _width = 0;

  static const List<String> _menuNames = ['logo', 'app', ...genericMenus];

  GlobalKey _keyFor(String name) => _itemKeys.putIfAbsent(name, GlobalKey.new);

  List<MenuEntry> _entries(String name) =>
      barMenu(name, _appName, (id) => ShellChannel.instance.execute(id));

  /// Il rettangolo di una voce della barra (la sua casella), nella superficie.
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

  /// Apre un menu sotto la sua voce: il corpo 5 pt a sinistra della casella, 1 pt sotto la
  /// barra. La superficie si allunga fino al menu e alla sua ombra.
  void _openMenu(String name) {
    final entries = _entries(name);
    final rect = _rectOf(name);
    if (entries.isEmpty || rect == null) return;
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

  /// → e ←: il menu accanto (solo quelli con voci).
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
      // Il menu si apre alla pressione; sulla voce già aperta si chiude.
      item == _open ? _menuKey.currentState?.close() : _openMenu(item);
      return;
    }
    if (_open != null) _menuKey.currentState?.pointerDown(e.localPosition);
  }

  void _pointerMoved(Offset p) {
    if (_open == null) return;
    // Con un menu aperto, passando su un'altra voce il menu cambia dopo ~85 ms.
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
    widget.system?.addListener(_changed);
    ShellChannel.instance.refresh();
    _scheduleTick();
  }

  // Il minuto successivo, allineato all'orologio.
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
    widget.system?.removeListener(_changed);
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
    final bar = SizedBox(
      height: barHeight,
      child: Stack(
        children: [
          // La voce col menu aperto: capsula alta 24, ~3,5 pt oltre la casella per lato.
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
                  _item(menu, regular, key: _keyFor(menu)),
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
                  onTap: widget.onControls,
                  behavior: HitTestBehavior.opaque,
                  child: const StatusIcon(kind: StatusKind.controls, cell: 26),
                ),
                const SizedBox(width: clockGap),
                GestureDetector(
                  onTap: widget.onClock,
                  behavior: HitTestBehavior.opaque,
                  child: _baselined(
                    clockText(widget.now ?? DateTime.now()),
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
                      // Con un sottomenu aperto la regione d'input lo comprende.
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

  /// Batteria (se c'è) e rete, dallo stato del sistema; senza, l'icona del Wi-Fi piena.
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
        StatusIcon(
          kind: StatusKind.network,
          cell: 22,
          level: s.wifiStrength / 100,
          dim: !s.wifiEnabled || s.wifiSsid.isEmpty,
        ),
    ];
  }

  /// Una voce: il testo con ~10,75 pt per lato.
  Widget _item(String text, TextStyle style, {Key? key}) => Padding(
    key: key,
    padding: const EdgeInsets.symmetric(horizontal: itemPadding),
    child: _baselined(text, style),
  );

  /// Testo con la linea di base a y 19.
  Widget _baselined(String text, TextStyle style) => SizedBox(
    height: barHeight,
    child: Baseline(
      baseline: baseline,
      baselineType: TextBaseline.alphabetic,
      child: Text(text, style: style),
    ),
  );
}
