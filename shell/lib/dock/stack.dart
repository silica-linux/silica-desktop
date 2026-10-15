// The Dock's folder stacks (plan/reference/dock-stacks.md, misc.md "Folder stack").
//
// Automatic view: fan up to 15 files, grid beyond (the real threshold is between 11 and 40,
// not measured).
// - Fan: from the bottom, one item every 64.6 pt, side from 65.8 to 82.3 pt going up, the
//   arc bending to the right with the measured offsets and the icons rotated accordingly;
//   labels on the left; the first item has its bottom edge 7 pt above the Dock area; at most
//   15 files, the sixteenth place opens the folder. Opening ~240 ms (the column comes out of
//   the icon and stretches, then the labels), closing ~210 ms.
// - Grid: a panel with the title and 128 pt cells, ⌈√(n + 1)⌉ columns, margins 17 / 16 at
//   the sides, 32 at the top (title), 11 at the bottom, radius 25, tip towards the icon 5 pt
//   above the Dock; it scales up from the tip in ~240 ms, closes in ~150 ms.
// A click on a file closes the stack and opens it.

import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:silica_ui/silica_ui.dart';

import '../faro/faro.dart' show OuterShadow;
import '../l10n/l10n.dart';
import '../system/folder_icons.dart';
import '../system/folder_style.dart';
import '../system/silica_symbol.dart';
import 'dock.dart' show DockLabel, labelFontSize, labelHeight, labelPadding;
import 'stack_settings.dart';

const int fanMax = 15;
const double fanPitch = 64.6;
const double fanFirstSide = 65.8;
const double fanSideStep = 1.1;
const double fanAboveZone = 7;

/// How much further apart the items are placed along the arc than measured (both the pitch and
/// the rightward shift scale; the angle and the sides do not). The measured column (pitch 64.6
/// for sides of 65.8-82.3) leaves the icons touching; DOCK-FAN has been requested for the real
/// spacing. `provisional`.
const double fanSpread = 1.25;

/// Rightward offset of the left edge of each item relative to the first (pt).
const List<double> fanShift = [
  0,
  1.4,
  4.7,
  9.1,
  15.5,
  22.9,
  31.3,
  41.7,
  54.2,
  66.6,
  81.1,
  97.6,
  115.1,
  134.6,
  155.2,
  177.8,
];

/// Center of the first item relative to the center of the stack icon: 1673.1 + 32.9 − 1703.7.
const double fanFirstOffset = 2.3;

const double gridCell = 128;

/// The icon in the cell: folder 92 × 74 19 pt from the top (a 92 box from y 10).
const double gridIcon = 92;
const double gridIconTop = 10;

/// The icon canvas in the cell (silica-icons 0.2: a file's side is the canvas): 100 pt from
/// y 6, so the folder (0.922 × 0.734 of the canvas, top at 132/1024) is again 92 × 74 at 19 pt.
const double gridIconCanvas = 100;
const double gridCanvasTop = 6;

/// The label: one line, capitals from y 109 (baseline at ~118), at most ~104 wide.
const double gridLabelBaseline = 118;
const double gridLabelWidth = 104;
const Color gridLabelText = Color(0xFF212122);

/// The color comes from the grid's [DefaultTextStyle] (light or dark).
TextStyle get gridLabelStyle => const TextStyle(
  fontFamily: uiFontFamily,
  package: 'silica_ui',
  fontSize: 12,
  height: 1,
);

/// The tip towards the icon is the menus' glass tip (`MenuMetrics.tipBase` x `tipDepth`, the
/// body edge `gridTipHeight` from the apex), see [GridStack].
const double gridLeft = 17;

/// 16 pt to the edge plus 1 of rim: 2 columns make 290 pt, 4 make 546.
const double gridRight = 17;
const double gridTop = 32;
const double gridBottom = 11;
const double gridRadius = 25;
const double gridTipHeight = MenuMetrics.tipHeight;
const double gridTipAboveDock = 5;

const Duration stackOpen = Duration(milliseconds: 240);
const Duration fanClose = Duration(milliseconds: 210);
const Duration gridClose = Duration(milliseconds: 150);

const Color stackText = Color(0xFF1E1E1F);
const Color stackLabelFill = Color(0xFFBDBEC3);
const Color stackLabelText = Color(0xFF1C1C1D);

/// In dark the stacks are not measured (dock-stacks.md, "Not observed"): *estimated* like the
/// Dock labels in dark (dock.md: body `#7f8086`, text `#f9f9f9`) and white texts on the
/// dark glass.
const Color gridLabelTextDark = Color(0xFFF9F9F9);
const Color stackTextDark = Color(0xFFFFFFFF);
const Color stackLabelFillDark = Color(0xFF7F8086);
const Color stackLabelTextDark = Color(0xFFF9F9F9);

