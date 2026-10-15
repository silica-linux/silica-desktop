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
import '../system/glass_profile.dart';
import '../system/silica_symbol.dart';
import 'dock.dart' show DockLabel, labelFontSize, labelHeight, labelPadding;
import 'glass_groups.dart';
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
///
/// Every frame of the opening only moves, turns and fades the items: their contents (icon,
/// label with its glass) are built once per stack and kept as the same widgets, each behind
/// its own repaint boundary, so a frame neither resolves icons nor measures names again.
class FanStack extends StatefulWidget {
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
  State<FanStack> createState() => _FanStackState();
}

class _FanStackState extends State<FanStack> {
  /// While the stack moves its contents are pictures taken once ([SnapshotWidget]): a frame
  /// of the opening or closing draws images, not the icons' vector layers and blurred ground
  /// shadows again at every new position and scale. At rest they are drawn live.
  final SnapshotController _snapshot = SnapshotController();

  @override
  void dispose() {
    _snapshot.dispose();
    super.dispose();
  }

  /// Per slot: the label and the icon, built once for these entries and this appearance.
  List<(Widget, Widget)> _items = const [];
  Object? _itemsKey;

  List<(Widget, Widget)> _contents(BuildContext context) {
    final dark = _dark(context);
    final l10n = l10nOf(context);
    final text = MenuColors.of(context).text;
    final key = Object.hash(identityHashCode(widget.entries), dark, l10n, text);
    if (key == _itemsKey) return _items;
    _itemsKey = key;
    final shown = widget.entries.take(fanMax).toList();
    final more = widget.entries.length - shown.length;
    return _items = [
      for (var i = 0; i <= shown.length; i++)
        () {
          final side = fanSlot(i).side;
          final entry = i < shown.length ? shown[i] : null;
          final label = entry == null
              ? (more > 0 ? l10n.stackMoreInFolder(more) : l10n.openFolder)
              : fileName(entry.path);
          final icon = entry == null ? null : fileIcon(entry);
          return (
            RepaintBoundary(
              child: SnapshotWidget(
                controller: _snapshot,
                child: _label(label, text, i),
              ),
            ),
            RepaintBoundary(
              child: SnapshotWidget(
                controller: _snapshot,
                child: SizedBox(
                  key: ValueKey('fan icon $i'),
                  width: side,
                  height: side,
                  child: entry == null
                      ? OpenFolderButton(
                          side: side,
                          dark: dark,
                          group: DockGlass.fanButton,
                        )
                      : icon == null
                      ? const SizedBox()
                      : SilicaIcon(icon, side: side, dark: false),
                ),
              ),
            ),
          );
        }(),
    ];
  }

  @override
  Widget build(BuildContext context) {
    final items = _contents(context);
    final shown = math.min(widget.entries.length, fanMax);
    final progress = widget.progress;
    _snapshot.allowSnapshotting = progress < 1;
    final fade = progress.clamp(0.0, 1.0);
    final labels = fade * ((progress - 0.7) / 0.3).clamp(0.0, 1.0);
    final children = <Widget>[];
    for (var i = 0; i <= shown && i < items.length; i++) {
      final slot = fanSlot(i);
      // The column comes out of the icon: the pitch grows with the progress.
      final center =
          widget.anchor +
          Offset(slot.center.dx * progress, slot.center.dy * progress);
      final side = slot.side;
      final entry = i < shown ? widget.entries[i] : null;
      final (label, icon) = items[i];
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
              // The current callbacks, not those of the build that made the contents.
              onTap: () =>
                  entry == null ? widget.onOpenFolder() : widget.onOpen(entry),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  // Each item fades on its own: a single layer over the whole fan would be
                  // as large as the column on every frame.
                  Opacity(opacity: labels, child: label),
                  const SizedBox(width: 6),
                  Opacity(opacity: fade, child: icon),
                ],
              ),
            ),
          ),
        ),
      );
    }
    return Stack(clipBehavior: Clip.none, children: children);
  }

  /// The item name: the glass of the Dock label (`thick`, the menus' shadow), as wide as the
  /// text (at most 360 pt), without the tip, in a glass group of its own.
  Widget _label(String text, Color color, int index) {
    final style = uiText(size: labelFontSize, color: color);
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
        group: DockGlass.fanLabel(index),
      ),
    );
  }
}

/// The glyph of a glass control: dark in light, white in dark (dock-stacks.md, close button).
Color glassGlyph(bool dark) =>
    dark ? const Color(0xFFFFFFFF) : const Color(0xFF19191B);

/// A glass control: a `ProfiledGlass` (control) circle of `diameter` with a silica-icons symbol
/// on top, in its own glass `group` (it never merges with the panel or the labels around it).
/// The glyph is `glyph` of the diameter (`provisional`, DOCK-FAN has been requested).
class GlassSymbolButton extends StatelessWidget {
  const GlassSymbolButton({
    super.key,
    required this.symbol,
    required this.diameter,
    required this.group,
    this.dark = false,
    this.glyph = 0.5,
    this.material = GlassMaterial.regular,
  });

