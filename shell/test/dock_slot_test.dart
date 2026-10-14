// The slots of the Dock open and close when an app that is not kept in the Dock launches or
// quits: the neighbours slide, the glass background follows the width every frame.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';

const _a = Item.app('A', command: 'a');
const _b = Item.app('B', command: 'b');
const _c = Item.app('C', command: 'c', running: true);
const _sep = Item.separator();

Widget _dock(List<Item> items, {bool autohide = false, double mag = 1}) =>
    Directionality(
      textDirection: TextDirection.ltr,
      child: Dock(items: items, autohide: autohide, magnification: mag),
    );

void main() {
  const total = 1280.0;
  double rest(List<Item> items) =>
      items.fold<double>(0, (s, i) => s + i.restWidth) + 2 * sidePadding;

  Future<void> setUp(WidgetTester tester) async {
    tester.view.physicalSize = const Size(total, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
  }

  double glassWidth(WidgetTester tester) {
    final dock = tester.state(find.byType(Dock));
    // The background is the first Positioned child of the Dock stack with the glass height.
    final positioned = find
        .descendant(
          of: find.byWidget(dock.widget),
          matching: find.byType(Positioned),
        )
        .evaluate()
        .map((e) => e.widget as Positioned)
        .firstWhere((p) => p.height == backgroundHeight);
    return positioned.width!;
  }

  testWidgets('a launching app opens its slot over the duration, centered', (
    tester,
  ) async {
    await setUp(tester);
    await tester.pumpWidget(_dock([_a, _b]));
    expect(glassWidth(tester), closeTo(rest([_a, _b]), 0.001));

    await tester.pumpWidget(_dock([_a, _b, _c]));
    // First frame: the slot is closed.
    await tester.pump();
    expect(glassWidth(tester), closeTo(rest([_a, _b]), 0.5));
    final half = <double>[];
    for (var i = 0; i < 5; i++) {
      await tester.pump(slotOpenDuration ~/ 5);
      half.add(glassWidth(tester));
    }
    // Monotonic growth, no jump, reaching the full width at the end.
    for (var i = 1; i < half.length; i++) {
      expect(half[i], greaterThanOrEqualTo(half[i - 1]));
    }
    expect(half.first, greaterThan(rest([_a, _b])));
    expect(half.first, lessThan(rest([_a, _b, _c])));
    expect(half.last, closeTo(rest([_a, _b, _c]), 0.01));
    await tester.pumpAndSettle();
    expect(glassWidth(tester), closeTo(rest([_a, _b, _c]), 0.001));
  });

  testWidgets('a quitting app collapses its slot and is gone afterwards', (
    tester,
  ) async {
    await setUp(tester);
    await tester.pumpWidget(_dock([_a, _b, _c]));
    await tester.pumpWidget(_dock([_a, _b]));
    await tester.pump();
    // Still there at the start...
    expect(glassWidth(tester), closeTo(rest([_a, _b, _c]), 0.5));
    await tester.pump(slotCloseDuration ~/ 2);
    final mid = glassWidth(tester);
    expect(mid, lessThan(rest([_a, _b, _c])));
    expect(mid, greaterThan(rest([_a, _b])));
    await tester.pump(slotCloseDuration);
    expect(glassWidth(tester), closeTo(rest([_a, _b]), 0.001));
    // ...and gone: nothing keeps animating.
    expect(tester.binding.transientCallbackCount, 0);
  });

  testWidgets('the separator opens and closes with the running section', (
    tester,
  ) async {
    await setUp(tester);
    await tester.pumpWidget(_dock([_a, _b]));
    await tester.pumpWidget(_dock([_a, _b, _sep, _c]));
    await tester.pump();
    expect(glassWidth(tester), closeTo(rest([_a, _b]), 0.5));
    await tester.pump(slotOpenDuration ~/ 2);
    expect(glassWidth(tester), greaterThan(rest([_a, _b])));
    expect(glassWidth(tester), lessThan(rest([_a, _b, _sep, _c])));
    await tester.pumpAndSettle();
    expect(glassWidth(tester), closeTo(rest([_a, _b, _sep, _c]), 0.001));

    await tester.pumpWidget(_dock([_a, _b]));
    await tester.pump(slotOpenDuration ~/ 2);
    expect(glassWidth(tester), lessThan(rest([_a, _b, _sep, _c])));
    await tester.pumpAndSettle();
    expect(glassWidth(tester), closeTo(rest([_a, _b]), 0.001));
  });

  testWidgets('with magnification the neighbours do not jump', (tester) async {
    await setUp(tester);
    await tester.pumpWidget(_dock([_a, _b], mag: maxMagnification));
    await tester.pumpWidget(_dock([_a, _b, _c], mag: maxMagnification));
    var last = glassWidth(tester);
    for (var i = 0; i < 20; i++) {
      await tester.pump(slotOpenDuration ~/ 20);
      final w = glassWidth(tester);
      // 63 pt over 20 frames: no step larger than a few points.
      expect((w - last).abs(), lessThan(12));
      last = w;
    }
    await tester.pumpAndSettle();
  });

  testWidgets('hidden by auto-hide the Dock updates without animating', (
    tester,
  ) async {
    await setUp(tester);
    await tester.pumpWidget(_dock([_a, _b], autohide: true));
    await tester.pumpWidget(_dock([_a, _b, _c], autohide: true));
    await tester.pump();
    expect(glassWidth(tester), closeTo(rest([_a, _b, _c]), 0.001));
    expect(tester.binding.transientCallbackCount, 0);
  });

  test('presence scales cells, background and icon', () {
    final open = layout(
      [_a, _c],
      width: total,
      height: 300,
      presenceOf: (i) => identical(i, _c) ? 0.5 : 1,
    );
    expect(open.cells[1].$2, cellPitch / 2);
    expect(iconRect(open, 1, 300).$3, iconSize / 2);
    expect(open.background.$3, cellPitch * 1.5 + 2 * sidePadding);
    // Centered while it changes.
    expect(
      open.background.$1 + open.background.$3 / 2,
      closeTo(total / 2, 0.001),
    );
  });
}