bool _dark(BuildContext context) =>
    MediaQuery.maybePlatformBrightnessOf(context) == Brightness.dark;

enum StackView { fan, grid }

/// Automatic view for `count` files.
StackView automaticView(int count) =>
    count <= fanMax ? StackView.fan : StackView.grid;

/// The files of the folder without the hidden ones, in the chosen order: by name; by date from
/// the most recent (like download stacks); by kind (extension, then name). On Linux the "date
/// added" is the status change (ctime: moving or copying a file into the folder updates it)
/// and there is no reliable creation date: the modification is used.
List<FileSystemEntity> stackEntries(
  String path, {
  StackSort sort = StackSort.name,
}) {
  try {
    final list = Directory(path)
        .listSync()
        .where(
          (e) => !e.uri.pathSegments
              .where((s) => s.isNotEmpty)
              .last
              .startsWith('.'),
        )
        .toList();
    int byName(FileSystemEntity a, FileSystemEntity b) =>
        naturalCompare(fileName(a.path), fileName(b.path));
    final stats = {for (final e in list) e.path: e.statSync()};
    DateTime when(FileSystemEntity e) {
      final stat = stats[e.path];
      if (stat == null) return DateTime(0);
      return sort == StackSort.added ? stat.changed : stat.modified;
    }

    String extension(FileSystemEntity e) {
      if (e is Directory) return '';
      final name = fileName(e.path);
      final dot = name.lastIndexOf('.');
      return dot <= 0 ? '' : name.substring(dot + 1).toLowerCase();
    }

    list.sort(
      (a, b) => switch (sort) {
        StackSort.name => byName(a, b),
        StackSort.kind => () {
          final c = extension(a).compareTo(extension(b));
          return c != 0 ? c : byName(a, b);
        }(),
        _ => () {
          final c = when(b).compareTo(when(a));
          return c != 0 ? c : byName(a, b);
        }(),
      },
    );
    return list;
  } on FileSystemException {
    return const [];
  }
}

/// "Natural" comparison: digits count as numbers (document-2 before document-10),
/// case-insensitive.
int naturalCompare(String a, String b) {
  final parts = RegExp(r'\d+|\D+');
  final pa = [for (final m in parts.allMatches(a.toLowerCase())) m[0] ?? ''];
  final pb = [for (final m in parts.allMatches(b.toLowerCase())) m[0] ?? ''];
  for (var i = 0; i < pa.length && i < pb.length; i++) {
    final na = int.tryParse(pa[i]);
    final nb = int.tryParse(pb[i]);
    final c = na != null && nb != null
        ? na.compareTo(nb)
        : pa[i].compareTo(pb[i]);
    if (c != 0) return c;
  }
  return pa.length.compareTo(pb.length);
}

String fileName(String path) {
  final parts = path.split('/').where((p) => p.isNotEmpty);
  return parts.isEmpty ? path : parts.last;
}

/// The silica-icons icon of a file or a folder: special folders with their symbol, files
/// with the silica-icons rules (MIME type, extension, generic file). No badges in the Dock
/// (icon-format.md).
IconSource? fileIcon(FileSystemEntity entry) {
  if (entry is Directory) {
    // With its colour and symbol (folder_style.dart).
    final folder = styledFolderIcon(entry.path);
    if (folder == null) return null;
    // A folder with something inside is drawn full (symbol and tint on top as before).
    return FolderSource(
      folderHasItems(entry.path) ? fullFolder(folder) : folder,
    );
  }
  final file = resolveFileIcon(entry.path);
  return file == null ? null : FileSource(file);
}

/// The fan geometry for item `i` (from the bottom): center and side, relative to the center
/// of the stack icon in x and to the top of the Dock area in y; rotation.
({Offset center, double side, double angle}) fanSlot(int i) {
  final side = fanFirstSide + fanSideStep * i;
  final left = fanShift[i.clamp(0, fanShift.length - 1)];
  final x = fanFirstOffset - fanFirstSide / 2 + left * fanSpread + side / 2;
  // The tops go at a constant pitch; the first has its bottom 7 pt above the area.
  final top = -fanAboveZone - fanFirstSide - fanPitch * fanSpread * i;
  final step = i == 0 ? fanShift[1] : fanShift[i] - fanShift[i - 1];
  return (
    center: Offset(x, top + side / 2),
    side: side,
    angle: math.atan(step / fanPitch),
  );
}

