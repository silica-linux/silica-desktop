// The Dock numbers against plan/reference/dock.md.

import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/model.dart';

/// The Dock of the recordings: 12 apps, separator, folder and trash, over 2560 pt.
final items = [
  for (var i = 0; i < 12; i++) Item.app('App $i'),
  const Item.separator(),
  const Item.app('Download'),
  const Item.app('Trash'),
];
const width = 2560.0;
const height = 200.0;

void main() {
  test('at rest: cells, shelf and icons as in the kit', () {
    final l = layout(items, width: width, height: height);
    // Cells: 14 x 56 + 12 = 796 pt, centered.
    final cellsWidth = l.cells.last.$1 + l.cells.last.$2 - l.cells.first.$1;
    expect(cellsWidth, closeTo(796, 0.01));
    // Shelf: cells + 2 x 6, 60 tall, 5 pt from the edge.
    final (bx, by, bw, bh) = l.background;
    expect(bw, 796 + 12);
    expect(bx + bw / 2, closeTo(width / 2, 0.01));
    expect(bh, 60);
    expect(height - (by + bh), 5);
    // Icon: 48 pt, 4 pt below the top of the shelf.
    final (_, iy, side) = iconRect(l, 0, height);
    expect(side, 48);
    expect(iy - by, 4);
  });

  test('the default Dock is the kit shelf: 528 x 60, tiles at 8 + 56 n', () {
    // Launchpad, Mail, Reminders, Photos, Notes, Terminal, Settings, separator, stack, trash.
    final kit = [
      for (var i = 0; i < 7; i++) Item.app('App $i'),
      const Item.separator(),
      const Item.app('Downloads'),
      const Item.app('Trash'),
    ];
    final l = layout(kit, width: 528, height: 100);
    final (bx, _, bw, bh) = l.background;
    expect((bx, bw, bh), (0, 528, 60));
    for (var i = 0; i < 7; i++) {
      // A 52 pt tile at x 8 + 56 i: its 48 pt icon is centred in it.
      expect(l.center(i), closeTo(8 + 56 * i + 26, 0.01));
    }
    // Separator: the 8 pt master at x 400, its line at x 404.
    expect(l.center(7), closeTo(404, 0.01));
    expect(l.center(8), closeTo(412 + 26, 0.01));
    expect(l.center(9), closeTo(468 + 26, 0.01));
  });

  test('magnification: cosine fall-off over 3 tiles, only the icons grow', () {
    final rest = layout(items, width: width, height: height);
    final mail = rest.center(4);
    final l = layout(
      items,
      width: width,
      height: height,
      pointerX: mail,
      amount: 1,
      magnified: 96,
    );
    // Under the pointer: the canvas reaches the magnified size, the tile grows with it.
    expect(l.scales[4] * 48, closeTo(96, 0.01));
    expect(l.cells[4].$2, closeTo(56 + 48, 0.01));
    // One pitch away: (1 + cos(pi / 3)) / 2 = 0.75 of the growth.
    expect(profile(56), closeTo(0.75, 1e-9));
    expect(l.scales[5] * 48, closeTo(48 + 48 * 0.75, 0.01));
    // Three tiles away and beyond: nothing.
    expect(profile(3 * 56), 0);
    expect(l.scales[7], 1);
    // The point under the pointer stays still.
    expect(l.center(4), closeTo(mail, 0.01));
    // The shelf keeps its height and its top; its width follows the grown tiles.
    expect(l.background.$2, rest.background.$2);
    expect(l.background.$4, rest.background.$4);
    var grown = 0.0;
    for (var i = 0; i < items.length; i++) {
      grown += l.cells[i].$2 - rest.cells[i].$2;
    }
    expect(l.background.$3, closeTo(rest.background.$3 + grown, 0.01));
    // The icons overflow upward: the bottom of the canvas stays on the tile's.
    final (_, restTop, restSide) = iconRect(rest, 4, height);
    final (_, top, side) = iconRect(l, 4, height);
    expect(top + side, closeTo(restTop + restSide, 0.001));
    expect(side, closeTo(96, 0.01));
    // Off (null) or not above the resting size: nothing moves.
    for (final off in [null, 48.0, 30.0]) {
      final same = layout(
        items,
        width: width,
        height: height,
        pointerX: mail,
        amount: 1,
        magnified: off,
      );
      expect(same.cells, rest.cells);
    }
  });

  test('near one end only the other side grows', () {
    // Like x 820 in the recordings: in the background margin, before the first cell.
    final rest = layout(items, width: width, height: height);
    final edge = rest.background.$1 + 3.5;
    final l = layout(
      items,
      width: width,
      height: height,
      pointerX: edge,
      amount: 1,
    );
    expect(l.cells.first.$1, closeTo(rest.cells.first.$1, 0.3));
  });

  test('entrance: ~60 ms for a small growth, ~160 ms for a large one', () {
    expect(enterDuration(52 / 48).inMilliseconds, 60);
    expect(enterDuration(128 / 48).inMilliseconds, 160);
    expect(enterProgress(0), 0);
    expect(enterProgress(50 / 160), closeTo(0.2065, 0.001));
    expect(enterProgress(1), 1);
  });

  test('geometry at 16, 48 and 128 pt', () {
    const small = DockMetrics(16);
    expect(
      (
        small.tileWidth,
        small.tileHeight,
        small.gap,
        small.shelfPadding,
        small.radius,
        small.separatorWidth,
        small.separatorHeight,
        small.dot,
        small.labelOffset,
      ),
      (20, 28, 1, 3, 7, 3, 12, 3, 4),
    );
    const normal = DockMetrics();
    expect(
      (
        normal.tileWidth,
        normal.tileHeight,
        normal.gap,
        normal.pitch,
        normal.shelfPadding,
        normal.radius,
        normal.separatorWidth,
        normal.separatorCell,
        normal.separatorHeight,
        normal.dot,
        normal.labelOffset,
      ),
      (52, 60, 4, 56, 8, 20, 8, 12, 44, 4, 12),
    );
    const big = DockMetrics(128);
    expect(
      (
        big.tileWidth,
        big.tileHeight,
        big.gap,
        big.pitch,
        big.shelfPadding,
        big.radius,
        big.separatorWidth,
        big.separatorHeight,
        big.dot,
        big.labelOffset,
      ),
      (132, 140, 11, 143, 21, 53, 21, 124, 11, 32),
    );
    // The shelf of the kit: 9 tiles and a separator at 48, 528 wide, 60 tall.
    for (final m in [small, normal, big]) {
      final kit = [
        for (var i = 0; i < 9; i++) const Item.app('A'),
        const Item.separator(),
      ];
      final l = layout(kit, width: 1000, height: 300, metrics: m);
      final k = m.k;
      // Tiles + one separator + the gaps between them + the padding on both sides.
      final width =
          9 * m.tileWidth + m.separatorWidth + 9 * m.gap + 2 * m.shelfPadding;
      expect(l.background.$3, closeTo(width, 0.01));
      expect(l.background.$4, m.canvas + 12);
      // The first tile starts at the padding; its canvas is centred in it.
      expect(
        l.cells.first.$1 + m.gap / 2 - l.background.$1,
        closeTo(m.shelfPadding, 0.01),
      );
      expect(k, m.canvas / 48);
    }
    expect(
      layout([const Item.app('A')], width: 528, height: 100).background.$4,
      60,
    );
  });

  test('the whole canvas of an app is the tile canvas, places spill over', () {
    final l = layout(items, width: width, height: height);
    final (x, y, side) = iconRect(l, 0, height);
    expect(side, 48);
    // Tile 52 x 60: the canvas is at x 2 and y 4 of it.
    expect(x - (l.cells[0].$1 + 4), closeTo(0, 0.01));
    expect(y - l.background.$2, closeTo(4, 0.001));
    // An app: the 824 body of the 1024 canvas is 38.6 pt.
    final (ax, ay, app) = iconBody(side, place: false);
    expect(app, closeTo(48 * 824 / 1024, 1e-9));
    expect(ax, closeTo((48 - app) / 2, 1e-9));
    // A place: its 824 body is as large as the app canvas (canvas 59.6) and its bottom is
    // on the bottom of the app bodies.
    final (px, py, place) = iconBody(side, place: true);
    expect(place, 48);
    expect(px, 0);
    expect(py + place, closeTo(ay + app, 1e-9));
    expect(place * 1024 / 824, closeTo(59.6, 0.1));
    // It overflows ~6 pt per side.
    expect((place * 1024 / 824 - 48) / 2, closeTo(5.8, 0.1));
    expect(const Item.app('Trash', trash: true).isPlace, isTrue);
    expect(const Item.app('Folder', folder: '/tmp').isPlace, isTrue);
    expect(const Item.app('App', command: 'a').isPlace, isFalse);
  });

  test('active zone: 5 pt above the shelf, inside the cells', () {
    final rest = layout(items, width: width, height: height);
    final top = rest.background.$2;
    final x = rest.center(3);
    expect(inActiveZone(rest, x, top - 5, height), isTrue);
    expect(inActiveZone(rest, x, top - 6, height), isFalse);
    expect(
      inActiveZone(rest, rest.background.$1 - 1, top + 10, height),
      isFalse,
    );
    expect(
      inActiveZone(rest, rest.background.$1 + 3.5, top + 10, height),
      isTrue,
    );
  });
}
