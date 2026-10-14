// L'uscita forzata (⌘⌥⎋, plan/reference/screenshot-ui.md): un pannello 370 × 310 al centro,
// con la barra e i pulsanti della finestra (la riduzione disattivata), due righe di
// spiegazione, l'elenco delle app aperte (313 pt di larghezza utile, righe da 22, icona 18 a
// +8, nome a +28; l'app in primo piano già scelta), un aiuto piccolo in basso a sinistra e il
// pulsante blu in basso a destra (115 × 30). Il pulsante termina l'app con `app.force_quit`.
// I testi sono nostri; la posizione del riferimento era ricordata, qui si centra.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/layer_arbiter.dart';
import '../system/shell_channel.dart';
import '../system/silica_icons.dart';

const Size forceQuitSize = Size(370, 310);
const Rect forceQuitList = Rect.fromLTWH(20, 92, 330, 158);
const double forceQuitRow = 22;
const Rect forceQuitButton = Rect.fromLTWH(240, 265, 115, 30);
const Color forceQuitBlue = Color(0xFF0A62E0);

/// I colori del pannello. Scuro misurato (screenshot-ui.md › K-06): fondo `#272727` con la
/// barra `#2f2f2f`, filo di 1 pt `#060606` fuori e `#3d3d3d` dentro, titolo `#9e9e9e` (finestra
/// non chiave), testi e nomi `#dedede`, elenco sul fondo del pannello con filo `#353535`, riga
/// scelta sRGB `#0059d1`, pulsante sRGB `#007aff`, ombra scura di ~50 pt. Restano *stimati*
/// in scuro il pulsante disattivato (mai visto: c'è sempre una riga scelta) e la riduzione
/// disattivata (windows.md `#4d4d4d`).
@immutable
class ForceQuitColors {
  const ForceQuitColors({
    required this.fill,
    required this.bar,
    required this.borderOuter,
    required this.borderInner,
    required this.button,
    required this.title,
    required this.text,
    required this.help,
    required this.list,
    required this.listBorder,
    required this.chosen,
    required this.disabledButton,
    required this.disabledButtonText,
    required this.minimize,
    required this.shadow,
  });

  final Color fill;

  /// La fascia della barra del titolo (in chiaro uguale al fondo).
  final Color bar;

  /// Il filo attorno al pannello, fuori e dentro (null: nessuno).
  final Color? borderOuter;
  final Color? borderInner;
  final Color button;
  final Color title;
  final Color text;
  final Color help;
  final Color list;
  final Color listBorder;
  final Color chosen;
  final Color disabledButton;
  final Color disabledButtonText;
  final Color minimize;
  final Color shadow;

  static const ForceQuitColors light = ForceQuitColors(
    fill: Color(0xFFF6F6F6),
    bar: Color(0xFFF6F6F6),
    borderOuter: null,
    borderInner: null,
    button: forceQuitBlue,
    title: Color(0xFF595959),
    text: Color(0xFF1D1D1F),
    help: Color(0xFF3C3C43),
    list: Color(0xFFFFFFFF),
    listBorder: Color(0xFFD9D9D9),
    chosen: forceQuitBlue,
    disabledButton: Color(0xFFB8C9E8),
    disabledButtonText: Color(0xFFFFFFFF),
    minimize: Color(0xFFDDDDDF),
    shadow: Color(0x55000000),
  );

  static const ForceQuitColors dark = ForceQuitColors(
    fill: Color(0xFF272727),
    bar: Color(0xFF2F2F2F),
    borderOuter: Color(0xFF060606),
    borderInner: Color(0xFF3D3D3D),
    button: Color(0xFF007AFF),
    title: Color(0xFF9E9E9E),
    text: Color(0xFFDEDEDE),
    help: Color(0xFFDEDEDE),
    list: Color(0xFF272727),
    listBorder: Color(0xFF353535),
    chosen: Color(0xFF0059D1),
    disabledButton: Color(0xFF2F2F30),
    disabledButtonText: Color(0xFF626263),
    minimize: Color(0xFF4D4D4D),
    // Sotto: −69 livelli a 1 pt, −50 a 13, −30 a 25, −14 a 37, −5 a 49 (su `#7a7b80`).
    shadow: Color(0xE6000000),
  );

