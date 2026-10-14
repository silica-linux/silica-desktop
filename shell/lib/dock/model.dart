// Dock geometry and magnification, from plan/reference/dock.md (macOS 27, light appearance,
// sliders of the recordings: size 0.384, magnification 0.429). Everything in logical points,
// with the origin at the top left of the Dock surface (as wide as the screen, with its
// bottom edge on the screen edge).

import 'dart:ui' as ui;

import 'package:silica_ui/silica_ui.dart';

/// Pitch between app cells: 63 pt (AX).
const double cellPitch = 63;

/// Width of the separator cell: 28.35 pt (AX).
const double separatorCell = 28.35;

/// Background height: 79 pt solid (y 1356–1434).
const double backgroundHeight = 79;

/// Distance between the background and the bottom edge of the screen: 5 pt.
const double bottomGap = 5;

/// Corner radius of the background: a 22 pt circular arc.
const double backgroundRadius = 22;

/// Margin between the cells and the background edge, at the sides: the resting background
/// is 927 pt wide against 910.35 pt of cells (12 apps, separator, 2 items): ~8 pt per side.
const double sidePadding = 8;

/// Icon side at rest and distance from the top of the background: 48 pt, 16 pt.
const double iconSize = 48;
const double iconTop = 16;

/// How much the bottom edge of the magnified icon rises, per unit of scale above 1:
/// +7 pt at ×2.095, +1 pt at ×1.079 (measured to 1 pt).
const double iconLiftPerScale = 6.4;

/// Separator: a 1 pt line 59 pt tall, 10 pt margin in the background.
const double separatorHeight = 59;

/// Indicator of open apps: a 4 pt disc, center 6 pt above the bottom edge of the
/// background.
const double indicatorSize = 4;
const double indicatorAbove = 6;

/// Magnification at the recordings' setting: the cell under the pointer goes from 63 to
/// 68 pt (×1.0794). At the maximum of the slider: 132 pt (×2.0952).
const double defaultMagnification = 68 / 63;
const double maxMagnification = 132 / 63;

/// The active zone starts 5 pt above the background.
const double activeZoneAbove = 5;

/// Magnification profile f(d) as a function of the distance d (pt) between the pointer and
/// the center of the cell at rest: measured table, it vanishes between 208 and 224 pt.
const List<(double, double)> magnificationProfile = [
  (0, 1.000),
  (16, 0.991),
  (32, 0.964),
  (48, 0.921),
  (64, 0.861),
  (80, 0.786),
  (96, 0.698),
  (112, 0.597),
  (128, 0.485),
  (144, 0.365),
  (160, 0.240),
  (176, 0.131),
  (192, 0.055),
  (208, 0.011),
  (224, 0.000),
];

/// f(d) by linear interpolation of the table.
double profile(double distance) {
  final d = distance.abs();
  for (var i = 1; i < magnificationProfile.length; i++) {
    final (d1, f1) = magnificationProfile[i];
    if (d <= d1) {
      final (d0, f0) = magnificationProfile[i - 1];
      return f0 + (f1 - f0) * (d - d0) / (d1 - d0);
    }
  }
  return 0;
}

/// Curve of entering and leaving the active zone (equal within 0.01: here the mean of
/// the two), 3 cycles at maximum magnification: progress every 10 ms over ~160 ms.
const List<double> enterCurve = [
  0,
  0.001,
  0.017,
  0.067,
  0.112,
  0.206,
  0.294,
  0.386,
  0.481,
  0.578,
  0.672,
  0.759,
  0.837,
  0.902,
  0.955,
  0.988,
  1.0,
];

/// The entrance progress at a fraction `t` (0–1) of the duration.
double enterProgress(double t) {
  final x = t.clamp(0.0, 1.0) * (enterCurve.length - 1);
  final i = x.floor().clamp(0, enterCurve.length - 2);
  return enterCurve[i] + (enterCurve[i + 1] - enterCurve[i]) * (x - i);
}

/// Entrance duration: ~60 ms at 68 pt, ~160 ms at 132 pt; it grows with the amplitude (with
/// only two points it is interpolated linearly).
Duration enterDuration(double magnification) {
  final span =
      (magnification - defaultMagnification) /
      (maxMagnification - defaultMagnification);
  final ms = 60 + (160 - 60) * span.clamp(0.0, 1.0);
  return Duration(microseconds: (ms * 1000).round());
}

/// A Dock item: an app (or folder, trash) or the separator.
enum ItemKind { app, separator }