  final String symbol;
  final double diameter;
  final int group;
  final bool dark;
  final double glyph;
  final GlassMaterial material;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: diameter,
    height: diameter,
    child: ProfiledGlass(
      profile: ShellGlassProfile.control,
      radius: diameter / 2,
      material: material,
      group: group,
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

/// The button the stack's Dock icon becomes while it is open (dock-stacks.md, "Close button of
/// the open stack"): an app tile, the same body as every app icon in the Dock (the `side`
/// square with the icons' superellipse corners, their drop shadow and bright rim), filled with
/// the measured gray gradient and its top hairline, with the silica-icons `chevron-down`.
/// Painted by the shell like the icons, not compositor glass: it cannot merge with anything.
class StackCloseButton extends StatelessWidget {
  const StackCloseButton({super.key, required this.side, required this.dark});

  final double side;
  final bool dark;

  static const String symbol = 'chevron-down';

  /// The symbol's box over the side: the measured chevron is 19 pt wide on a 51 pt square
  /// (0.37), and silica-icons' chevron-down spans 0.61 of its box.
  static const double glyph = 0.6;

  /// Top to bottom (measured, linear).
  static const List<Color> fillLight = [Color(0xFFADADB0), Color(0xFF8B8B90)];
  static const List<Color> fillDark = [Color(0xFF5C5C5F), Color(0xFF313232)];

  /// The 1 pt line along the top edge.
  static const Color hairlineLight = Color(0xFFC2C2C5);
  static const Color hairlineDark = Color(0xFF8B8C8E);

  @override
  Widget build(BuildContext context) => SizedBox(
    width: side,
    height: side,
    child: CustomPaint(
      painter: StackCloseTilePainter(dark: dark),
      child: Center(
        child: SilicaSymbol(
          symbol,
          size: side * glyph,
          color: glassGlyph(dark),
        ),
      ),
    ),
  );
}

/// The close tile's body: drop shadow, gradient, hairline and rim of an app tile.
class StackCloseTilePainter extends CustomPainter {
  const StackCloseTilePainter({required this.dark});

  final bool dark;

  @override
  void paint(Canvas canvas, Size size) {
    final side = size.shortestSide;
    final body = Offset.zero & Size.square(side);
    final path = iconBodyPath(body);
    // The app icons' drop shadow (silica_ui: 3 % of the side below, blurred as much).
    canvas.drawPath(
      iconBodyPath(body.shift(Offset(0, side * 0.03))),
      Paint()
        ..color = const Color(0x55000000)
        ..maskFilter = MaskFilter.blur(BlurStyle.normal, side * 0.03),
    );
    canvas.drawPath(
      path,
      Paint()
        ..shader = LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: dark ? StackCloseButton.fillDark : StackCloseButton.fillLight,
        ).createShader(body),
    );
    canvas
      ..save()
      ..clipPath(path)
      ..drawRect(
        Rect.fromLTWH(0, 0, side, 1),
        Paint()
          ..color = dark
              ? StackCloseButton.hairlineDark
              : StackCloseButton.hairlineLight,
      )
      ..restore();
    // The app icons' rim: lighter at the top, fading at the bottom.
    final width = math.max(1.0, side / 64);
    canvas.drawPath(
      iconBodyPath(body.deflate(width / 2)),
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = width
        ..shader = LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: dark
              ? const [Color(0x40FFFFFF), Color(0x0FFFFFFF)]
              : const [Color(0x80FFFFFF), Color(0x1AFFFFFF)],
        ).createShader(body),
    );
  }

  @override
  bool shouldRepaint(StackCloseTilePainter old) => old.dark != dark;
}

/// "Open folder" (in the file manager): a glass circle with the silica-icons `arrow-up-right`,
/// in the glass `group` of its view (above the grid panel, apart from the fan labels).
class OpenFolderButton extends StatelessWidget {
  const OpenFolderButton({
    super.key,
    required this.side,
    required this.group,
    this.dark = false,
  });

  /// The box of the item (the circle is [diameterRatio] of it).
  final double side;
  final int group;
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
      group: group,
    ),
  );
}

/// The grid: a panel with the title, above the icon with the tip pointing at it.
/// `tip` is the apex of the tip (5 pt above the Dock).
///
/// Like the fan, the panel's contents (title, cells, names truncated to fit) are built once
/// per stack and kept behind a repaint boundary: a frame of the opening only scales and fades.
class GridStack extends StatefulWidget {
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
  State<GridStack> createState() => _GridStackState();
}

class _GridStackState extends State<GridStack> {
  /// While the stack moves its contents are pictures taken once ([SnapshotWidget]): a frame
  /// of the opening or closing draws images, not the icons' vector layers and blurred ground
  /// shadows again at every new position and scale. At rest they are drawn live.
  final SnapshotController _snapshot = SnapshotController();

  @override
  void dispose() {
    _snapshot.dispose();
    super.dispose();
  }

  Widget? _content;
  Object? _contentKey;