  static ForceQuitColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;
}

/// Un'app aperta nell'elenco.
class ForceQuitApp {
  const ForceQuitApp(this.appId, this.name, this.icon);
  final String appId;
  final String name;
  final IconSource? icon;
}

class ForceQuitLayer extends StatefulWidget {
  const ForceQuitLayer({super.key, required this.entries});
  final Map<String, DesktopEntry> entries;

  @override
  State<ForceQuitLayer> createState() => _ForceQuitLayerState();
}

class _ForceQuitLayerState extends State<ForceQuitLayer> {
  StreamSubscription<String>? _commands;
  bool _open = false;
  String? _chosen;
  final FocusNode _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _commands = ShellChannel.instance.commands.listen((id) {
      if (id == 'app.force_quit_panel') _show();
    });
    ShellChannel.instance.toplevels.addListener(_changed);
  }

  @override
  void dispose() {
    LayerArbiter.instance.remove(this);
    _commands?.cancel();
    ShellChannel.instance.toplevels.removeListener(_changed);
    _focus.dispose();
    super.dispose();
  }

  void _changed() {
    if (_open) setState(() {});
  }

  /// Le app aperte, per nome, una volta ciascuna.
  List<ForceQuitApp> _apps() {
    final seen = <String>{};
    final apps = <ForceQuitApp>[];
    for (final top in ShellChannel.instance.toplevels.value) {
      if (top.appId.isEmpty || !seen.add(top.appId)) continue;
      final entry = widget.entries.values
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
      apps.add(
        ForceQuitApp(
          top.appId,
          entry?.name ?? (top.title.isNotEmpty ? top.title : top.appId),
          icon,
        ),
      );
    }
    apps.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
    return apps;
  }

  void _show() {
    // Scelta: l'app in primo piano.
    final focused = ShellChannel.instance.focused.value;
    final top = ShellChannel.instance.toplevels.value
        .where((t) => t.id == focused)
        .firstOrNull;
    setState(() {
      _open = true;
      _chosen = top?.appId;
    });
    LayerArbiter.instance.set(this, const LayerWish.all());
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  void _hide() {
    setState(() => _open = false);
    LayerArbiter.instance.set(this, const LayerWish.none());
  }

  void _quit() {
    final app = _chosen;
    if (app == null) return;
    ShellChannel.instance.execute('app.force_quit', {'app_id': app});
    _hide();
  }

  @override
  Widget build(BuildContext context) {
    if (!_open) return const SizedBox.shrink();
    final apps = _apps();
    final c = ForceQuitColors.of(context);
    return Focus(
      focusNode: _focus,
      onKeyEvent: (node, event) {
        if (event is KeyDownEvent &&
            event.logicalKey == LogicalKeyboardKey.escape) {
          _hide();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Center(
        child: Container(
          key: const ValueKey('uscita forzata'),
          width: forceQuitSize.width,
          height: forceQuitSize.height,
          decoration: BoxDecoration(
            color: c.fill,
            borderRadius: BorderRadius.circular(16),
            border: c.borderOuter == null
                ? null
                : Border.all(color: c.borderOuter!),
            boxShadow: [
              c.borderOuter == null
                  ? BoxShadow(
                      color: c.shadow,
                      blurRadius: 40,
                      offset: const Offset(0, 18),
                    )
                  : BoxShadow(
                      color: c.shadow,
                      blurRadius: 37,
                      offset: const Offset(0, 10),
                    ),
            ],
          ),
          child: Stack(
            children: [
              // La barra del titolo (in scuro più chiara del fondo) e il filo interno.
              Positioned(
                left: 0,
                right: 0,
                top: 0,
                height: 52,
                child: ClipRRect(
                  borderRadius: const BorderRadius.vertical(
                    top: Radius.circular(15),
                  ),
                  child: ColoredBox(color: c.bar),
                ),
              ),
              if (c.borderInner case final inner?)
                Positioned.fill(
                  child: IgnorePointer(
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(15),
                        border: Border.all(color: inner),
                      ),
                    ),
                  ),
                ),
              // I pulsanti della finestra: chiudi, riduci disattivato, schermo intero.
              for (final (x, color) in [
                (26.0, const Color(0xFFFF5F57)),
                (49.0, c.minimize),
                (72.0, const Color(0xFF28C840)),
              ])
                Positioned(
                  left: x - 7,
                  top: 26 - 7,
                  width: 14,
                  height: 14,
                  child: GestureDetector(
                    onTap: x == 26 ? _hide : null,
                    child: DecoratedBox(
                      decoration: BoxDecoration(
                        color: color,
                        shape: BoxShape.circle,
                      ),
                    ),
                  ),
                ),
              // Il titolo: semibold ~13 (maiuscole di 9 pt), `#595959`, da x 85.
              Positioned(
                left: 85,
                right: 20,
                top: 8,
                child: Text(
                  'Termina le app',
                  style: uiText(
                    size: 13,
                    color: c.title,
                  ).copyWith(fontWeight: FontWeight.w600),
                ),
              ),
              Positioned(
                left: 19,
                width: 332,
                top: 51,
                child: Text(
                  'Se un\'app non risponde, sceglila nell\'elenco e fai clic su '
                  '«Termina».',
                  style: uiText(size: 13, color: c.text),
                ),
              ),
              Positioned.fromRect(
                rect: forceQuitList,
                child: Container(
                  decoration: BoxDecoration(
                    color: c.list,
                    border: Border.all(color: c.listBorder),
                  ),
                  child: ListView(
                    padding: EdgeInsets.zero,
                    itemExtent: forceQuitRow,
                    children: [for (final app in apps) _row(c, app)],
                  ),
                ),
              ),
              Positioned(
                left: 19,
                width: 215,
                top: 265,
                child: Text(
                  'Questo pannello si apre anche con ⌘⌥Esc.',
                  style: uiText(size: 10, color: c.help),
                ),
              ),
              Positioned.fromRect(
                rect: forceQuitButton,
                child: GestureDetector(
                  key: const ValueKey('termina'),
                  onTap: _chosen == null ? null : _quit,
                  child: Container(
                    decoration: BoxDecoration(
                      color: _chosen == null ? c.disabledButton : c.button,
                      borderRadius: BorderRadius.circular(15),
                    ),
                    alignment: Alignment.center,
                    child: Text(
                      'Termina',
                      style: uiText(
                        size: 13,
                        color: _chosen == null
                            ? c.disabledButtonText
                            : const Color(0xFFFFFFFF),
                      ).copyWith(fontWeight: FontWeight.w600),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _row(ForceQuitColors c, ForceQuitApp app) {
    final chosen = app.appId == _chosen;
    final text = chosen ? const Color(0xFFFFFFFF) : c.text;
    return GestureDetector(
      key: ValueKey('app ${app.appId}'),
      behavior: HitTestBehavior.opaque,
      onTap: () => setState(() => _chosen = app.appId),
      onDoubleTap: () {
        setState(() => _chosen = app.appId);
        _quit();
      },
      child: Container(
        color: chosen ? c.chosen : null,
        child: Row(
          children: [
            const SizedBox(width: 8),
            SizedBox(
              width: 18,
              height: 18,
              child: switch (app.icon) {
                final icon? => SilicaIcon(icon, side: 18, dark: false),
                null => null,
              },
            ),
            const SizedBox(width: 2),
            Expanded(
              child: Text(
                app.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: uiText(size: 13, color: text),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
