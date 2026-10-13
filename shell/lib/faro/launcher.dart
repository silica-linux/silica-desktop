// Il launcher delle app: Faro in un'altra modalità, da plan/reference/launcher.md (aspetto
// chiaro). Pannello di 844 × 577 pt con la cima al 30 % dello schermo, raggio 25, materiale
// di Faro. Testata di 56 pt (icona delle app a 22 pt dal bordo, segnaposto "Applicazioni" a
// 26 pt, "…" a destra), separatore, una riga di 7 categorie, poi la griglia: 7 colonne a passo
// 112,5 pt, righe a passo 102, icone da 61 pt, nomi da 14 pt troncati in coda; in cima una
// riga di app recenti separata da una linea. Barra di scorrimento sempre visibile.
//
// Una categoria diventa un gettone nel campo e la griglia mostra solo quella; scrivere filtra
// (dentro la categoria, o su tutte le app con la migliore per prima e un anello attorno).
// Esc toglie il testo, poi la categoria, poi torna a Faro.
//
// Cercando su tutte le app, se il primo risultato comincia con il testo il campo lo completa
// in linea ("cal" + "endario — Apri" in un rettangolo grigio) e mostra l'icona dell'app a destra.
//
// Non fatti (PA-60): selezione da tastiera nella griglia, menù del "…", ondeggiamento finale
// dell'apertura.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/shell_icon.dart';
import '../system/silica_icons.dart';
import 'search.dart';

const double launcherWidth = 844;
const double launcherHeight = 577;
const double launcherRadius = 25;
const double launcherHeader = 56;

/// Margine laterale di separatori, capsule e griglia.
const double launcherMargin = 20;

/// Zona delle capsule: 47 pt, capsule alte 26 a 10 pt dal separatore, 8 pt fra l'una e
/// l'altra, larghe almeno 85,5 pt o il testo più 9 pt per lato, raggio 9.
const double categoryZone = 47;
const double categoryTop = 10;
const double categoryHeight = 26;
const double categoryGap = 8;
const double categoryMinWidth = 85.5;
const double categoryPadding = 9;
const double categoryRadius = 9;
const double categoryText = 15;

/// Griglia: celle 112,5 × 92, icone 61 a 7 pt dalla cima della cella, nome a 75 pt (alto 18,
/// largo al più 110); prima cella a 11 pt dall'inizio dell'area; righe a passo 102; fra i
/// recenti e l'elenco una linea 13 pt sotto le celle, l'elenco 12 pt sotto la linea.
const int gridColumns = 7;
const double cellWidth = 112.5;
const double cellHeight = 92;
const double gridIcon = 61;
const double gridIconTop = 7;
const double gridLabelTop = 75;
const double gridLabelHeight = 18;
const double gridLabelWidth = 110;
const double gridLabelSize = 14;
const double gridTop = 11;
const double rowPitch = 102;
const double recentGap = 13;
const double recentAfter = 12;

/// Barra di scorrimento: guida opaca di 11 pt a 3 pt dal bordo, da 40 pt sotto l'inizio
/// dell'area a 49 pt dal fondo del pannello; estremi a semicerchio.
const double scrollbarWidth = 11;
const double scrollbarRight = 3;
const double scrollbarTop = 40;
const double scrollbarBottom = 49;

/// Anello del primo risultato: 4 pt attorno all'icona, raggio esterno 17.
const double ringWidth = 4;
const double ringRadius = 17;

/// Gettone della categoria nel campo: alto 31, raggio 10, testo a 6 pt dal bordo e 11 dopo.
const double tokenHeight = 31;
const double tokenRadius = 10;

const Color launcherText = Color(0xFF16171C);
const Color launcherPlaceholder = Color(0xFF494A4F);
const Color launcherLine = Color(0x20000000);

/// `#d9dadf` sopra il riempimento `#c8c9ce`: bianco al ~31 %.
const Color categoryFill = Color(0x4FFFFFFF);
const Color categoryTextColor = Color(0xFF5A5B60);

