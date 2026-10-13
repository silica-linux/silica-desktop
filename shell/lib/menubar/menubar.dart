// La barra dei menu, da plan/reference/menubar.md (identica in chiaro e in scuro): alta
// 30 pt, senza fondo (il gradiente che comprime le luci sotto di lei lo fa Ossido), testo
// bianco in Inter 13 pt con la linea di base a y 19, nome dell'app in grassetto, area di
// stato e orologio in bianco al 90 %.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/shell_channel.dart';
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
  const MenuBar({super.key, this.entries, this.onSearch, this.now});

  /// Le app installate, per il nome dell'app con il focus.
  final Map<String, DesktopEntry>? entries;

  /// Clic sull'icona di Faro.
  final VoidCallback? onSearch;

  /// Orologio fisso (test).
  final DateTime? now;

  @override
  State<MenuBar> createState() => _MenuBarState();
}

class _MenuBarState extends State<MenuBar> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    ShellChannel.instance.toplevels.addListener(_changed);
    ShellChannel.instance.focused.addListener(_changed);
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
    ShellChannel.instance.toplevels.removeListener(_changed);
    ShellChannel.instance.focused.removeListener(_changed);
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
    return SizedBox(
      height: barHeight,
      child: Stack(
        children: [
          const Positioned(
            left: logoLeft,
            top: 0,
            width: logoCell,
            height: barHeight,
            child: Center(child: SilicaMark(color: itemColor)),
          ),
          Positioned(
            left: logoLeft + logoCell - 1,
            top: 0,
            height: barHeight,
            child: Row(
              children: [
                _item(_appName, bold),
                for (final menu in genericMenus) _item(menu, regular),
              ],
            ),
          ),
          Positioned(
            right: clockRightMargin,
            top: 0,
            height: barHeight,
            child: Row(
              children: [
                const StatusIcon(kind: StatusKind.network, cell: 22),
                const SizedBox(width: statusGap),
                GestureDetector(
                  onTap: widget.onSearch,
                  child: const StatusIcon(kind: StatusKind.search, cell: 33),
                ),
                const SizedBox(width: statusGap),
                const StatusIcon(kind: StatusKind.controls, cell: 26),
                const SizedBox(width: clockGap),
                _baselined(clockText(widget.now ?? DateTime.now()), status),
              ],
            ),
          ),
        ],
      ),
    );
  }

  /// Una voce: il testo con ~10,75 pt per lato.
  Widget _item(String text, TextStyle style) => Padding(
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
