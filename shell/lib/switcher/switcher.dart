// Il selettore delle app (⌘Tab), da plan/reference/appswitcher.md (aspetto chiaro).
//
// Pannello centrato sullo schermo con il materiale del Dock: icone da 104 pt su una riga, passo
// 134 pt, 37 pt di margine ai lati e 35 in alto; con 4 app è 580 × 176 pt, angoli di raggio ~56. L'app
// scelta ha dietro un riquadro 120 × 120 (raggio 30, `#484950`) e sotto il nome (14 pt
// semigrassetto, `#131314`) 7 pt sotto il riquadro.
//
// Ossido gestisce i tasti e inoltra `switcher.next`, `switcher.previous` (con ⌘ tenuto),
// `switcher.commit` (⌘ rilasciato) e `switcher.cancel` (Esc). L'ordine d'uso delle app lo
// tiene questo componente dagli eventi di focus del canale silica/shell. Il componente resta
// sempre in esecuzione su un layer overlay a schermo intero, senza puntatore né tastiera.
//
// Non misurati: comparsa, molte app (qui le icone si riducono per entrare), puntatore sopra
// un'icona.

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import '../system/shell_icon.dart';
import '../system/silica_icons.dart';

const double iconSide = 104;
const double iconPitch = 134;
const double sidePadding = 37;
const double topPadding = 35;
const double panelHeight = 176;

/// Angoli del pannello: rientri di 34, 16 e 7 pt a 4, 16 e 28 pt dal bordo, cioè un arco
/// di raggio ~56 pt (non una capsula piena).
const double panelRadius = 56;
const double selectionSide = 120;
const double selectionRadius = 30;
const double labelGap = 7;
const double labelSize = 14;
const Color selectionColor = Color(0xFF484950);
const Color labelColor = Color(0xFF131314);

/// Larghezza della capsula per `count` app a grandezza piena.
double panelWidth(int count) =>
    2 * sidePadding +
    count * iconSide +
    math.max(0, count - 1) * (iconPitch - iconSide);

/// Un'app nel selettore: le sue finestre dalla più recente.
@immutable
class SwitcherApp {
  const SwitcherApp({
    required this.key,
    required this.name,
    required this.windows,
    this.icon,
  });

  /// L'app_id (o l'id della finestra, se l'app non ne ha uno).
  final String key;
  final String name;
  final List<String> windows;
  final IconSource? icon;
}

/// L'ordine d'uso delle finestre e il selettore aperto.
class SwitcherModel {
  SwitcherModel({Map<String, DesktopEntry>? entries})
    : _entries = entries ?? const {};

  final Map<String, DesktopEntry> _entries;

  /// Finestre dalla più recente.
  final List<String> _recent = [];
  List<Toplevel> _toplevels = const [];

  /// Le app del selettore aperto (null: chiuso) e quella scelta.
  List<SwitcherApp>? apps;
  int selected = 0;

  bool get visible => apps != null;

  void toplevels(List<Toplevel> list) {
    _toplevels = list;
    final ids = {for (final t in list) t.id};
    _recent.removeWhere((id) => !ids.contains(id));
    // Le finestre nuove in fondo, nell'ordine in cui sono comparse.
    for (final t in list.reversed) {
      if (!_recent.contains(t.id)) _recent.add(t.id);
    }
  }

  void focused(String id) {
    if (id.isEmpty) return;
    _recent
      ..remove(id)
      ..insert(0, id);
  }

  /// Le app nell'ordine d'uso.
  List<SwitcherApp> recentApps() {
    final byId = {for (final t in _toplevels) t.id: t};
    final order = <String>[];
    final windows = <String, List<String>>{};
    final first = <String, Toplevel>{};
    for (final id in _recent) {
      final t = byId[id];
      if (t == null) continue;
      final key = t.appId.isEmpty ? 'window:${t.id}' : t.appId;
      if (!windows.containsKey(key)) {
        order.add(key);
        first[key] = t;
      }
      windows.putIfAbsent(key, () => []).add(id);
    }
    return [for (final key in order) _app(key, first[key]!, windows[key]!)];
  }

  SwitcherApp _app(String key, Toplevel top, List<String> ids) {
    final entry = _entries.values
        .where((e) => e.matchesAppId(top.appId))
        .firstOrNull;
    final IconSource? icon = switch (entry) {
      null => null,
      final e => switch ((silicaIconFor(e), findIcon(e.icon))) {
        (final LayeredIcon l, _) => LayeredSource(l),
        (null, final String path) => FlatSource(path),
        _ => null,
      },
    };
    final name = entry?.name ?? (top.title.isNotEmpty ? top.title : top.appId);
    return SwitcherApp(key: key, name: name, windows: ids, icon: icon);
  }

  /// ⌘Tab: apre il selettore sulla seconda app (quella usata prima), o va avanti.
  void next() => _step(1);