class Item {
  const Item.app(
    this.label, {
    this.running = false,
    this.iconPath,
    this.layered,
    this.command,
    this.windows = const [],
    this.id,
    this.pinned = false,
    this.folder,
    this.openFiles,
    this.trash = false,
    this.action,
    this.fixed = false,
  }) : kind = ItemKind.app,
       minimizedWindow = null,
       thumbnail = null;

  /// A minimized window (D-08): its thumbnail with the app icon in the corner.
  const Item.window(
    this.label, {
    required String window,
    this.thumbnail,
    this.iconPath,
    this.layered,
  }) : kind = ItemKind.app,
       running = false,
       command = null,
       windows = const [],
       id = null,
       pinned = false,
       folder = null,
       openFiles = null,
       trash = false,
       action = null,
       fixed = false,
       minimizedWindow = window;
  const Item.separator()
    : kind = ItemKind.separator,
      label = '',
      running = false,
      iconPath = null,
      layered = null,
      command = null,
      windows = const [],
      id = null,
      pinned = false,
      folder = null,
      openFiles = null,
      trash = false,
      action = null,
      fixed = false,
      minimizedWindow = null,
      thumbnail = null;

  final ItemKind kind;
  final String label;
  final bool running;

  /// The icon file (SVG or PNG); null: placeholder drawn in code.
  final String? iconPath;

  /// The layered silica-icons icon, preferred to [iconPath].
  final LayeredIcon? layered;

  /// The command to launch it (Exec without field codes), if it is an app.
  final String? command;

  /// The open windows of the app (identifiers of Ossido's list).
  final List<String> windows;

  /// The id of the .desktop entry (null for folders and trash).
  final String? id;

  /// Pinned in the Dock (it stays even when closed).
  final bool pinned;

  /// A folder shown as a stack (D-06): its path.
  final String? folder;

  /// Exec with field codes, to open the files dropped on the app (null: it takes none).
  final String? openFiles;

  /// The Trash: dropped files end up inside it.
  final bool trash;

  /// A shell command (engine.execute id) run on a click instead of launching [command]: the
  /// Apps item opens the Launcher.
  final String? action;

  /// Always first in the Dock, neither dragged nor removed: the file manager (#141).
  final bool fixed;

  /// Accepts files dragged from another app (D-05, MU-40).
  bool get acceptsFiles =>
      trash ||
      folder != null ||
      (openFiles != null && RegExp('%[fFuU]').hasMatch(openFiles!));

  /// The minimized window this item stands for (null for apps and folders).
  final String? minimizedWindow;

  /// Its thumbnail, if it has already arrived from Ossido.
  final ui.Image? thumbnail;

  /// The icon to draw; null: placeholder with the initial.
  IconSource? get icon => switch ((layered, iconPath)) {
    (final LayeredIcon l, _) => LayeredSource(l),
    (null, final String path) => FlatSource(path),
    _ => null,
  };

  double get restWidth =>
      kind == ItemKind.separator ? separatorCell : cellPitch;
}

/// The Dock layout at one moment: cells, background, icons.
class Layout {
  Layout._(this.cells, this.scales, this.background);

  /// Rectangle (x, width) of every cell, in the order of the items.
  final List<(double, double)> cells;

  /// Scale of every cell (1 at rest; the separator stays 1).
  final List<double> scales;

  /// The background: left, top, width, height.
  final (double, double, double, double) background;

  /// Center of cell `i`.
  double center(int i) => cells[i].$1 + cells[i].$2 / 2;
}

/// Computes the layout for a surface `width` wide and `height` tall, with the pointer at
/// `pointerX` (null: outside the active zone) and the magnification `amount` (0–1, the
/// progress of the entrance) up to `magnification`.
Layout layout(
  List<Item> items, {
  required double width,
  required double height,
  double? pointerX,
  double amount = 0,
  double magnification = defaultMagnification,
}) {
  final restTotal = items.fold<double>(0, (sum, item) => sum + item.restWidth);
  final restLeft = (width - restTotal) / 2;
  // Resting centers, from which the distance is measured.
  final restCenters = <double>[];
  var x = restLeft;
  for (final item in items) {
    restCenters.add(x + item.restWidth / 2);
    x += item.restWidth;
  }
  final scales = [
    for (var i = 0; i < items.length; i++)
      if (items[i].kind == ItemKind.separator || pointerX == null)
        1.0
      else
        1 + (magnification - 1) * profile(pointerX - restCenters[i]) * amount,
  ];
  // Lateral expansion: the point under the pointer stays still. Every cell gives its growth
  // to the side it is on relative to the pointer; the one under the pointer splits it
  // according to the pointer's position in the resting cell.
  var leftGrowth = 0.0;
  for (var i = 0; i < items.length; i++) {
    final extra = items[i].restWidth * (scales[i] - 1);
    if (extra == 0 || pointerX == null) continue;
    final cellLeft = restCenters[i] - items[i].restWidth / 2;
    final fraction = ((pointerX - cellLeft) / items[i].restWidth).clamp(
      0.0,
      1.0,
    );
    leftGrowth += extra * fraction;
  }
  final cells = <(double, double)>[];
  x = restLeft - leftGrowth;
  for (var i = 0; i < items.length; i++) {
    final w = items[i].restWidth * scales[i];
    cells.add((x, w));
    x += w;
  }
  final first = cells.first.$1;
  final last = cells.last.$1 + cells.last.$2;
  final top = height - bottomGap - backgroundHeight;
  return Layout._(cells, scales, (
    first - sidePadding,
    top,
    last - first + 2 * sidePadding,
    backgroundHeight,
  ));
}

