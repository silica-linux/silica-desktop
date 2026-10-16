import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/faro/launcher.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_ui/silica_ui.dart';

// Faro and the Launcher are one surface: one container whose rect and radius animate, and
// one Ossido glass region that follows it every frame.
const entries = {
  'calc': DesktopEntry(id: 'calc', name: 'Calculator', exec: 'calc'),
};

void main() {
  Future<void> open(WidgetTester tester) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: Faro(
          entries: entries,
          startOpen: true,
          folders: FolderIndex.empty,
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 800));
  }

  /// The first frame of a ticker has no time yet: one frame, then the animation to its end.
  Future<void> settle(WidgetTester t) async {
    await t.pump(const Duration(milliseconds: 16));
    await t.pump(const Duration(milliseconds: 1500));
    await t.pump();
  }

  Rect slot(WidgetTester t) =>
      t.getRect(find.byKey(const ValueKey('faro panel slot')));

  Future<void> cmd1(WidgetTester t) async {
    await t.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
    await t.sendKeyEvent(LogicalKeyboardKey.digit1);
    await t.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
  }

  /// The largest glass region (the panel's; menus are smaller).
  Rect glass() {
    Rect? best;
    for (final r in GlassRegistry.instance.regions) {
      final rect = Rect.fromLTWH(
        (r['x'] as num).toDouble(),
        (r['y'] as num).toDouble(),
        (r['width'] as num).toDouble(),
        (r['height'] as num).toDouble(),
      );
      if (best == null || rect.width * rect.height > best.width * best.height) {
        best = rect;
      }
    }
    return best!;
  }

  /// Plays `ms` in 4 ms frames and returns the slot rect of each frame.
  Future<List<Rect>> play(WidgetTester t, int ms) async {
    final rects = [slot(t)];
    for (var i = 0; i < ms ~/ 4; i++) {
      await t.pump(const Duration(milliseconds: 4));
      rects.add(slot(t));
      // The glass region is the container (inflated 2 pt) of this frame or, as it is sent
      // after the frame, of the one before.
      final g = glass();
      final now = rects.last, before = rects[rects.length - 2];
      expect(
        (g.height - now.height).abs() < 1.5 ||
            (g.height - before.height).abs() < 1.5,
        isTrue,
        reason: 'glass $g, panel $now',
      );
      expect(g.center.dx, closeTo(now.center.dx, 1.5));
    }
    return rects;
  }

  void continuous(List<Rect> rects, {double step = 120}) {
    for (var i = 1; i < rects.length; i++) {
      expect(
        (rects[i].width - rects[i - 1].width).abs(),
        lessThan(step),
        reason: "frame $i: ${rects[i - 1]} -> ${rects[i]}",
      );
      expect(
        (rects[i].height - rects[i - 1].height).abs(),
        lessThan(step),
        reason: "frame $i: ${rects[i - 1]} -> ${rects[i]}",
      );
      expect(
        (rects[i].left - rects[i - 1].left).abs(),
        lessThan(step),
        reason: "frame $i: ${rects[i - 1]} -> ${rects[i]}",
      );
    }
  }

  testWidgets('the bar morphs into the Launcher without jump frames', (
    tester,
  ) async {
    await open(tester);
    expect(slot(tester).size, const Size(640, 56));
    await cmd1(tester);
    final rects = await play(tester, 1000);
    continuous(rects);
    expect(rects[1].height, lessThan(120));
    expect(rects.last.size, const Size(844, 580));
    expect(find.byType(Launcher), findsOneWidget);
    expect(
      rects.any((r) => r.height > 100 && r.height < 500 && r.width > 660),
      isTrue,
    );
  });

  testWidgets('Esc goes back along the measured curve, continuous', (
    tester,
  ) async {
    await open(tester);
    await cmd1(tester);
    await settle(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    final rects = await play(tester, 400);
    continuous(rects);
    expect(rects.last.size, const Size(640, 56));
    expect(find.byType(Launcher), findsNothing);
  });

  testWidgets('grid and list morph into each other', (tester) async {
    await open(tester);
    await cmd1(tester);
    await settle(tester);
    await tester.tap(find.byKey(const ValueKey('launcher more')));
    await tester.pump();
    await tester.tap(find.text('as List'));
    final rects = await play(tester, 1000);
    continuous(rects, step: 60);
    expect(rects.last.size, const Size(640, 560));
    expect(rects.any((r) => r.width > 650 && r.width < 840), isTrue);
  });

  testWidgets('the Launcher "…" menu declares a compositor glass region', (
    tester,
  ) async {
    await open(tester);
    await cmd1(tester);
    await settle(tester);
    final before = GlassRegistry.instance.regions.length;
    await tester.tap(find.byKey(const ValueKey('launcher more')));
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byKey(const ValueKey('launcher view menu')), findsOneWidget);
    final scope = tester.widget<AmalfiGlassBackendScope>(
      find
          .ancestor(
            of: find.byKey(const ValueKey('launcher view menu')),
            matching: find.byType(AmalfiGlassBackendScope),
          )
          .first,
    );
    expect(scope.backend, AmalfiGlassBackend.compositor);
    expect(GlassRegistry.instance.regions.length, greaterThan(before));
  });
}
