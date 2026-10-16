import 'package:flutter/gestures.dart';
import 'package:flutter/physics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/system/desktop_entries.dart';
import 'package:silica_ui/silica_ui.dart';

const entries = {
  'calc': DesktopEntry(id: 'calc', name: 'Calculator', exec: 'calc'),
};

/// Samples `simulation` every 1 ms over `ms` ms.
List<double> samples(Simulation simulation, int ms) => [
  for (var t = 0; t <= ms; t++) simulation.x(t / 1000),
];

void main() {
  group('the springs are the Amalfi motion tokens', () {
    test('the shapes use the tokens of motion.md', () {
      expect(verticalSpring, AmalfiSprings.panel);
      expect(scopeSpring, AmalfiSprings.panel);
      expect(widthSpring, AmalfiSprings.morph);
      expect(engageWidthSpring, AmalfiSprings.morph);
      expect(buttonsSpring, AmalfiSprings.morph);
      expect(engageButtonsSpring, AmalfiSprings.morph);
      expect(engageDelay, AmalfiDelays.hoverIntent);
      expect(closeDuration, AmalfiFades.instant.duration);
    });

    test(
      'spring.panel: ~+2 pt on a 270 pt growth, settled in well under 600 ms',
      () {
        final s = samples(SpringSimulation(verticalSpring, 56, 326, 0), 800);
        final high = s.reduce((a, b) => a > b ? a : b);
        expect(high - 326, inInclusiveRange(1, 6));
        for (var t = 600; t <= 800; t++) {
          expect(s[t], closeTo(326, 1), reason: 't = $t ms');
        }
      },
    );

    test(
      'spring.morph: the bar narrows with a few percent of undershoot and settles',
      () {
        final s = samples(SpringSimulation(widthSpring, 640, 384, 0), 1000);
        final low = s.reduce((a, b) => a < b ? a : b);
        // 4-5 % of the 256 pt travel.
        expect(384 - low, inInclusiveRange(6, 16));
        for (var t = 800; t <= 1000; t++) {
          expect(s[t], closeTo(384, 1), reason: 't = $t ms');
        }
      },
    );

    test('the tables: fade, Launcher scale, return to Faro', () {
      expect(opacityAt(0), 0.22);
      expect(opacityAt(26.7), closeTo(0.79, 0.001));
      expect(opacityAt(fadeMs), 1);
      expect(launcherScaleAt(0), closeTo(1.06, 1e-9));
      expect(launcherScaleAt(launcherOpenMs), 1);
      expect(backProgress(0), 0);
      expect(backProgress(backDuration), 1);
    });
  });

  group('Faro moves as one panel', () {
    late WidgetTester t;

    Future<void> open(WidgetTester tester) async {
      t = tester;
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
    }

    Rect panel() => t.getRect(find.byKey(const ValueKey('faro panel')));
    double scaleY() => t
        .widget<Transform>(find.byKey(const ValueKey('faro panel')))
        .transform
        .storage[5];
    double opacity() => t
        .widget<Opacity>(
          find
              .descendant(
                of: find.byKey(const ValueKey('faro panel')),
                matching: find.byType(Opacity),
              )
              .first,
        )
        .opacity;
    Finder button(int i) => find.byKey(ValueKey('faro button $i'));

    testWidgets(
      'opening: 0.97 tall at the first frame, never wider than 640, faded at 0.22',
      (tester) async {
        await open(tester);
        expect(panel().width, closeTo(640 * openScaleX, 0.5));
        expect(scaleY(), closeTo(openScaleY, 0.001));
        expect(opacity(), closeTo(0.22, 0.01));
        // The envelope: never wider than 640 pt.
        expect(panel().width, 640);
        expect(panel().center.dx, closeTo(720, 0.5));
        await tester.pump(const Duration(milliseconds: 70));
        expect(opacity(), 1);
        await tester.pump(const Duration(milliseconds: 130));
        expect(scaleY(), closeTo(1, 0.003));
        await tester.pump(const Duration(milliseconds: 250));
        expect(panel().width, closeTo(640, 0.5));
        expect(button(0), findsNothing);
      },
    );

    /// Frames of 16 ms for `ms` (a timer fires at its time, an animation then runs).
    Future<void> run(WidgetTester tester, int ms) async {
      for (var t = 0; t < ms; t += 16) {
        await tester.pump(const Duration(milliseconds: 16));
      }
    }

    Future<TestGesture> hover(Offset at) async {
      final mouse = await t.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(5, 5));
      await mouse.moveTo(at);
      return mouse;
    }

    testWidgets(
      'hover: after 60 ms the capsule becomes a 384 pt field (settled ~690 ms) and '
      'four discs (~800 ms)',
      (tester) async {
        await open(tester);
        await tester.pumpAndSettle();
        final frame = panel();
        final mouse = await hover(Offset(frame.right - 40, frame.center.dy));
        await tester.pump();
        // The 60 ms wait: nothing moves yet.
        await tester.pump(const Duration(milliseconds: 50));
        expect(panel().width, 640);
        expect(button(3), findsNothing);
        await run(tester, 40);
        await run(tester, 16);
        expect(panel().width, lessThan(640));
        // The discs wait 80 ms more.
        expect(button(3), findsNothing);
        await run(tester, 100);
        expect(button(3), findsOneWidget);
        // Settled: the field ~690 ms after the pointer entered, the discs ~800 ms.
        await run(tester, 450);
        expect(panel().width, closeTo(384, 1.5));
        await run(tester, 120);
        final last = tester.getRect(button(3));
        expect(last.right, closeTo(frame.left + 640, 1.5));
        expect(
          tester.getRect(button(1)).left - tester.getRect(button(0)).left,
          closeTo(64, 0.5),
        );
        // The pointer going away does not take them back: they stay while Faro is open.
        await mouse.moveTo(Offset(frame.center.dx, frame.bottom + 300));
        await run(tester, 1500);
        expect(panel().width, closeTo(384, 0.5));
        expect(button(0), findsOneWidget);
        await mouse.removePointer();
      },
    );

    testWidgets('leaving within 60 ms changes nothing', (tester) async {
      await open(tester);
      await tester.pumpAndSettle();
      final frame = panel();
      final mouse = await hover(frame.center);
      await tester.pump(const Duration(milliseconds: 30));
      await mouse.moveTo(Offset(frame.center.dx, frame.bottom + 200));
      await run(tester, 1000);
      expect(panel().width, 640);
      expect(button(0), findsNothing);
      await mouse.removePointer();
    });

    testWidgets('the discs have no tooltip, hover or pressed state', (
      tester,
    ) async {
      await open(tester);
      await tester.pumpAndSettle();
      final mouse = await hover(panel().center);
      await run(tester, 1500);
      expect(find.byType(SilicaTooltip), findsNothing);
      // Over a disc and pressed: still no tooltip, and the glass is not interactive.
      await mouse.moveTo(tester.getCenter(button(2)));
      await run(tester, 2000);
      await mouse.down(tester.getCenter(button(2)));
      await run(tester, 300);
      expect(find.byType(SilicaTooltip), findsNothing);
      final glass = tester.widget<LiquidGlass>(
        find.descendant(of: button(2), matching: find.byType(LiquidGlass)),
      );
      expect(glass.interactive, isFalse);
      await mouse.up();
      await mouse.removePointer();
    });

    testWidgets(
      'typing on the narrow panel widens it again and grows it down',
      (tester) async {
        await open(tester);
        await tester.pumpAndSettle();
        final frame = panel();
        final mouse = await hover(frame.center);
        await tester.pumpAndSettle();
        expect(panel().width, closeTo(384, 1));
        await tester.enterText(find.byType(EditableText), 'calc');
        await tester.pump();
        final top = panel().top;
        final start = panel().height;
        await tester.pump(const Duration(milliseconds: 40));
        // The height grows with the vertical spring, the top edge stays.
        expect(panel().height, greaterThan(start));
        expect(panel().top, closeTo(top, 0.01));
        await tester.pumpAndSettle();
        expect(panel().width, closeTo(640, 0.5));
        expect(panel().height, greaterThan(56 + 57));
        expect(button(0), findsNothing);
        await mouse.removePointer();
      },
    );

    testWidgets(
      'closing plays the fade backwards, reopening midway continues from there',
      (tester) async {
        await open(tester);
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 30));
        final midway = opacity();
        expect(midway, inExclusiveRange(0.22, 1));
        // Reopen: no restart from 0.22.
        final state = tester.state(find.byType(Faro));
        (state as dynamic).reopenForTest();
        await tester.pump();
        expect(opacity(), greaterThanOrEqualTo(midway - 0.01));
        await tester.pumpAndSettle();
        expect(opacity(), 1);
        await tester.sendKeyEvent(LogicalKeyboardKey.escape);
        await tester.pumpAndSettle();
        expect(find.byKey(const ValueKey('faro panel')), findsNothing);
      },
    );

    testWidgets('the buttons coming out keep the field focused', (
      tester,
    ) async {
      await open(tester);
      await tester.pumpAndSettle();
      final frame = panel();
      final mouse = await hover(frame.center);
      await tester.pumpAndSettle();
      expect(button(0), findsOneWidget);
      await mouse.moveTo(Offset(frame.center.dx, frame.bottom + 300));
      await tester.pumpAndSettle();
      // Typed, not tapped: the focus must still be in the field.
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
      tester.testTextInput.enterText('c');
      await tester.pumpAndSettle();
      final field = tester.widget<EditableText>(find.byType(EditableText));
      expect(field.focusNode.hasFocus, isTrue);
      expect(field.controller.text, 'c');
      await mouse.removePointer();
    });
  });
}