  Widget _contents(BuildContext context) {
    final dark = _dark(context);
    final l10n = l10nOf(context);
    final key = Object.hash(
      identityHashCode(widget.entries),
      widget.title,
      dark,
      l10n,
    );
    final cached = _content;
    if (cached != null && key == _contentKey) return cached;
    _contentKey = key;
    return _content = RepaintBoundary(
      child: SnapshotWidget(
        controller: _snapshot,
        child: DefaultTextStyle.merge(
          style: TextStyle(color: dark ? gridLabelTextDark : gridLabelText),
          child: Stack(
            children: [
              Positioned(
                left: 0,
                right: 0,
                top: 9,
                child: Text(
                  widget.title,
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    fontFamily: uiFontFamily,
                    package: 'silica_ui',
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    color: dark ? stackTextDark : stackText,
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
                      for (final entry in widget.entries) _cell(entry),
                      _openCell(l10n.openFolder, dark),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final rect = widget.rect;
    final tipX = widget.tipX;
    final p = widget.progress.clamp(0.0, 1.0);
    _snapshot.allowSnapshotting = widget.progress < 1;
    return Positioned.fromRect(
      rect: rect,
      child: Transform.scale(
        scale: 0.2 + 0.8 * p,
        alignment: Alignment(
          (tipX / rect.width) * 2 - 1,
          1 + 2 * gridTipHeight / rect.height,
        ),
        child: Opacity(
          opacity: p,
          child: OuterShadow(
            radius: gridRadius,
            blurRadius: 30,
            offset: const Offset(0, 8),
            child: ProfiledGlass(
              profile: ShellGlassProfile.panel,
              radius: gridRadius,
              material: GlassMaterial.thick,
              group: DockGlass.gridPanel,
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
              child: _contents(context),
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
      // The current callback, not the one of the build that made the cell.
      onTap: () => widget.onOpen(entry),
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
            _name(
              middleEllipsis(
                fileName(entry.path),
                gridLabelStyle,
                gridLabelWidth,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _name(String text) => Positioned(
    left: (gridCell - gridLabelWidth) / 2,
    width: gridLabelWidth,
    top: 0,
    child: Baseline(
      baseline: gridLabelBaseline,
      baselineType: TextBaseline.alphabetic,
      child: SizedBox(
        width: gridLabelWidth,
        child: Text(
          text,
          textAlign: TextAlign.center,
          maxLines: 1,
          softWrap: false,
          style: gridLabelStyle,
        ),
      ),
    ),
  );

  Widget _openCell(String label, bool dark) => GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTap: () => widget.onOpenFolder(),
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
            child: OpenFolderButton(
              side: gridIcon,
              dark: dark,
              group: DockGlass.gridButton,
            ),
          ),
          _name(label),
        ],
      ),
    ),
  );
}

/// The open stack under a Dock menu (the menu opened on the stack's own icon). Ossido draws all
/// the glass of the Dock's surface under all of its content, so the menu's glass alone cannot
/// cover the stack's icons and names: the stack's content is cut out where the menu's panels
/// (`menu`, in the stack's coordinates) and its tip lie, and the menu's glass group, above the
/// stack's (glass_groups.dart), covers the stack's glass there. Without a menu nothing is cut.
class StackUnderMenu extends StatelessWidget {
  const StackUnderMenu({super.key, required this.menu, required this.child});

  /// The menu's panels and its tip.
  final List<Rect> menu;
  final Widget child;

  @override
  Widget build(BuildContext context) => ClipPath(
    // The same widgets with or without the menu: the stack keeps its state.
    clipBehavior: menu.isEmpty ? Clip.none : Clip.antiAlias,
    clipper: MenuHoleClipper(menu),
    child: child,
  );
}

/// Everything but the menu's panels (with the menus' corner radius) and its tip.
class MenuHoleClipper extends CustomClipper<Path> {
  const MenuHoleClipper(this.holes);

  final List<Rect> holes;

  @override
  Path getClip(Size size) {
    // The stack reaches outside its box (the fan's top items above the surface): far enough.
    final all = Path()..addRect((Offset.zero & size).inflate(4096));
    if (holes.isEmpty) return all;
    final cut = Path();
    for (final hole in holes) {
      cut.addRRect(
        RRect.fromRectAndRadius(
          hole,
          const Radius.circular(MenuMetrics.radius),
        ),
      );
    }
    return Path.combine(PathOperation.difference, all, cut);
  }

  @override
  bool shouldReclip(MenuHoleClipper old) => !_same(old.holes, holes);

  static bool _same(List<Rect> a, List<Rect> b) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i] != b[i]) return false;
    }
    return true;
  }
}

/// The body of a Dock menu laid out as `rect` (body and tip, as the Dock places it).
Rect menuBody(Rect rect) => Rect.fromLTRB(
  rect.left,
  rect.top,
  rect.right,
  rect.bottom - MenuMetrics.tipHeight,
);

/// The box of a Dock menu's tip whose apex is `apex`.
Rect menuTipRect(Offset apex) => Rect.fromLTRB(
  apex.dx - MenuMetrics.tipBase / 2,
  apex.dy - MenuMetrics.tipDepth,
  apex.dx + MenuMetrics.tipBase / 2,
  apex.dy,
);

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