/// Grid columns and rows for `count` files (+1 to open the folder).
(int, int) gridShape(int count) {
  final cells = count + 1;
  final columns = math.sqrt(cells).ceil().clamp(1, 12);
  return (columns, (cells / columns).ceil());
}

Size gridPanelSize(int count, double maxHeight) {
  final (columns, rows) = gridShape(count);
  final height = gridTop + rows * gridCell + gridBottom;
  return Size(
    gridLeft + columns * gridCell + gridRight,
    // Never a reversed range: with little room the minimum row wins.
    height.clamp(
      gridTop + gridCell + gridBottom,
      math.max(gridTop + gridCell + gridBottom, maxHeight),
    ),
  );
}

/// The fan above the icon: `anchor` is the center of the icon in x and the top of the Dock
/// area in y; `progress` 0–1 opens and closes.
class FanStack extends StatelessWidget {
  const FanStack({
    super.key,
    required this.entries,
    required this.anchor,
    required this.progress,
    required this.onOpen,
    required this.onOpenFolder,
  });

  final List<FileSystemEntity> entries;
  final Offset anchor;
  final double progress;
  final ValueChanged<FileSystemEntity> onOpen;
  final VoidCallback onOpenFolder;

  @override
  Widget build(BuildContext context) {
    final shown = entries.take(fanMax).toList();
    final more = entries.length - shown.length;
    final children = <Widget>[];
    final labels = ((progress - 0.7) / 0.3).clamp(0.0, 1.0);
    for (var i = 0; i <= shown.length; i++) {
      final slot = fanSlot(i);
      // The column comes out of the icon: the pitch grows with the progress.
      final center =
          anchor + Offset(slot.center.dx * progress, slot.center.dy * progress);
      final side = slot.side;
      final entry = i < shown.length ? shown[i] : null;
      final label = entry == null
          ? (more > 0
                ? l10nOf(context).stackMoreInFolder(more)
                : l10nOf(context).openFolder)
          : fileName(entry.path);
      final icon = entry == null ? null : fileIcon(entry);
      children.add(
        Positioned(
          left: center.dx - side / 2 - 400,
          top: center.dy - side / 2,
          width: side + 400,
          height: side,
          child: Transform.rotate(
            angle: slot.angle * progress,
            alignment: Alignment(1 - side / (side + 400), 0),
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTap: () => entry == null ? onOpenFolder() : onOpen(entry),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  Opacity(opacity: labels, child: _label(context, label)),
                  const SizedBox(width: 6),
                  SizedBox(
                    key: ValueKey('fan icon $i'),
                    width: side,
                    height: side,
                    child: entry == null
                        ? OpenFolderButton(side: side, dark: _dark(context))
                        : icon == null
                        ? const SizedBox()
                        : SilicaIcon(icon, side: side, dark: false),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    }
    return Opacity(
      opacity: progress.clamp(0.0, 1.0),
      child: Stack(clipBehavior: Clip.none, children: children),
    );
  }

  /// The item name: the glass of the Dock label (`thick`, the menus' shadow), as wide as the
  /// text (at most 360 pt), without the tip.
  Widget _label(BuildContext context, String text) {
    final style = uiText(
      size: labelFontSize,
      color: MenuColors.of(context).text,
    );
    final painter = TextPainter(
      text: TextSpan(text: text, style: style),
      textDirection: TextDirection.ltr,
      maxLines: 1,
    )..layout();
    final width = math.min(
      360.0,
      painter.width.ceilToDouble() + 2 * labelPadding,
    );
    painter.dispose();
    return SizedBox(
      width: width,
      height: labelHeight,
      child: DockLabel(
        text: text,
        style: style.copyWith(overflow: TextOverflow.ellipsis),
        tip: false,
      ),
    );
  }
}

/// The glyph of a glass control: dark in light, white in dark (dock-stacks.md, close button).
Color glassGlyph(bool dark) =>
    dark ? const Color(0xFFFFFFFF) : const Color(0xFF19191B);

/// A glass control: a `LiquidGlass` circle of `diameter` with a silica-icons symbol on top.
/// The glyph is `glyph` of the diameter (`provisional`, DOCK-FAN has been requested).
class GlassSymbolButton extends StatelessWidget {
  const GlassSymbolButton({
    super.key,
    required this.symbol,
    required this.diameter,
    this.dark = false,
    this.glyph = 0.5,
    this.material = GlassMaterial.regular,
  });

  final String symbol;
  final double diameter;
  final bool dark;
  final double glyph;
  final GlassMaterial material;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: diameter,
    height: diameter,
    child: LiquidGlass(
      radius: diameter / 2,
      material: material,
      interactive: false,
      child: Center(
        child: SilicaSymbol(
          symbol,
          size: diameter * glyph,
          color: glassGlyph(dark),
        ),
      ),
    ),
  );
}

/// The button the stack's Dock icon becomes while it is open: a glass circle with the
/// silica-icons `chevron-down`, in the `side` box of the icon. The circle is 0.85 of the side
/// (`provisional`; the measured button is a 51 x 52 rounded square on a 54 pt cell).
class StackCloseButton extends StatelessWidget {
  const StackCloseButton({super.key, required this.side, required this.dark});

  final double side;
  final bool dark;

  static const String symbol = 'chevron-down';
  static const double diameterRatio = 0.85;

  @override
  Widget build(BuildContext context) => Center(
    child: GlassSymbolButton(
      symbol: symbol,
      diameter: side * diameterRatio,
      dark: dark,
      glyph: 0.55,
    ),
  );
}

/// "Open folder" (in the file manager): a glass circle with the silica-icons `arrow-up-right`.
class OpenFolderButton extends StatelessWidget {
  const OpenFolderButton({super.key, required this.side, this.dark = false});

  /// The box of the item (the circle is [diameterRatio] of it).
  final double side;
  final bool dark;

  static const String symbol = 'arrow-up-right';
  static const double diameterRatio = 0.7;

  @override
  Widget build(BuildContext context) => Center(
    child: GlassSymbolButton(
      symbol: symbol,
      diameter: side * diameterRatio,
      dark: dark,
      glyph: 0.55,
    ),
  );
}

/// The grid: a panel with the title, above the icon with the tip pointing at it.
/// `tip` is the apex of the tip (5 pt above the Dock).
class GridStack extends StatelessWidget {
  const GridStack({
    super.key,
    required this.title,
    required this.entries,
    required this.rect,
    required this.tipX,
    required this.progress,
    required this.onOpen,
    required this.onOpenFolder,
  });

  final String title;
  final List<FileSystemEntity> entries;

  /// The panel body in the area.
  final Rect rect;

  /// The apex of the tip, from the left edge of the panel.
  final double tipX;
  final double progress;
  final ValueChanged<FileSystemEntity> onOpen;
  final VoidCallback onOpenFolder;

  @override
  Widget build(BuildContext context) {
    final p = progress.clamp(0.0, 1.0);
    return Positioned.fromRect(
      rect: rect,
      child: Transform.scale(
        scale: 0.2 + 0.8 * p,
        alignment: Alignment(
          (tipX / rect.width) * 2 - 1,
          1 + 2 * gridTipHeight / rect.height,
        ),
        child: DefaultTextStyle.merge(
          style: TextStyle(
            color: _dark(context) ? gridLabelTextDark : gridLabelText,
          ),
          child: Opacity(
            opacity: p,
            child: OuterShadow(
              radius: gridRadius,
              blurRadius: 30,
              offset: const Offset(0, 8),
              child: LiquidGlass(
                radius: gridRadius,
                material: GlassMaterial.thick,
                interactive: false,
                // The same glass tip as the menus (MenuPanel): the measured outline picked by
                // this rectangle, the body edge 8 pt from the apex.
                tip: GlassTip(
                  x: tipX.clamp(
                    gridRadius + MenuMetrics.tipBase / 2,
                    math.max(
                      gridRadius + MenuMetrics.tipBase / 2,
                      rect.width - gridRadius - MenuMetrics.tipBase / 2,
                    ),
                  ),
                  width: MenuMetrics.tipBase,
                  height: MenuMetrics.tipDepth,
                ),
                child: Stack(
                  children: [
                    Positioned(
                      left: 0,
                      right: 0,
                      top: 9,
                      child: Text(
                        title,
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          fontFamily: uiFontFamily,
                          package: 'silica_ui',
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: _dark(context) ? stackTextDark : stackText,
                        ),
                      ),
                    ),
                    Positioned(
                      left: gridLeft,
                      right: gridRight,
                      top: gridTop,
                      bottom: gridBottom,
                      child: SingleChildScrollView(
                        child: Wrap(
                          children: [
                            for (final entry in entries) _cell(entry),
                            _openCell(context),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// A 128 cell (misc.md › Folder stack): the icon of ~92 centered with the top of the
  /// folder at 19 pt (box from y 10), the name on one line truncated in the middle with the
  /// capitals from y 109, `#212122`.
  Widget _cell(FileSystemEntity entry) {
    final icon = fileIcon(entry);
    return GestureDetector(
      onTap: () => onOpen(entry),
      child: SizedBox(
        width: gridCell,
        height: gridCell,
        child: Stack(
          children: [
            Positioned(
              key: ValueKey('icon ${entry.path}'),
              left: (gridCell - gridIconCanvas) / 2,
              top: gridCanvasTop,
              width: gridIconCanvas,
              height: gridIconCanvas,
              child: icon == null
                  ? const SizedBox.shrink()
                  : SilicaIcon(icon, side: gridIconCanvas, dark: false),
            ),
            Positioned(
              left: (gridCell - gridLabelWidth) / 2,
              width: gridLabelWidth,
              top: 0,
              child: Baseline(
                baseline: gridLabelBaseline,
                baselineType: TextBaseline.alphabetic,
                child: SizedBox(
                  width: gridLabelWidth,
                  child: Text(
                    middleEllipsis(
                      fileName(entry.path),
                      gridLabelStyle,
                      gridLabelWidth,
                    ),
                    textAlign: TextAlign.center,
                    maxLines: 1,
                    softWrap: false,
                    style: gridLabelStyle,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _openCell(BuildContext context) => GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTap: onOpenFolder,
    child: SizedBox(
      width: gridCell,
      height: gridCell,
      child: Stack(
        children: [
          Positioned(
            left: (gridCell - gridIcon) / 2,
            top: gridIconTop,
            width: gridIcon,
            height: gridIcon,
            child: OpenFolderButton(side: gridIcon, dark: _dark(context)),
          ),
          Positioned(
            left: (gridCell - gridLabelWidth) / 2,
            width: gridLabelWidth,
            top: 0,
            child: Baseline(
              baseline: gridLabelBaseline,
              baselineType: TextBaseline.alphabetic,
              child: SizedBox(
                width: gridLabelWidth,
                child: Text(
                  l10nOf(context).openFolder,
                  textAlign: TextAlign.center,
                  maxLines: 1,
                  softWrap: false,
                  style: gridLabelStyle,
                ),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}

/// The "stack" icon of a folder in the Dock (dock-stacks.md › Icon in the Dock): the first file in front,
/// the next two behind, smaller (0.89 and 0.73) and with their tops 4 pt higher each, no
/// rotation. Measured on a 54 pt icon: front sheet 37 × 46, tops at +0, −4, −8.
class PileIcon extends StatelessWidget {
  const PileIcon({super.key, required this.sources, required this.side});

  /// From the first (in front) to the third.
  final List<IconSource> sources;
  final double side;

  static const List<double> scales = [1, 0.89, 0.73];

  @override
  Widget build(BuildContext context) {
    final unit = side / 54;
    // The front sheet: ~46 tall over 54, resting at the bottom (2 pt margin).
    final front = side * 0.88;
    final bottom = side - 2 * unit;
    return SizedBox(
      width: side,
      height: side,
      child: Stack(
        clipBehavior: Clip.none,
        children: [
          for (var i = math.min(sources.length, 3) - 1; i >= 0; i--)
            Positioned(
              left: (side - front * scales[i]) / 2,
              top: bottom - front - 4 * unit * i,
              width: front * scales[i],
              height: front * scales[i],
              // Measured with the sheet on the app body (d06-icon-pila-10).
              child: SilicaIcon(
                sources[i],
                side: front * scales[i],
                besideApps: true,
              ),
            ),
        ],
      ),
    );
  }
}

/// Truncates in the middle with "…" so that `text` fits in `width` (the grid labels).
String middleEllipsis(String text, TextStyle style, double width) {
  double measure(String s) {
    final painter = TextPainter(
      text: TextSpan(text: s, style: style),
      textDirection: TextDirection.ltr,
      maxLines: 1,
    )..layout();
    final w = painter.width;
    painter.dispose();
    return w;
  }

  if (measure(text) <= width) return text;
  final runes = text.runes.toList();
  var lo = 0;
  var hi = runes.length;
  var best = '…';
  while (lo <= hi) {
    final keep = (lo + hi) ~/ 2;
    final head = (keep + 1) ~/ 2;
    final tail = keep - head;
    final candidate =
        '${String.fromCharCodes(runes.take(head)).trimRight()}…'
        '${String.fromCharCodes(runes.skip(runes.length - tail)).trimLeft()}';
    if (measure(candidate) <= width) {
      best = candidate;
      lo = keep + 1;
    } else {
      hi = keep - 1;
    }
  }
  return best;
}
