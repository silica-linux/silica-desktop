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
  test('at rest: cells, background and icons as in the recordings', () {
    final l = layout(items, width: width, height: height);
    // Cells: 14 × 63 + 28.35 = 910.35 pt, centered.
    final cellsWidth = l.cells.last.$1 + l.cells.last.$2 - l.cells.first.$1;
    expect(cellsWidth, closeTo(910.35, 0.01));
    // Background: ~927 pt wide (x 816–1743), 79 tall, 5 pt from the edge.
    final (bx, by, bw, bh) = l.background;
    expect(bw, closeTo(927, 1.5));
    expect(bx + bw / 2, closeTo(width / 2, 0.01));
    expect(bh, 79);
    expect(height - (by + bh), 5);
    // Icon: 48 pt, 16 pt below the top of the background.
    final (_, iy, side) = iconRect(l, 0, height);
    expect(side, 48);
    expect(iy - by, 16);
  });

  test('magnification: 68 pt under the pointer, table profile', () {
    final rest = layout(items, width: width, height: height);
    final mail = rest.center(4);
    final l = layout(
      items,
      width: width,
      height: height,
      pointerX: mail,
      amount: 1,
    );
    expect(l.cells[4].$2, closeTo(68, 0.01));
    // 2 cells (126 pt) from the pointer f ≈ 0.499: 63 · (1 + 0.0794 · 0.499).
    expect(
      l.cells[6].$2,
      closeTo(63 * (1 + (68 / 63 - 1) * profile(126)), 0.01),
    );
    // The point under the pointer stays still.
    expect(l.center(4), closeTo(mail, 0.01));
    // Each side grows by at most ~9.66 pt (list from 910.35 to 929.67).
    final grown = l.cells.last.$1 + l.cells.last.$2 - l.cells.first.$1;
    expect(grown, closeTo(929.67, 0.6));
    // The background does not rise.
    expect(l.background.$2, rest.background.$2);
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

  test('maximum magnification: 132 pt and the icon edge rises by ~7 pt', () {
    final rest = layout(items, width: width, height: height);
    final mail = rest.center(4);
    final l = layout(
      items,
      width: width,
      height: height,
      pointerX: mail,
      amount: 1,
      magnification: maxMagnification,
    );
    expect(l.cells[4].$2, closeTo(132, 0.01));
    final (_, restTop, restSide) = iconRect(rest, 4, height);
    final (_, top, side) = iconRect(l, 4, height);
    expect((restTop + restSide) - (top + side), closeTo(7, 1));
  });

  test('entrance: ~60 ms at 68 pt, ~160 ms at 132 pt, measured curve', () {
    expect(enterDuration(defaultMagnification).inMilliseconds, 60);
    expect(enterDuration(maxMagnification).inMilliseconds, 160);
    expect(enterProgress(0), 0);
    expect(enterProgress(50 / 160), closeTo(0.2065, 0.001));
    expect(enterProgress(1), 1);
  });

  test('active zone: 5 pt above the background, inside the cells', () {
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
