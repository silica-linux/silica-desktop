// The app launcher: Faro in another mode, from plan/reference/launcher.md (light
// appearance). An 844 × 577 pt panel with its top at 30 % of the screen, radius 25, Faro's
// material. A 56 pt header (apps icon 22 pt from the edge, "Applicazioni" placeholder at
// 26 pt, "…" on the right), a separator, a row of 7 categories, then the grid: 7 columns at a
// 112.5 pt pitch, rows at a 102 pitch, 61 pt icons, 14 pt names truncated at the end; at the
// top a row of recent apps separated by a line. The scroll bar is always visible.
//
// A category becomes a token in the field and the grid shows only that one; typing filters
// (inside the category, or over all apps with the best first and a ring around it).
// Esc removes the text, then the category, then goes back to Faro.
//
// Searching all apps, if the first result starts with the text the field completes it
// inline ("cal" + "endario — Apri" in a grey rectangle) and shows the app icon on the right.
//
// Not done (PA-60): keyboard selection in the grid, the "…" menu, the final wobble of the
// opening.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../l10n/l10n.dart';
import '../system/desktop_entries.dart';
import '../system/icon_lookup.dart';
import '../system/silica_icons.dart';
import 'search.dart';

const double launcherWidth = 844;
const double launcherHeight = 577;
const double launcherRadius = 25;
const double launcherHeader = 56;

/// Side margin of separators, capsules and grid.
const double launcherMargin = 20;

/// Capsule area: 47 pt, capsules 26 tall 10 pt below the separator, 8 pt apart, at least
/// 85.5 pt wide or the text plus 9 pt per side, radius 9.
const double categoryZone = 47;
const double categoryTop = 10;
const double categoryHeight = 26;
const double categoryGap = 8;
const double categoryMinWidth = 85.5;
const double categoryPadding = 9;
const double categoryRadius = 9;
const double categoryText = 15;

/// Grid: 112.5 × 92 cells, 61 icons 7 pt from the top of the cell, name at 75 pt (18 tall,
/// at most 110 wide); first cell 11 pt from the start of the area; rows at a 102 pitch;
/// between the recents and the list a line 13 pt below the cells, the list 12 pt below it.
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

/// Scroll bar: an opaque 11 pt track 3 pt from the edge, from 40 pt below the start of the
/// area to 49 pt from the bottom of the panel; semicircular ends.
const double scrollbarWidth = 11;
const double scrollbarRight = 3;
const double scrollbarTop = 40;
const double scrollbarBottom = 49;

/// Ring of the first result: 4 pt around the icon, outer radius 17.
const double ringWidth = 4;
const double ringRadius = 17;

/// Category token in the field: 31 tall, radius 10, text 6 pt from the edge and 11 after.
const double tokenHeight = 31;
const double tokenRadius = 10;

const Color launcherText = Color(0xFF16171C);
const Color launcherPlaceholder = Color(0xFF494A4F);
const Color launcherLine = Color(0x20000000);

/// `#d9dadf` over the `#c8c9ce` fill: white at ~31 %.
const Color categoryFill = Color(0x4FFFFFFF);
const Color categoryTextColor = Color(0xFF5A5B60);

/// `#afb0b5` over `#c8c9ce`: like Faro's selected row.
const Color tokenFill = Color(0x20000000);
const Color tokenText = Color(0xFF0D0E13);
const Color scrollTrack = Color(0xFFF2F2F2);
const Color scrollThumb = Color(0xFF808080);

/// Inline completion: a rectangle 32 tall centered in the header (12 pt from the top), radius 8, `#a2a3a8` over
/// `#c8c9ce` (black at ~19 %); text `#232429`. App preview: 27 pt, 18 pt from the "…".
const double completionHeight = 32;
const double completionRadius = 8;
const Color completionFill = Color(0x30000000);
const Color completionText = Color(0xFF232429);
const double previewSide = 27;
const double previewRight = 60;

/// The suffix to complete: the rest of the first result's name, if it starts with the typed
/// text (and there is no category). The field shows it followed by "— Open".
String? completionFor(String query, List<DesktopEntry> results) {
  final q = query.trimLeft();
  final first = results.firstOrNull;
  if (q.isEmpty || first == null) return null;
  if (!first.name.toLowerCase().startsWith(q.toLowerCase())) return null;
  return first.name.substring(q.length);
}

/// `#838488 → #747579` over `#c8c9ce`.
const Color ringColor = Color(0x80404145);