  /// ⇧⌘Tab: apre sull'ultima app, o torna indietro.
  void previous() => _step(-1);

  void _step(int delta) {
    if (apps case final list?) {
      if (list.isEmpty) return;
      selected = (selected + delta) % list.length;
      return;
    }
    final list = recentApps();
    apps = list;
    selected = list.length < 2 ? 0 : (delta > 0 ? 1 : list.length - 1);
  }

  /// ⌘ rilasciato: la finestra da attivare (la più recente dell'app scelta).
  String? commit() {
    final list = apps;
    apps = null;
    if (list == null || list.isEmpty) return null;
    return list[selected.clamp(0, list.length - 1)].windows.firstOrNull;
  }

  void cancel() => apps = null;
}

class AppSwitcher extends StatefulWidget {
  const AppSwitcher({super.key, required this.entries});

  final Map<String, DesktopEntry> entries;

  @override
  State<AppSwitcher> createState() => _AppSwitcherState();
}

class _AppSwitcherState extends State<AppSwitcher> {
  late final SwitcherModel _model = SwitcherModel(entries: widget.entries);
  StreamSubscription<String>? _commands;
  final ShellChannel _shell = ShellChannel.instance;

  @override
  void initState() {
    super.initState();
    LayerControl.inputRegion(const []);
    LayerControl.keyboard('none');
    _shell.toplevels.addListener(_toplevels);
    _shell.focused.addListener(_focused);
    _toplevels();
    _focused();
    _commands = _shell.commands.listen(_command);
    unawaited(_shell.refresh());
  }

  @override
  void dispose() {
    _commands?.cancel();
    _shell.toplevels.removeListener(_toplevels);
    _shell.focused.removeListener(_focused);
    super.dispose();
  }

  void _toplevels() => _model.toplevels(_shell.toplevels.value);

  void _focused() => _model.focused(_shell.focused.value);

  void _command(String id) {
    switch (id) {
      case 'switcher.next':
        setState(_model.next);
      case 'switcher.previous':
        setState(_model.previous);
      case 'switcher.commit':
        final window = _model.commit();
        setState(() {});
        if (window != null) unawaited(_shell.activate(window));
      case 'switcher.cancel':
        setState(_model.cancel);
    }
  }

  @override
  Widget build(BuildContext context) {
    final apps = _model.apps;
    if (apps == null || apps.isEmpty) return const SizedBox.expand();
    return SwitcherPanel(apps: apps, selected: _model.selected);
  }
}

/// Il pannello, centrato nello spazio che ha.
class SwitcherPanel extends StatelessWidget {
  const SwitcherPanel({super.key, required this.apps, required this.selected});

  final List<SwitcherApp> apps;
  final int selected;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        // Troppe app per lo schermo: tutto si riduce insieme.
        final natural = panelWidth(apps.length);
        final room = constraints.maxWidth - 2 * sidePadding;
        final k = natural > room && room > 0 ? room / natural : 1.0;
        final width = natural * k;
        final height = panelHeight * k;
        final left = (constraints.maxWidth - width) / 2;
        final top = (constraints.maxHeight - height) / 2;
        return Stack(
          children: [
            Positioned(
              left: left,
              top: top,
              width: width,
              height: height,
              child: LiquidGlass(
                radius: panelRadius * k,
                material: GlassMaterial.clear,
                interactive: false,
                child: const SizedBox.expand(),
              ),
            ),
            for (var i = 0; i < apps.length; i++)
              ..._item(
                i,
                left + (sidePadding + i * iconPitch) * k,
                top + topPadding * k,
                k,
              ),
          ],
        );
      },
    );
  }

  List<Widget> _item(int index, double x, double y, double k) {
    final app = apps[index];
    final side = iconSide * k;
    final inset = (selectionSide - iconSide) / 2 * k;
    return [
      if (index == selected)
        Positioned(
          left: x - inset,
          top: y - inset,
          width: selectionSide * k,
          height: selectionSide * k,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: selectionColor,
              borderRadius: BorderRadius.circular(selectionRadius * k),
            ),
          ),
        ),
      Positioned(
        left: x,
        top: y,
        width: side,
        height: side,
        child: switch (app.icon) {
          final source? => ShellIcon(source, side: side, dark: false),
          null => DecoratedBox(
            decoration: BoxDecoration(
              color: const Color(0xFFDDDDE2),
              borderRadius: BorderRadius.circular(side * 0.225),
            ),
          ),
        },
      ),
      if (index == selected)
        Positioned(
          left: x - inset - 40 * k,
          top: y - inset + (selectionSide + labelGap) * k,
          width: (selectionSide + 80) * k,
          child: Text(
            app.name,
            textAlign: TextAlign.center,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              fontFamily: uiFontFamily,
              package: 'silica_ui',
              fontSize: labelSize * k,
              fontWeight: FontWeight.w600,
              height: 1,
              color: labelColor,
            ),
          ),
        ),
    ];
  }
}