/// The icon square of cell `i`: left, top, side.
(double, double, double) iconRect(Layout l, int i, double height) {
  final side = iconSize * l.scales[i];
  final restBottom = height - bottomGap - backgroundHeight + iconTop + iconSize;
  final bottom = restBottom - iconLiftPerScale * (l.scales[i] - 1);
  return (l.center(i) - side / 2, bottom - side, side);
}

/// The pointer is in the active zone: inside the resting background horizontally (x 816 no,
/// x 820 yes with the background from 816.5), from 5 pt above the background downwards.
bool inActiveZone(Layout rest, double x, double y, double height) {
  final top = height - bottomGap - backgroundHeight - activeZoneAbove;
  final (left, _, w, _) = rest.background;
  return y >= top && x >= left && x <= left + w;
}

/// Leaving upwards the zone stays active while the pointer is in the magnified cell
/// (hysteresis, dock.md D-01): up to the top of its icon minus this margin.
const double activeZoneHold = 6;

/// Stays in the zone: horizontally as on entering (the resting list), vertically up to the
/// top of the magnified cell under the pointer.
bool staysInActiveZone(
  Layout rest,
  Layout current,
  List<Item> items,
  double x,
  double y,
  double height,
) {
  final (left, _, w, _) = rest.background;
  if (x < left || x > left + w) return false;
  if (inActiveZone(rest, x, y, height)) return true;
  for (var i = 0; i < current.cells.length; i++) {
    final (cellLeft, cellWidth) = current.cells[i];
    if (x >= cellLeft && x < cellLeft + cellWidth) {
      final (_, top, _) = iconRect(current, i, height);
      return y >= top - activeZoneHold;
    }
  }
  return false;
}

/// Label (D-12): it appears on entering (full in ~40 ms), jumps immediately from one icon to
/// the next, on exit it stays ~40 ms and fades in ~250 ms.
const Duration labelAppear = Duration(milliseconds: 40);
const Duration labelHold = Duration(milliseconds: 40);
const Duration labelVanish = Duration(milliseconds: 250);

/// Launch bounce (misc.md): height in pt above rest, a jump every ~0.7 s, until the app
/// opens a window (at most `bounceLimit`).
const List<(double, double)> bounceCurve = [
  (0, 0),
  (27, 3),
  (40, 5),
  (53, 6),
  (80, 8),
  (93, 9),
  (133, 11),
  (160, 13),
  (213, 14),
  (293, 17),
  (440, 14),
  (533, 11),
  (587, 7),
  (613, 6),
  (653, 3),
  (693, 0),
];
const double bouncePeriod = 693;
const Duration bounceLimit = Duration(seconds: 10);

/// Bounce height `ms` after the start (repeated).
double bounceHeight(double ms) {
  if (ms <= 0) return 0;
  final t = ms % bouncePeriod;
  for (var i = 1; i < bounceCurve.length; i++) {
    final (t1, h1) = bounceCurve[i];
    if (t <= t1) {
      final (t0, h0) = bounceCurve[i - 1];
      return h0 + (h1 - h0) * (t - t0) / (t1 - t0);
    }
  }
  return 0;
}

/// The cell under the pointer, if there is an app.
/// The cell under `x`, separator included.
int? cellAt(Layout l, double x) {
  for (var i = 0; i < l.cells.length; i++) {
    final (left, w) = l.cells[i];
    if (x >= left && x < left + w) return i;
  }
  return null;
}

int? itemAt(Layout l, List<Item> items, double x) {
  for (var i = 0; i < l.cells.length; i++) {
    final (left, w) = l.cells[i];
    if (x >= left && x < left + w) {
      return items[i].kind == ItemKind.app ? i : null;
    }
  }
  return null;
}