/// The colors of Faro and the launcher over thick glass, light and dark (spotlight.md,
/// launcher.md). In dark the fill is `#717276`: the backgrounds measured over it become
/// translucent white (chosen row and token `#949599` → 25 %, capsules `#828387` → 12 %,
/// completion `#97989c` → 27 %, ring `#abacb0 → #98999d` → ~35 %); texts are solid.
/// The shadow in dark is ~1.7 times denser (at contact −68 against −39 below): the opacity
/// is *estimated* by scaling the light one.
@immutable
class FaroColors {
  const FaroColors({
    required this.text,
    required this.selectedText,
    required this.placeholder,
    required this.subtitle,
    required this.selection,
    required this.categoryFill,
    required this.categoryText,
    required this.tokenText,
    required this.completionFill,
    required this.completionText,
    required this.ring,
    required this.track,
    required this.thumb,
    required this.shadowScale,
  });

  final Color text;
  final Color selectedText;
  final Color placeholder;
  final Color subtitle;

  /// Chosen row, separators and category token.
  final Color selection;
  final Color categoryFill;
  final Color categoryText;
  final Color tokenText;
  final Color completionFill;
  final Color completionText;
  final Color ring;
  final Color track;
  final Color thumb;

  /// Factor on the opacity of the panel shadows.
  final double shadowScale;

  static const FaroColors light = FaroColors(
    text: launcherText,
    selectedText: Color(0xFF000003),
    placeholder: launcherPlaceholder,
    subtitle: Color(0xFF88898E),
    selection: launcherLine,
    categoryFill: Color(0x4FFFFFFF),
    categoryText: categoryTextColor,
    tokenText: Color(0xFF0D0E13),
    completionFill: Color(0x30000000),
    completionText: Color(0xFF232429),
    ring: ringColor,
    track: scrollTrack,
    thumb: scrollThumb,
    shadowScale: 1,
  );

  static const FaroColors dark = FaroColors(
    text: Color(0xFFFFFFFF),
    selectedText: Color(0xFFFFFFFF),
    placeholder: Color(0xFFEDEEF2),
    subtitle: Color(0xFFB2B3B7),
    selection: Color(0x41FFFFFF),
    categoryFill: Color(0x1FFFFFFF),
    categoryText: Color(0xFFFEFFFF),
    tokenText: Color(0xFFFFFFFF),
    completionFill: Color(0x45FFFFFF),
    completionText: Color(0xFFFFFFFF),
    ring: Color(0x58FFFFFF),
    track: Color(0xFF141414),
    thumb: Color(0xFF7C7C7C),
    shadowScale: 1.7,
  );

  static FaroColors of(BuildContext context) =>
      MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark
      ? dark
      : light;

  /// The shadow `color` with the opacity of this appearance.
  Color shadow(Color color) =>
      color.withValues(alpha: (color.a * shadowScale).clamp(0.0, 1.0));
}

/// The launcher categories (stable ids) and the freedesktop categories they contain, from
/// the first.
const List<(String, List<String>)> launcherCategories = [
  ('productivity', ['Office', 'Finance', 'ProjectManagement']),
  ('utilities', ['Utility', 'System', 'Settings', 'Accessibility']),
  ('creativity', ['Graphics', 'Audio', 'Video', 'AudioVideo', 'Development']),
  ('entertainment', ['Game', 'Player', 'Music']),
  ('social', ['Chat', 'InstantMessaging', 'Email', 'Network']),
  ('reading', ['Education', 'Science', 'News', 'Viewer', 'Documentation']),
];
const String otherCategory = 'other';

List<String> get categoryNames => [
  for (final (name, _) in launcherCategories) name,
  otherCategory,
];

/// The shown name of a category id.
String categoryLabel(AppLocalizations l, String id) => switch (id) {
  'productivity' => l.catProductivity,
  'utilities' => l.catUtilities,
  'creativity' => l.catCreativity,
  'entertainment' => l.catEntertainment,
  'social' => l.catSocial,
  'reading' => l.catReading,
  _ => l.catOther,
};

/// The launcher category of an app: the first that matches, or the other one.
String categoryOf(DesktopEntry entry) {
  for (final (name, members) in launcherCategories) {
    if (entry.categories.any(members.contains)) return name;
  }
  return otherCategory;
}

int _byName(DesktopEntry a, DesktopEntry b) =>
    a.name.toLowerCase().compareTo(b.name.toLowerCase());

/// The grid apps: alphabetical, in the category if there is one; with text, inside a
/// category it filters alphabetically, over all apps the best goes first.
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

