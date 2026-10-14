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
            color: const Color(0xFFF6F6F6),
            borderRadius: BorderRadius.circular(16),
            boxShadow: const [
              BoxShadow(
                color: Color(0x55000000),
                blurRadius: 40,
                offset: Offset(0, 18),
              ),
            ],
          ),
          child: Stack(
            children: [
              // I pulsanti della finestra: chiudi, riduci disattivato, schermo intero.
              for (final (x, color) in const [
                (26.0, Color(0xFFFF5F57)),
                (49.0, Color(0xFFDDDDDF)),
                (72.0, Color(0xFF28C840)),
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
                    color: const Color(0xFF595959),
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
                  style: uiText(size: 13, color: const Color(0xFF1D1D1F)),
                ),
              ),
              Positioned.fromRect(
                rect: forceQuitList,
                child: Container(
                  decoration: BoxDecoration(
                    color: const Color(0xFFFFFFFF),
                    border: Border.all(color: const Color(0xFFD9D9D9)),
                  ),
                  child: ListView(
                    padding: EdgeInsets.zero,
                    itemExtent: forceQuitRow,
                    children: [for (final app in apps) _row(app)],
                  ),
                ),
              ),
              Positioned(
                left: 19,
                width: 215,
                top: 265,
                child: Text(
                  'Questo pannello si apre anche con ⌘⌥Esc.',
                  style: uiText(size: 10, color: const Color(0xFF3C3C43)),
                ),
              ),
              Positioned.fromRect(
                rect: forceQuitButton,
                child: GestureDetector(
                  key: const ValueKey('termina'),
                  onTap: _chosen == null ? null : _quit,
                  child: Container(
                    decoration: BoxDecoration(
                      color: _chosen == null
                          ? const Color(0xFFB8C9E8)
                          : forceQuitBlue,
                      borderRadius: BorderRadius.circular(15),
                    ),
                    alignment: Alignment.center,
                    child: Text(
                      'Termina',
                      style: uiText(
                        size: 13,
                        color: const Color(0xFFFFFFFF),
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

  Widget _row(ForceQuitApp app) {
    final chosen = app.appId == _chosen;
    final text = chosen ? const Color(0xFFFFFFFF) : const Color(0xFF1D1D1F);
    return GestureDetector(
      key: ValueKey('app ${app.appId}'),
      behavior: HitTestBehavior.opaque,
      onTap: () => setState(() => _chosen = app.appId),
      onDoubleTap: () {
        setState(() => _chosen = app.appId);
        _quit();
      },
      child: Container(
        color: chosen ? forceQuitBlue : null,
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
