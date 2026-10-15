// The app switcher (⌘Tab), from plan/reference/appswitcher.md (light appearance).
//
// Panel centered on the screen with the Dock material: 104 pt icons in one row, 134 pt
// pitch, 37 pt margin at the sides and 35 at the top; with 4 apps it is 580 × 176 pt, corner radius ~56. The
// chosen app has a 120 × 120 box behind it (radius 30, `#484950`) and below it the name (14 pt
// semibold, `#131314`) 7 pt below the box.
//
// Ossido handles the keys and forwards `switcher.next`, `switcher.previous` (⇧⌘Tab, with ⌘
// held), `switcher.commit` (⌘ released), `switcher.cancel` (Esc) and `switcher.expose` (↑ or
// ↓: App Exposé of the chosen app). The usage order of the apps is kept by this component
// from the focus events of the silica/shell channel. The component always runs on a
// full-screen overlay layer; the keyboard is Ossido's, and the pointer reaches only the
// panel while it is open: it moves the selection, and a file dragged from another app
// can be dropped on an icon to open it with that app.
//
// ⌘` is not bound (measured: it does nothing on the Italian layout; Ossido matches
// shortcuts on the layout's own symbols).

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
    this.openFiles,
  });

  /// The app_id (or the window id, if the app has none).
  final String key;
  final String name;
  final List<String> windows;
  final IconSource? icon;

  /// The app's command line, to open files dropped on its icon (null: it takes none).
  final String? openFiles;
}

/// Where the panel is on a screen of `size` with `count` apps: too many apps for the screen and
/// everything shrinks together by `k`.
({double left, double top, double width, double height, double k})
switcherFrame(Size size, int count) {
  final natural = panelWidth(count);
  final room = size.width - 2 * sidePadding;
  final k = natural > room && room > 0 ? room / natural : 1.0;
  final width = natural * k;
  final height = panelHeight * k;
  return (
    left: (size.width - width) / 2,
    top: (size.height - height) / 2,
    width: width,
    height: height,
    k: k,
  );
}

/// The app under `position` (the panel's cells, one pitch wide), or null outside the panel.
int? switcherIndexAt(Size size, int count, Offset position) {
  if (count == 0) return null;
  final f = switcherFrame(size, count);
  final panel = Rect.fromLTWH(f.left, f.top, f.width, f.height);
  if (!panel.contains(position)) return null;
  final gap = iconPitch - iconSide;
  final first = f.left + (sidePadding - gap / 2) * f.k;
  return ((position.dx - first) / (iconPitch * f.k)).floor().clamp(
    0,
    count - 1,
  );
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
    return SwitcherApp(
      key: key,
      name: name,
      windows: ids,
      icon: icon,
      openFiles: entry?.rawExec,
    );
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

  /// Moves the choice to `index` (the pointer, a file dragged over an icon).
  void select(int index) {
    final list = apps;
    if (list == null || index < 0 || index >= list.length) return;
    selected = index;
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
  StreamSubscription<SilicaDragEvent>? _drags;
  final ShellChannel _shell = ShellChannel.instance;

  /// The layer's size (from the last build) and the input region last given to Ossido.
  Size _size = Size.zero;
  String _regionKey = '';

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
    _drags = SilicaDrag.events.listen(_drag);
    unawaited(_shell.refresh());
  }

  @override
  void dispose() {
    _commands?.cancel();
    _drags?.cancel();
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
      case 'switcher.expose':
        // ↑ or ↓: App Exposé of the chosen app, which stays after ⌘ is released.
        final window = _model.commit();
        setState(() {});
        if (window != null) unawaited(_exposeOf(window));
    }
  }

  Future<void> _exposeOf(String window) async {
    await _shell.activate(window);
    await _shell.execute('panorama.app');
  }

  /// A file dragged from another app: the icon under it is chosen; dropped on it, the app
  /// opens the file.
  void _drag(SilicaDragEvent event) {
    final apps = _model.apps;
    if (apps == null || apps.isEmpty) return;
    switch (event) {
      case SilicaDragOver(:final position):
        final index = switcherIndexAt(_size, apps.length, position);
        if (index != null && index != _model.selected) {
          setState(() => _model.select(index));
        }
      case SilicaDrop(:final position, :final paths):
        final index = switcherIndexAt(_size, apps.length, position);
        if (index == null || paths.isEmpty) return;
        final exec = apps[index].openFiles;
        final command = exec == null ? null : execWithFiles(exec, paths);
        setState(_model.cancel);
        if (command != null) unawaited(_shell.launch(command));
      case SilicaDragLeave() || SilicaDragFinished():
        break;
    }
  }

  /// The pointer reaches the panel and nothing else while the switcher is open.
  void _syncRegion(List<SwitcherApp>? apps) {
    final key = apps == null || apps.isEmpty
        ? ''
        : '${_size.width},${_size.height},${apps.length}';
    if (key == _regionKey) return;
    _regionKey = key;
    final rects = key.isEmpty
        ? const <Rect>[]
        : [switcherFrame(_size, apps!.length)]
              .map((f) => Rect.fromLTWH(f.left, f.top, f.width, f.height))
              .toList();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) LayerControl.inputRegion(rects);
    });
  }

  /// A click on an icon activates that app, as releasing ⌘ on it would.
  void _click(int index) {
    setState(() => _model.select(index));
    final window = _model.commit();
    setState(() {});
    if (window != null) unawaited(_shell.activate(window));
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        _size = constraints.biggest;
        final apps = _model.apps;
        _syncRegion(apps);
        if (apps == null || apps.isEmpty) return const SizedBox.expand();
        return SwitcherPanel(
          apps: apps,
          selected: _model.selected,
          onHover: (i) {
            if (i != _model.selected) setState(() => _model.select(i));
          },
          onTap: _click,
        );
      },
    );
  }
}

/// The panel, centered in the space it has.
class SwitcherPanel extends StatelessWidget {
  const SwitcherPanel({
    super.key,
    required this.apps,
    required this.selected,
    this.onHover,
    this.onTap,
  });

  final List<SwitcherApp> apps;
  final int selected;

  /// The pointer entered an icon's cell (it moves the choice at once); a click on it.
  final ValueChanged<int>? onHover;
  final ValueChanged<int>? onTap;

  @override
  Widget build(BuildContext context) {
    final dark =
        MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark;
    return LayoutBuilder(
      builder: (context, constraints) {
        // Too many apps for the screen: everything shrinks together.
        final frame = switcherFrame(constraints.biggest, apps.length);
        final (:left, :top, :width, :height, :k) = frame;
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
            // On top, so the pointer reaches them through the selection box and the icons.
            for (var i = 0; i < apps.length; i++) ..._cell(i, left, top, k),
          ],
        );
      },
    );
  }

  /// The cell of an icon (one pitch wide, the panel's height): hover and click.
  List<Widget> _cell(int index, double left, double top, double k) {
    if (onHover == null && onTap == null) return const [];
    final gap = iconPitch - iconSide;
    return [
      Positioned(
        key: ValueKey('switcher cell $index'),
        left: left + (sidePadding - gap / 2 + index * iconPitch) * k,
        top: top,
        width: iconPitch * k,
        height: panelHeight * k,
        child: MouseRegion(
          onEnter: (_) => onHover?.call(index),
          onHover: (_) => onHover?.call(index),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: () => onTap?.call(index),
          ),
        ),
      ),
    ];
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
