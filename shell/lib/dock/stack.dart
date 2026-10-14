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
import 'stack_settings.dart';

const int fanMax = 15;
const double fanPitch = 64.6;
const double fanFirstSide = 65.8;
const double fanSideStep = 1.1;
const double fanAboveZone = 7;

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

/// The tip towards the icon: base ~22, ~8 tall (misc.md). As in the menus, the rounded apex
/// and the smooth union widen the base by ~14 pt.
const double gridTipBase = 22;
const double gridLeft = 17;

/// 16 pt to the edge plus 1 of rim: 2 columns make 290 pt, 4 make 546.
const double gridRight = 17;
const double gridTop = 32;
const double gridBottom = 11;
const double gridRadius = 25;
const double gridTipHeight = 8;
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
    final folder = resolveFolderIcon(entry.path);
    return folder == null ? null : FolderSource(folder);
  }
  final file = resolveFileIcon(entry.path);
  return file == null ? null : FileSource(file);
}

/// The fan geometry for item `i` (from the bottom): center and side, relative to the center
/// of the stack icon in x and to the top of the Dock area in y; rotation.
({Offset center, double side, double angle}) fanSlot(int i) {
  final side = fanFirstSide + fanSideStep * i;
  final left = fanShift[i.clamp(0, fanShift.length - 1)];
  final x = fanFirstOffset - fanFirstSide / 2 + left + side / 2;
  // The tops go at a constant pitch; the first has its bottom 7 pt above the area.
  final top = -fanAboveZone - fanFirstSide - fanPitch * i;
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
          ? (more > 0 ? 'Altri $more nella cartella' : 'Apri la cartella')
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
                  Opacity(
                    opacity: labels,
                    child: _label(_dark(context), label),
                  ),
                  const SizedBox(width: 6),
                  SizedBox(
                    width: side,
                    height: side,
                    child: entry == null
                        ? CustomPaint(painter: _OpenFolderPainter())
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

  /// Label as wide as the text (at most 360 pt), like the Dock ones.
  Widget _label(bool dark, String text) => ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: 360),
    child: DecoratedBox(
      decoration: BoxDecoration(
        color: dark ? stackLabelFillDark : stackLabelFill,
        borderRadius: const BorderRadius.all(Radius.circular(11)),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
        child: Text(
          text,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: uiText(
            size: 12,
            color: dark ? stackLabelTextDark : stackLabelText,
          ),
        ),
      ),
    ),
  );
}

/// "Apri la cartella": an arrow in a circle.
class _OpenFolderPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final c = size.center(Offset.zero);
    final r = size.shortestSide * 0.32;
    canvas.drawCircle(c, r, Paint()..color = const Color(0xFFFFFFFF));
    final paint = Paint()
      ..color = const Color(0xFF3A3A3C)
      ..style = PaintingStyle.stroke
      ..strokeWidth = r * 0.14
      ..strokeCap = StrokeCap.round;
    canvas
      ..drawLine(
        c + Offset(-r * 0.35, r * 0.35),
        c + Offset(r * 0.35, -r * 0.35),
        paint,
      )
      ..drawLine(
        c + Offset(-r * 0.05, -r * 0.35),
        c + Offset(r * 0.35, -r * 0.35),
        paint,
      )
      ..drawLine(
        c + Offset(r * 0.35, -r * 0.35),
        c + Offset(r * 0.35, r * 0.05),
        paint,
      );
  }

  @override
  bool shouldRepaint(_OpenFolderPainter old) => false;
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
    final (columns, _) = gridShape(entries.length);
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
                tip: GlassTip(
                  x: tipX,
                  width: gridTipBase - 14,
                  height: gridTipHeight + 2,
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
                            _openCell(columns),
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

  Widget _openCell(int columns) => GestureDetector(
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
            child: CustomPaint(painter: _OpenFolderPainter()),
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
                  'Apri la cartella',
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

/// The "stack" icon of a folder in the Dock (d06-icon-pila-10-light): the first file in front,
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