/// `#afb0b5` sopra `#c8c9ce`: come la riga selezionata di Faro.
const Color tokenFill = Color(0x20000000);
const Color tokenText = Color(0xFF0D0E13);
const Color scrollTrack = Color(0xFFF2F2F2);
const Color scrollThumb = Color(0xFF808080);

/// Completamento in linea: rettangolo alto 32 centrato nella testata (12 pt dalla cima), raggio 8, `#a2a3a8` sopra
/// `#c8c9ce` (nero al ~19 %); testo `#232429`. Anteprima dell'app: 27 pt, a 18 pt dal "…".
const double completionHeight = 32;
const double completionRadius = 8;
const Color completionFill = Color(0x30000000);
const Color completionText = Color(0xFF232429);
const double previewSide = 27;
const double previewRight = 60;

/// Il suffisso da completare: il resto del nome del primo risultato, se comincia con il
/// testo scritto (e non c'è una categoria).
String? completionFor(String query, List<DesktopEntry> results) {
  final q = query.trimLeft();
  final first = results.firstOrNull;
  if (q.isEmpty || first == null) return null;
  if (!first.name.toLowerCase().startsWith(q.toLowerCase())) return null;
  return '${first.name.substring(q.length)} — Apri';
}

/// `#838488 → #747579` sopra `#c8c9ce`.
const Color ringColor = Color(0x80404145);

/// Le categorie del launcher e le categorie freedesktop che contengono, dalla prima.
const List<(String, List<String>)> launcherCategories = [
  ('Produttività e finanza', ['Office', 'Finance', 'ProjectManagement']),
  ('Utility', ['Utility', 'System', 'Settings', 'Accessibility']),
  ('Creatività', ['Graphics', 'Audio', 'Video', 'AudioVideo', 'Development']),
  ('Svago', ['Game', 'Player', 'Music']),
  ('Social', ['Chat', 'InstantMessaging', 'Email', 'Network']),
  (
    'Informazione e lettura',
    ['Education', 'Science', 'News', 'Viewer', 'Documentation'],
  ),
];
const String otherCategory = 'Altro';

List<String> get categoryNames => [
  for (final (name, _) in launcherCategories) name,
  otherCategory,
];

/// La categoria del launcher di un'app: la prima che corrisponde, o "Altro".
String categoryOf(DesktopEntry entry) {
  for (final (name, members) in launcherCategories) {
    if (entry.categories.any(members.contains)) return name;
  }
  return otherCategory;
}

int _byName(DesktopEntry a, DesktopEntry b) =>
    a.name.toLowerCase().compareTo(b.name.toLowerCase());

/// Le app della griglia: in ordine alfabetico, nella categoria se c'è; con del testo, dentro
/// una categoria si filtra in ordine alfabetico, su tutte le app la migliore va prima.
List<DesktopEntry> launcherApps(
  Iterable<DesktopEntry> entries, {
  String? category,
  String query = '',
}) {
  final inCategory = [
    for (final e in entries)
      if (category == null || categoryOf(e) == category) e,
  ];
  if (query.trim().isEmpty) return inCategory..sort(_byName);
  final found = searchApps(inCategory, query);
  if (category != null) found.sort(_byName);
  return found;
}

/// Le app recenti (id dal più recente), da ~/.local/state/silica/recent-apps.json.
class RecentApps {
  RecentApps({Map<String, String>? env}) : _env = env ?? Platform.environment;

  final Map<String, String> _env;
  static const int limit = gridColumns;

  File get _file {
    final home = _env['HOME'] ?? '';
    final state = _env['XDG_STATE_HOME'] ?? '$home/.local/state';
    return File('$state/silica/recent-apps.json');
  }

  List<String> load() {
    try {
      final json = jsonDecode(_file.readAsStringSync());
      if (json is List) return [for (final id in json.take(limit)) '$id'];
    } on FileSystemException {
      // Mai salvato.
    } on FormatException {
      // File rovinato: si riparte.
    }
    return const [];
  }

  void add(String id) {
    final list = [
      id,
      ...load().where((other) => other != id),
    ].take(limit).toList();
    try {
      _file.parent.createSync(recursive: true);
      _file.writeAsStringSync(jsonEncode(list));
    } on FileSystemException {
      // Senza stato scrivibile i recenti non si ricordano.
    }
  }
}