/// The recent apps (ids from the most recent), from ~/.local/state/silica/recent-apps.json.
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
      // Never saved.
    } on FormatException {
      // Broken file: start over.
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
      // Without a writable state the recents are not remembered.
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

  /// The ids of the recent apps, from the most recent.
  final List<String> recents;
  final ValueChanged<DesktopEntry> onLaunch;

  /// Esc with no text and no category: back to Faro.
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

  /// The category change: the grid disappears at once and fades back in after a gap
  /// (controller value = ms since the change).
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

  /// Grid opacity after a category change (a gap of ~75 ms, then ~155 ms).
  double _gridOpacity() => categoryFade(_fade.value);

  FaroColors get _c => FaroColors.of(context);

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
            child: ColoredBox(color: _c.selection),
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
          // With the grid scrolled a second separator appears above the area.
          if (browsing && _scroll.hasClients && _scroll.offset > 0)
            Positioned(
              left: launcherMargin,
              right: launcherMargin,
              top: areaTop - 1,
              height: 1,
              child: ColoredBox(color: _c.selection),
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
              style: uiText(size: 26, color: _c.text),
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
              child: CustomPaint(painter: AppsGlyphPainter(_c.placeholder)),
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
                    decoration: BoxDecoration(
                      color: _c.selection,
                      borderRadius: BorderRadius.all(
                        Radius.circular(tokenRadius),
                      ),
                    ),
                    child: Text(
                      categoryLabel(l10nOf(context), token),
                      style: uiText(size: 26, color: _c.tokenText),
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
                              l10nOf(context).launcherPlaceholder,
                              style: uiText(size: 26, color: _c.placeholder),
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
                                decoration: BoxDecoration(
                                  color: _c.completionFill,
                                  borderRadius: BorderRadius.all(
                                    Radius.circular(completionRadius),
                                  ),
                                ),
                                child: Text(
                                  l10nOf(context).completionOpen(completion),
                                  maxLines: 1,
                                  style: uiText(
                                    size: 26,
                                    color: _c.completionText,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ),
                      EditableText(
                        controller: _query,
                        focusNode: _focus,
                        style: uiText(size: 26, color: _c.text),
                        cursorColor: _c.text,
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
              child: SilicaIcon(preview, side: previewSide, dark: false),
            ),
          // "…": three 4 × 3 dots at a 7 pitch, the last one 23 pt from the right edge.
          Positioned(
            right: 23,
            top: (launcherHeader - 3) / 2,
            width: 18,
            height: 3,
            child: CustomPaint(painter: _DotsPainter(_c.placeholder)),
          ),
        ],
      ),
    );
  }

  Widget _categories() {
    // Names longer than expected (other languages): the row shrinks instead of overflowing.
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
              decoration: BoxDecoration(
                color: _c.categoryFill,
                borderRadius: const BorderRadius.all(
                  Radius.circular(categoryRadius),
                ),
              ),
              child: Text(
                categoryLabel(l10nOf(context), name),
                maxLines: 1,
                style: uiText(size: categoryText, color: _c.categoryText),
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
          child: ColoredBox(color: _c.selection),
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
              track: _c.track,
              thumbColor: _c.thumb,
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
                    border: Border.all(color: _c.ring, width: ringWidth),
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
                  : SilicaIcon(icon, side: gridIcon, dark: false),
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
                style: uiText(size: gridLabelSize, color: _c.text),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Grid opacity `ms` after the category change (measured on the video).
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

/// Where the cells are in the scroll area.
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

/// The scroll bar: always the track, the thumb only if the content does not fit.
class LauncherScrollbar extends CustomPainter {
  const LauncherScrollbar({
    required this.visible,
    required this.content,
    required this.offset,
    this.track = scrollTrack,
    this.thumbColor = scrollThumb,
  });

  final Color track;
  final Color thumbColor;

  final double visible;
  final double content;
  final double offset;

  /// The thumb in a track `track` tall, or null if not needed.
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
      Paint()..color = track,
    );
    if (thumb(size.height) case final rect?) {
      canvas.drawRRect(
        RRect.fromRectAndRadius(rect, radius),
        Paint()..color = thumbColor,
      );
    }
  }

  @override
  bool shouldRepaint(LauncherScrollbar old) =>
      old.visible != visible ||
      old.content != content ||
      old.offset != offset ||
      old.track != track ||
      old.thumbColor != thumbColor;
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
  const _DotsPainter(this.color);
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = color;
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
  bool shouldRepaint(_DotsPainter old) => old.color != color;
}

/// The apps glyph: four rounded squares.
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
