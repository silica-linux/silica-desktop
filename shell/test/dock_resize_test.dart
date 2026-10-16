// Resizing the Dock by dragging the separator, and the sizes dock.json carries.

import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/dock/apps.dart';
import 'package:silica_shell/dock/dock.dart';
import 'package:silica_shell/dock/model.dart';
import 'package:silica_shell/dock/options.dart';

const _items = [
  Item.app('A', command: 'a'),
  Item.app('B', command: 'b'),
  Item.separator(),
  Item.app('C', command: 'c'),
];

void main() {
  test(
    'dock.json sizes: defaults, clamped to 16-128, magnified from the size up',
    () {
      final dir = Directory.systemTemp.createTempSync('dock-size');
      addTearDown(() => dir.deleteSync(recursive: true));
      final env = {'XDG_CONFIG_HOME': dir.path};
      final defaults = DockOptions.read(env: env);
      expect(defaults.tileSize, 60);
      expect(defaults.magnify, isFalse);
      saveDockOption('tileSize', 200, env: env);
      expect(DockOptions.read(env: env).tileSize, 128);
      saveDockOption('tileSize', 3, env: env);
      expect(DockOptions.read(env: env).tileSize, 16);
      saveDockOption('tileSize', 64, env: env);
      saveDockOption('magnifiedSize', 40, env: env);
      expect(DockOptions.read(env: env).magnifiedSize, 64);
      saveDockOption('magnifiedSize', 500, env: env);
      expect(DockOptions.read(env: env).magnifiedSize, 128);
      saveDockOption('magnify', true, env: env);
      expect(DockOptions.read(env: env).magnify, isTrue);
    },
  );

  group('separator drag', () {
    late List<double> saved;

    Future<(TestGesture, Offset)> setUp(
      WidgetTester tester,
      Map<String, String> env,
    ) async {
      tester.view.physicalSize = const Size(1280, 400);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      saved = [];
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(
            items: _items,
            onResize: (canvas) {
              saved.add(canvas);
              saveDockOption('tileSize', canvas.round(), env: env);
            },
          ),
        ),
      );
      final rest = layout(_items, width: 1280, height: 400);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(0, 0));
      return (mouse, Offset(rest.center(2), 400 - bottomGap - 30));
    }

    double shelfHeight(WidgetTester tester) => tester
        .widgetList<Positioned>(find.byType(Positioned))
        .firstWhere(
          (p) => p.width != null && p.height != null && p.height! >= 36,
        )
        .height!;

    testWidgets('up is bigger, 1 pt per point, whole points, kept on release', (
      tester,
    ) async {
      final dir = Directory.systemTemp.createTempSync('dock-drag');
      addTearDown(() => dir.deleteSync(recursive: true));
      final env = {'XDG_CONFIG_HOME': dir.path};
      final (mouse, start) = await setUp(tester, env);
      expect(shelfHeight(tester), 80);

      // Over the separator: the ns-resize pointer.
      await mouse.moveTo(start);
      await tester.pump();
      expect(
        tester.binding.mouseTracker.debugDeviceActiveCursor(1),
        SystemMouseCursors.resizeUpDown,
      );

      await mouse.down(start);
      await mouse.moveTo(start - const Offset(0, 10));
      await tester.pump();
      // Canvas 70: the shelf is canvas + 20, at once (no animation).
      expect(shelfHeight(tester), 90);
      // Continuous but snapped to whole points.
      await mouse.moveTo(start - const Offset(0, 17.4));
      await tester.pump();
      expect(shelfHeight(tester), 97);
      // Nothing is saved before the release.
      expect(saved, isEmpty);
      expect(DockOptions.read(env: env).tileSize, 60);
      // Down again, below the starting size.
      await mouse.moveTo(start + const Offset(0, 8));
      await tester.pump();
      expect(shelfHeight(tester), 72);
      await mouse.moveTo(start - const Offset(0, 20));
      await tester.pump();
      await mouse.up();
      await tester.pump();
      expect(saved, [80]);
      expect(DockOptions.read(env: env).tileSize, 80);
      expect(shelfHeight(tester), 100);
    });

    testWidgets('clamped to 16 and 128', (tester) async {
      final dir = Directory.systemTemp.createTempSync('dock-clamp');
      addTearDown(() => dir.deleteSync(recursive: true));
      final (mouse, start) = await setUp(tester, {'XDG_CONFIG_HOME': dir.path});
      await mouse.down(start);
      await mouse.moveTo(start - const Offset(0, 300));
      await tester.pump();
      expect(shelfHeight(tester), 148);
      await mouse.moveTo(start + const Offset(0, 300));
      await tester.pump();
      expect(shelfHeight(tester), 36);
      await mouse.up();
      await tester.pump();
      expect(saved, [16]);
    });

    testWidgets('a click without moving changes nothing', (tester) async {
      final dir = Directory.systemTemp.createTempSync('dock-click');
      addTearDown(() => dir.deleteSync(recursive: true));
      final (mouse, start) = await setUp(tester, {'XDG_CONFIG_HOME': dir.path});
      await mouse.down(start);
      await mouse.up();
      await tester.pump();
      expect(saved, isEmpty);
      expect(shelfHeight(tester), 80);
    });
  });

  testWidgets('the Dock follows its canvas', (tester) async {
    tester.view.physicalSize = const Size(1280, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    for (final canvas in [16.0, 48.0, 128.0]) {
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Dock(items: _items, canvas: canvas),
        ),
      );
      final m = DockMetrics(canvas);
      final glass = tester
          .widgetList<Positioned>(find.byType(Positioned))
          .firstWhere((p) => p.height == m.backgroundHeight);
      final expected = 3 * m.pitch + m.separatorCell + 2 * m.sidePadding;
      expect(glass.width, closeTo(expected, 0.01));
    }
  });
}
