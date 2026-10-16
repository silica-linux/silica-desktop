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

  test('magnification: 68/63 under the pointer, scaled to the 56 pt pitch', () {
    final rest = layout(items, width: width, height: height);
    final mail = rest.center(4);
    final l = layout(
      items,
      width: width,
      height: height,
      pointerX: mail,
      amount: 1,
    );
    expect(l.cells[4].$2, closeTo(56 * 68 / 63, 0.01));
    // 2 cells (112 pt) from the pointer: the measured table at 126 pt of the 63 pt pitch.
    expect(
      l.cells[6].$2,
      closeTo(56 * (1 + (68 / 63 - 1) * profile(112)), 0.01),
    );
    expect(profile(112), closeTo(0.499, 0.01));
    // The point under the pointer stays still.
    expect(l.center(4), closeTo(mail, 0.01));
    // The shelf does not rise.
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

  test(
    'maximum magnification: 132/63 of the pitch and the icon edge rises',
    () {
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
      expect(l.cells[4].$2, closeTo(56 * 132 / 63, 0.01));
      final (_, restTop, restSide) = iconRect(rest, 4, height);
      final (_, top, side) = iconRect(l, 4, height);
      expect((restTop + restSide) - (top + side), closeTo(7 * amalfiScale, 1));
    },
  );

  test('entrance: ~60 ms at 68 pt, ~160 ms at 132 pt, measured curve', () {
    expect(enterDuration(defaultMagnification).inMilliseconds, 60);
    expect(enterDuration(maxMagnification).inMilliseconds, 160);
    expect(enterProgress(0), 0);
    expect(enterProgress(50 / 160), closeTo(0.2065, 0.001));
    expect(enterProgress(1), 1);
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
