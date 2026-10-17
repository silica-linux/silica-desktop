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
    // Cells: 14 x 64 + 28 = 924 pt, centered.
    final cellsWidth = l.cells.last.$1 + l.cells.last.$2 - l.cells.first.$1;
    expect(cellsWidth, closeTo(924, 0.01));
    // Shelf: cells + 2 x 8, 80 tall, 4 pt from the edge.
    final (bx, by, bw, bh) = l.background;
    expect(bw, 924 + 16);
    expect(bx + bw / 2, closeTo(width / 2, 0.01));
    expect(bh, 80);
    expect(height - (by + bh), 4);
    // Icon: 60 pt, 10 pt below the top of the shelf.
    final (_, iy, side) = iconRect(l, 0, height);
    expect(side, 60);
    expect(iy - by, 10);
  });

  test('the default Dock is the kit shelf: 620 x 80, tiles at 8 + 64 n', () {
    // Launchpad, Mail, Reminders, Photos, Notes, Terminal, Settings, separator, stack, trash.
    final kit = [
      for (var i = 0; i < 7; i++) Item.app('App $i'),
      const Item.separator(),
      const Item.app('Downloads'),
      const Item.app('Trash'),
    ];
    final l = layout(kit, width: 620, height: 100);
    final (bx, _, bw, bh) = l.background;
    expect((bx, bw, bh), (0, 620, 80));
    for (var i = 0; i < 7; i++) {
      // A 64 pt tile at x 8 + 64 i: its 60 pt icon is centred in it.
      expect(l.center(i), closeTo(8 + 64 * i + 32, 0.01));
    }
    // Separator: the 28 pt master at x 456, its line at x 470.
    expect(l.center(7), closeTo(470, 0.01));
    expect(l.center(8), closeTo(484 + 32, 0.01));
    expect(l.center(9), closeTo(548 + 32, 0.01));
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
    expect(l.scales[4] * 60, closeTo(96, 0.01));
    expect(l.cells[4].$2, closeTo(64 + 36, 0.01));
    // One pitch away: (1 + cos(pi / 3)) / 2 = 0.75 of the growth.
    expect(profile(64), closeTo(0.75, 1e-9));
    expect(l.scales[5] * 60, closeTo(60 + 36 * 0.75, 0.01));
    // Three tiles away and beyond: nothing.
    expect(profile(3 * 64), 0);
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
    for (final off in [null, 60.0, 30.0]) {
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
    expect(enterDuration(65 / 60).inMilliseconds, 60);
    expect(enterDuration(128 / 60).inMilliseconds, 160);
    expect(enterProgress(0), 0);
    expect(enterProgress(50 / 160), closeTo(0.2065, 0.001));
    expect(enterProgress(1), 1);
  });

  test(
    'resizing keeps the icon and the dot in their place in the tile (c 16-128)',
    () {
      double? previousGap;
      for (var c = 16; c <= 128; c++) {
        final m = DockMetrics(c.toDouble());
        final k = c / 60;
        final reason = 'canvas $c';
        // The icon canvas is centred in the (c + 4k) x (c + 20k) tile (margins 2k and 10k, each
        // rounded once); the dot, drawn at the tile's centre line, is on the same axis.
        expect(m.iconLeft + m.canvas / 2, m.tileWidth / 2, reason: reason);
        expect(m.iconTop + m.canvas / 2, m.tileHeight / 2, reason: reason);
        expect(m.tileWidth, c + 2 * (2 * k).roundToDouble(), reason: reason);
        expect(m.tileHeight, c + 2 * (10 * k).roundToDouble(), reason: reason);
        // The margins scale with the canvas: never off 2k / 10k by more than a point.
        expect(
          (m.tileWidth - c - 4 * k).abs(),
          lessThanOrEqualTo(1),
          reason: reason,
        );
        expect(
          (m.tileHeight - c - 20 * k).abs(),
          lessThanOrEqualTo(1),
          reason: reason,
        );
        // The dot's centre lies inside the margin below the icon, whatever the size.
        expect(
          m.tileHeight - m.iconTop - m.canvas,
          greaterThanOrEqualTo(m.dotAbove),
          reason: reason,
        );
        // One rounding per derived frame: top at 10k, dot centre 6k above the shelf bottom.
        expect(m.iconTop, (10 * k).roundToDouble(), reason: reason);
        expect(m.dotAbove, (6 * k).roundToDouble(), reason: reason);
        // In a laid-out Dock the icon and the dot share the tile's centre.
        final items = [const Item.app('A', running: true), const Item.app('B')];
        final l = layout(items, width: 800, height: 200, metrics: m);
        for (var i = 0; i < items.length; i++) {
          final (x, _, side) = iconRect(l, i, 200);
          expect(x + side / 2, l.center(i), reason: reason);
          expect(
            l.cells[i].$1 + m.iconLeft + m.canvas / 2,
            l.center(i),
            reason: reason,
          );
        }
        // The icon-to-dot distance moves by at most the two roundings (10k and 6k) per step.
        final gap = (m.tileHeight - m.dotAbove) - (m.iconTop + m.canvas);
        if (previousGap != null) {
          expect(
            (gap - previousGap).abs(),
            lessThanOrEqualTo(2),
            reason: reason,
          );
        }
        previousGap = gap;
      }
    },
  );

  test('geometry at 16, 60 and 128 pt', () {
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
      (18, 22, 0, 2, 6, 7, 16, 3, 3),
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
      (64, 80, 0, 64, 8, 24, 28, 28, 60, 4, 12),
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
      (136, 170, 0, 136, 17, 51, 60, 128, 9, 26),
    );
    for (final m in [small, normal, big]) {
      final kit = [
        for (var i = 0; i < 9; i++) const Item.app('A'),
        const Item.separator(),
      ];
      final l = layout(kit, width: 1000, height: 300, metrics: m);
      // Tiles + one separator + the padding on both sides.
      final width = 9 * m.tileWidth + m.separatorWidth + 2 * m.shelfPadding;
      expect(l.background.$3, closeTo(width, 0.01));
      expect(l.background.$4, m.tileHeight);
      // The first tile starts at the padding.
      expect(l.cells.first.$1 - l.background.$1, closeTo(m.shelfPadding, 0.01));
      expect(m.k, m.canvas / 60);
    }
    expect(
      layout([const Item.app('A')], width: 620, height: 100).background.$4,
      80,
    );
  });

  test('the whole canvas of an app is the tile canvas, places have the same', () {
    final l = layout(items, width: width, height: height);
    final (x, y, side) = iconRect(l, 0, height);
    expect(side, 60);
    // Tile 64 x 80: the canvas is at x 2 and y 10 of it.
    expect(x - l.cells[0].$1, closeTo(2, 0.01));
    expect(y - l.background.$2, closeTo(10, 0.001));
    // An app: the 824 body of the 1024 canvas is 48.3 pt.
    final (ax, ay, app) = iconBody(side, place: false);
    expect(app, closeTo(60 * 824 / 1024, 1e-9));
    expect(ax, closeTo((60 - app) / 2, 1e-9));
    // A place: its drawing is the whole canvas, at the same place as the app's.
    final (px, py, place) = iconBody(side, place: true);
    expect((px, py, place), (0, 0, 60));
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