IconSource? iconOf(DesktopEntry entry) {
  if (silicaIconFor(entry) case final layered?) return LayeredSource(layered);
  if (findIcon(entry.icon) case final path?) return FlatSource(path);
  return null;
}

class Launcher extends StatefulWidget {
  const Launcher({
    super.key,
    required this.entries,
    required this.recents,
    required this.onLaunch,
    required this.onBack,
  });

  final Map<String, DesktopEntry> entries;

  /// Gli id delle app recenti, dalla più recente.
  final List<String> recents;
  final ValueChanged<DesktopEntry> onLaunch;

  /// Esc senza testo né categoria: si torna a Faro.
  final VoidCallback onBack;

  @override
  State<Launcher> createState() => LauncherState();
}

class LauncherState extends State<Launcher>
    with SingleTickerProviderStateMixin {
  final TextEditingController _query = TextEditingController();
  final FocusNode _focus = FocusNode();
  final ScrollController _scroll = ScrollController();
  String? _category;

  /// Il cambio di categoria: la griglia sparisce subito e torna in dissolvenza dopo un vuoto
  /// (valore del controller = ms dal cambio).
  late final AnimationController _fade = AnimationController(
    vsync: this,
    upperBound: 260,
    value: 260,
    duration: const Duration(milliseconds: 260),
  )..addListener(() => setState(() {}));

  String? get category => _category;

  @override
  void initState() {
    super.initState();
    _query.addListener(() => setState(() {}));
    _scroll.addListener(() => setState(() {}));
    WidgetsBinding.instance.addPostFrameCallback((_) => _focus.requestFocus());
  }

  @override
  void dispose() {
    _fade.dispose();
    _query.dispose();
    _focus.dispose();
    _scroll.dispose();
    super.dispose();
  }

  void chooseCategory(String? name) {
    _fade.forward(from: 0);
    setState(() {
      _category = name;
      _query.clear();
    });
    if (_scroll.hasClients) _scroll.jumpTo(0);
  }

  KeyEventResult _key(FocusNode node, KeyEvent event, List<DesktopEntry> apps) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    switch (event.logicalKey) {
      case LogicalKeyboardKey.escape:
        if (_query.text.isNotEmpty) {
          _query.clear();
        } else if (_category != null) {
          chooseCategory(null);
        } else {
          widget.onBack();
        }
      case LogicalKeyboardKey.backspace
          when _query.text.isEmpty && _category != null:
        chooseCategory(null);
      case LogicalKeyboardKey.enter || LogicalKeyboardKey.numpadEnter:
        if (_query.text.trim().isNotEmpty && apps.isNotEmpty)
          widget.onLaunch(apps.first);
      default:
        return KeyEventResult.ignored;
    }
    return KeyEventResult.handled;
  }

  /// Opacità della griglia dopo un cambio di categoria (vuoto di ~75 ms, poi ~155 ms).
  double _gridOpacity() => categoryFade(_fade.value);

  @override
  Widget build(BuildContext context) {
    final searching = _query.text.trim().isNotEmpty;
    final apps = launcherApps(
      widget.entries.values,
      category: _category,
      query: _query.text,
    );
    final recents = [
      if (!searching && _category == null)
        for (final id in widget.recents)
          if (widget.entries[id] case final entry?) entry,
    ];
    final browsing = !searching && _category == null;
    final areaTop = launcherHeader + 1 + (browsing ? categoryZone : 0);
    final areaHeight = launcherHeight - areaTop;
    return SizedBox(
      width: launcherWidth,
      height: launcherHeight,
      child: Stack(
        children: [
          Positioned(
            left: 0,
            top: 0,
            right: 0,
            height: launcherHeader,
            child: _header(apps),
          ),
          Positioned(
            left: launcherMargin,
            right: launcherMargin,
            top: launcherHeader,
            height: 1,
            child: const ColoredBox(color: launcherLine),
          ),
          if (browsing)
            Positioned(
              left: launcherMargin,
              top: launcherHeader + 1 + categoryTop,
              right: launcherMargin,
              height: categoryHeight,
              child: _categories(),
            ),
          Positioned(
            left: 0,
            top: areaTop,
            right: 0,
            height: areaHeight,
            child: Opacity(
              opacity: _gridOpacity(),
              child: _grid(
                apps,
                recents,
                areaHeight,
                ringFirst: _completion(apps) != null,
              ),
            ),
          ),
          // Con la griglia scorsa compare un secondo separatore sopra l'area.
          if (browsing && _scroll.hasClients && _scroll.offset > 0)
            Positioned(
              left: launcherMargin,
              right: launcherMargin,
              top: areaTop - 1,
              height: 1,
              child: const ColoredBox(color: launcherLine),
            ),
        ],
      ),
    );
  }

  String? _completion(List<DesktopEntry> apps) =>
      _category == null ? completionFor(_query.text, apps) : null;

  Widget _header(List<DesktopEntry> apps) {
    final token = _category;
    final completion = _completion(apps);
    final typed = completion == null
        ? 0.0
        : (TextPainter(
            text: TextSpan(
              text: _query.text,
              style: uiText(size: 26, color: launcherText),
            ),
            textDirection: TextDirection.ltr,
          )..layout()).width;
    final preview = completion == null ? null : iconOf(apps.first);
    return Focus(
      onKeyEvent: (node, event) => _key(node, event, apps),
      child: Stack(
        children: [
          Positioned(
            left: 22,
            top: (launcherHeader - 22) / 2,
            width: 25,
            height: 22,
            child: GestureDetector(
              onTap: widget.onBack,
              child: const CustomPaint(
                painter: AppsGlyphPainter(launcherPlaceholder),
              ),
            ),
          ),
          Positioned(
            left: 61,
            top: 0,
            bottom: 0,
            right: 61,
            child: Row(
              children: [
                if (token != null) ...[
                  Container(
                    height: tokenHeight,
                    padding: const EdgeInsets.only(left: 6, right: 11),
                    alignment: Alignment.center,
                    decoration: const BoxDecoration(
                      color: tokenFill,
                      borderRadius: BorderRadius.all(
                        Radius.circular(tokenRadius),
                      ),
                    ),
                    child: Text(
                      token,
                      style: uiText(size: 26, color: tokenText),
                    ),
                  ),
                  const SizedBox(width: 8),
                ],
                Expanded(
                  child: Stack(
                    alignment: Alignment.centerLeft,
                    children: [
                      if (_query.text.isEmpty && token == null)
                        IgnorePointer(
                          child: Padding(
                            padding: const EdgeInsets.only(left: 2),
                            child: Text(
                              'Applicazioni',
                              style: uiText(
                                size: 26,
                                color: launcherPlaceholder,
                              ),
                            ),
                          ),
                        ),
                      if (completion != null)
                        Positioned(
                          left: typed + 1,
                          top: 0,
                          bottom: 0,
                          child: IgnorePointer(
                            child: Center(
                              child: Container(
                                height: completionHeight,
                                padding: const EdgeInsets.only(
                                  left: 1,
                                  right: 6,
                                ),
                                alignment: Alignment.center,
                                decoration: const BoxDecoration(
                                  color: completionFill,
                                  borderRadius: BorderRadius.all(
                                    Radius.circular(completionRadius),
                                  ),
                                ),
                                child: Text(
                                  completion,
                                  maxLines: 1,
                                  style: uiText(
                                    size: 26,
                                    color: completionText,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                      EditableText(
                        controller: _query,
                        focusNode: _focus,
                        style: uiText(size: 26, color: launcherText),
                        cursorColor: launcherText,
                        cursorWidth: 2,
                        cursorHeight: 31,
                        backgroundCursorColor: const Color(0x00000000),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          if (preview != null)
            Positioned(
              right: previewRight,
              top: (launcherHeader - previewSide) / 2,
              width: previewSide,
              height: previewSide,
              child: ShellIcon(preview, side: previewSide, dark: false),
            ),
          // "…": tre punti 4 × 3 a passo 7, l'ultimo a 23 pt dal bordo destro.
          Positioned(
            right: 23,
            top: (launcherHeader - 3) / 2,
            width: 18,
            height: 3,
            child: const CustomPaint(painter: _DotsPainter()),
          ),
        ],
      ),
    );
  }

  Widget _categories() {
    // Nomi più lunghi del previsto (altre lingue): la riga si stringe invece di uscire.
    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.centerLeft,
      child: _categoryRow(),
    );
  }

  Widget _categoryRow() {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final (i, name) in categoryNames.indexed) ...[
          if (i > 0) const SizedBox(width: categoryGap),
          GestureDetector(
            onTap: () => chooseCategory(name),
            child: Container(
              height: categoryHeight,
              constraints: const BoxConstraints(minWidth: categoryMinWidth),
              padding: const EdgeInsets.symmetric(horizontal: categoryPadding),
              alignment: Alignment.center,
              decoration: const BoxDecoration(
                color: categoryFill,
                borderRadius: BorderRadius.all(Radius.circular(categoryRadius)),
              ),
              child: Text(
                name,
                maxLines: 1,
                style: uiText(size: categoryText, color: categoryTextColor),
              ),
            ),
          ),
        ],
      ],
    );
  }

  Widget _grid(
    List<DesktopEntry> apps,
    List<DesktopEntry> recents,
    double areaHeight, {
    required bool ringFirst,
  }) {
    final layout = GridLayout(recents: recents.length, apps: apps.length);
    final contentHeight = layout.height;
    final children = <Widget>[
      for (final (i, entry) in recents.indexed)
        _cell(entry, layout.recentCell(i), ring: false),
      if (recents.isNotEmpty)
        Positioned(
          left: launcherMargin,
          top: layout.separatorY,
          width: gridColumns * cellWidth,
          height: 1,
          child: const ColoredBox(color: launcherLine),
        ),
      for (final (i, entry) in apps.indexed)
        _cell(entry, layout.cell(i), ring: ringFirst && i == 0),
    ];
    return Stack(
      children: [
        Positioned.fill(
          child: ScrollConfiguration(
            behavior: const _NoBars(),
            child: SingleChildScrollView(
              controller: _scroll,
              child: SizedBox(
                height: contentHeight,
                child: Stack(clipBehavior: Clip.none, children: children),
              ),
            ),
          ),
        ),
        Positioned(
          right: scrollbarRight,
          top: scrollbarTop,
          width: scrollbarWidth,
          bottom: scrollbarBottom,
          child: CustomPaint(
            painter: LauncherScrollbar(
              visible: areaHeight,
              content: contentHeight,
              offset: _scroll.hasClients ? _scroll.offset : 0,
            ),
          ),
        ),
      ],
    );
  }

  Widget _cell(DesktopEntry entry, Offset at, {required bool ring}) {
    final icon = iconOf(entry);
    final iconLeft = (cellWidth - gridIcon) / 2;
    return Positioned(
      left: at.dx,
      top: at.dy,
      width: cellWidth,
      height: cellHeight,
      child: GestureDetector(
        onTap: () => widget.onLaunch(entry),
        child: Stack(
          clipBehavior: Clip.none,
          children: [
            if (ring)
              Positioned(
                left: iconLeft - ringWidth,
                top: gridIconTop - ringWidth,
                width: gridIcon + 2 * ringWidth,
                height: gridIcon + 2 * ringWidth,
                child: DecoratedBox(
                  decoration: BoxDecoration(
                    border: Border.all(color: ringColor, width: ringWidth),
                    borderRadius: const BorderRadius.all(
                      Radius.circular(ringRadius),
                    ),
                  ),
                ),
              ),
            Positioned(
              left: iconLeft,
              top: gridIconTop,
              width: gridIcon,
              height: gridIcon,
              child: icon == null
                  ? const SizedBox()
                  : ShellIcon(icon, side: gridIcon, dark: false),
            ),
            Positioned(
              left: (cellWidth - gridLabelWidth) / 2,
              top: gridLabelTop,
              width: gridLabelWidth,
              height: gridLabelHeight,
              child: Text(
                entry.name,
                textAlign: TextAlign.center,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: uiText(size: gridLabelSize, color: launcherText),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Opacità della griglia `ms` dopo il cambio di categoria (misurata sul video).
double categoryFade(double ms) {
  const table = [
    (75.0, 0.0),
    (80.0, 0.059),
    (107.0, 0.322),
    (120.0, 0.483),
    (134.0, 0.616),
    (147.0, 0.731),
    (174.0, 0.869),
    (187.0, 0.901),
    (200.0, 0.934),
    (227.0, 0.986),
    (240.0, 0.994),
    (254.0, 1.0),
  ];
  if (ms <= table.first.$1) return 0;
  for (var i = 1; i < table.length; i++) {
    final (t1, a1) = table[i];
    if (ms <= t1) {
      final (t0, a0) = table[i - 1];
      return a0 + (a1 - a0) * (ms - t0) / (t1 - t0);
    }
  }
  return 1;
}

/// Dove stanno le celle nell'area di scorrimento.
class GridLayout {
  const GridLayout({required this.recents, required this.apps});

  final int recents;
  final int apps;

  double get _listTop =>
      recents == 0 ? gridTop : gridTop + cellHeight + recentGap + recentAfter;

  double get separatorY => gridTop + cellHeight + recentGap;

  Offset recentCell(int i) => Offset(launcherMargin + i * cellWidth, gridTop);

  Offset cell(int i) => Offset(
    launcherMargin + (i % gridColumns) * cellWidth,
    _listTop + (i ~/ gridColumns) * rowPitch,
  );

  double get height {
    final rows = (apps + gridColumns - 1) ~/ gridColumns;
    return rows == 0
        ? _listTop
        : _listTop + (rows - 1) * rowPitch + cellHeight + gridTop;
  }
}

/// La barra di scorrimento: guida sempre, cursore solo se il contenuto non entra.
class LauncherScrollbar extends CustomPainter {
  const LauncherScrollbar({
    required this.visible,
    required this.content,
    required this.offset,
  });

  final double visible;
  final double content;
  final double offset;

  /// Il cursore nella guida alta `track`, o null se non serve.
  Rect? thumb(double track) {
    if (content <= visible || track <= 0) return null;
    final height = (track * visible / content).clamp(scrollbarWidth, track);
    final travel = track - height;
    final top = travel * (offset / (content - visible)).clamp(0.0, 1.0);
    return Rect.fromLTWH(0, top, scrollbarWidth, height);
  }

  @override
  void paint(Canvas canvas, Size size) {
    const radius = Radius.circular(scrollbarWidth / 2);
    canvas.drawRRect(
      RRect.fromRectAndRadius(Offset.zero & size, radius),
      Paint()..color = scrollTrack,
    );
    if (thumb(size.height) case final rect?) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(rect, radius),
        Paint()..color = scrollThumb,
      );
    }
  }

  @override
  bool shouldRepaint(LauncherScrollbar old) =>
      old.visible != visible || old.content != content || old.offset != offset;
}

class _NoBars extends ScrollBehavior {
  const _NoBars();

  @override
  Widget buildScrollbar(
    BuildContext context,
    Widget child,
    ScrollableDetails details,
  ) => child;
}

class _DotsPainter extends CustomPainter {
  const _DotsPainter();

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = launcherPlaceholder;
    for (var i = 0; i < 3; i++) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromLTWH(i * 7.0, 0, 4, 3),
          const Radius.circular(1.5),
        ),
        paint,
      );
    }
  }

  @override
  bool shouldRepaint(_DotsPainter old) => false;
}

/// Il glifo delle app: quattro quadrati arrotondati.
class AppsGlyphPainter extends CustomPainter {
  const AppsGlyphPainter(this.color);

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2;
    final w = (size.width - 6) / 2;
    final h = (size.height - 6) / 2;
    for (final dx in [1.0, w + 5]) {
      for (final dy in [1.0, h + 5]) {
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTWH(dx, dy, w, h),
            const Radius.circular(3),
          ),
          paint,
        );
      }
    }
  }

  @override
  bool shouldRepaint(AppsGlyphPainter old) => old.color != color;
}
