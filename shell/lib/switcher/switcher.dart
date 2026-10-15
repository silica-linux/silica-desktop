// The app switcher (⌘Tab), from plan/reference/appswitcher.md (light appearance).
//
// Panel centered on the screen with the Dock material: 104 pt icons in one row, 134 pt
// pitch, 37 pt margin at the sides and 35 at the top; with 4 apps it is 580 × 176 pt, corner radius ~56. The
// chosen app has a 120 × 120 box behind it (radius 30, `#484950`) and below it the name (14 pt
// semibold, `#131314`) 7 pt below the box.
//
// Ossido handles the keys and forwards `switcher.next`, `switcher.previous` (with ⌘ held),
// `switcher.commit` (⌘ released) and `switcher.cancel` (Esc). The usage order of the apps
// is kept by this component from the focus events of the silica/shell channel. The component
// always runs on a full-screen overlay layer, without pointer or keyboard.
//
// Not measured: appearance animation, many apps (here the icons shrink to fit), pointer
// over an icon.

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/glass_profile.dart';
import '../system/icon_lookup.dart';
import '../system/layer_control.dart';
import '../system/shell_channel.dart';
import '../system/silica_icons.dart';

const double iconSide = 104;
const double iconPitch = 134;
const double sidePadding = 37;
const double topPadding = 35;
const double panelHeight = 176;

/// Panel corners: insets of 34, 16 and 7 pt at 4, 16 and 28 pt from the edge, i.e. an arc
/// of radius ~56 pt (not a full capsule).
const double panelRadius = 56;
const double selectionSide = 120;
const double selectionRadius = 30;
const double labelGap = 7;
const double labelSize = 14;
const Color selectionColor = Color(0xFF484950);
const Color labelColor = Color(0xFF131314);

/// In dark (appswitcher.md): selection `#a9aaaf`, label `#f7f7f8`.
const Color selectionColorDark = Color(0xFFA9AAAF);
const Color labelColorDark = Color(0xFFF7F7F8);

/// Capsule width for `count` apps at full size.
double panelWidth(int count) =>
    2 * sidePadding +
    count * iconSide +
    math.max(0, count - 1) * (iconPitch - iconSide);

/// An app in the switcher: its windows from the most recent.
@immutable
class SwitcherApp {
  const SwitcherApp({
    required this.key,
    required this.name,
    required this.windows,
    this.icon,
  });

  /// The app_id (or the window id, if the app has none).
  final String key;
  final String name;
  final List<String> windows;
  final IconSource? icon;
}

/// The usage order of the windows and the open switcher.
class SwitcherModel {
  SwitcherModel({Map<String, DesktopEntry>? entries})
    : _entries = entries ?? const {};

  final Map<String, DesktopEntry> _entries;

  /// Windows from the most recent.
  final List<String> _recent = [];
  List<Toplevel> _toplevels = const [];

  /// The apps of the open switcher (null: closed) and the chosen one.
  List<SwitcherApp>? apps;
  int selected = 0;

  bool get visible => apps != null;

  void toplevels(List<Toplevel> list) {
    _toplevels = list;
    final ids = {for (final t in list) t.id};
    _recent.removeWhere((id) => !ids.contains(id));
    // New windows at the end, in the order they appeared.
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

  /// The apps in usage order.
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

  /// ⌘Tab: opens the switcher on the second app (the one used before), or moves forward.
  void next() => _step(1);

  /// ⇧⌘Tab: opens on the last app, or moves back.
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

  /// ⌘ released: the window to activate (the most recent of the chosen app).
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

/// The panel, centered in the space it has.
class SwitcherPanel extends StatelessWidget {
  const SwitcherPanel({super.key, required this.apps, required this.selected});

  final List<SwitcherApp> apps;
  final int selected;

  @override
  Widget build(BuildContext context) {
    final dark =
        MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark;
    return LayoutBuilder(
      builder: (context, constraints) {
        // Too many apps for the screen: everything shrinks together.
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
              child: ProfiledGlass(
                profile: ShellGlassProfile.dock,
                radius: panelRadius * k,
                material: GlassMaterial.clear,
                child: const SizedBox.expand(),
              ),
            ),
            for (var i = 0; i < apps.length; i++)
              ..._item(
                dark,
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

  List<Widget> _item(bool dark, int index, double x, double y, double k) {
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
              color: dark ? selectionColorDark : selectionColor,
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
          final source? => SilicaIcon(source, side: side, dark: false),
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
              color: dark ? labelColorDark : labelColor,
            ),
          ),
        ),
    ];
  }
}
