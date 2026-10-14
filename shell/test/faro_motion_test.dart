import 'package:flutter/gestures.dart';
import 'package:flutter/physics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/faro/faro.dart';
import 'package:silica_shell/faro/folders.dart';
import 'package:silica_shell/system/desktop_entries.dart';

const entries = {
  'calc': DesktopEntry(id: 'calc', name: 'Calculator', exec: 'calc'),
};

/// Samples `simulation` every 1 ms over `ms` ms.
List<double> samples(Simulation simulation, int ms) => [
  for (var t = 0; t <= ms; t++) simulation.x(t / 1000),
];

void main() {
  group('the springs reproduce the measured motion', () {
    test(
      'width: 661 → 384 pt, ~22 pt undershoot near +193 ms, settled by ~400 ms',
      () {
        final s = samples(SpringSimulation(widthSpring, 661, 384, 0), 700);
        final low = s.reduce((a, b) => a < b ? a : b);
        final at = s.indexOf(low);
        expect(low, closeTo(362, 3));
        expect(at, inInclusiveRange(175, 215));
        for (var t = 430; t <= 700; t++) {
          expect(s[t], closeTo(384, 1), reason: 't = $t ms');
        }
      },
    );

    test(
      'height: 0.966 → 1 with a +2 pt overshoot on 447 near +133 ms, settled by ~190 ms',
      () {
        final s = samples(SpringSimulation(verticalSpring, 432, 447, 0), 400);
        final high = s.reduce((a, b) => a > b ? a : b);
        expect(high, closeTo(449, 0.8));
        expect(s.indexOf(high), inInclusiveRange(110, 155));
        for (var t = 190; t <= 400; t++) {
          expect(s[t], closeTo(447, 1), reason: 't = $t ms');
        }
      },
    );

    test(
      'buttons: 146 pt with ~11 pt overshoot ~300 ms after their start, settled by ~590 ms',
      () {
        final s = samples(SpringSimulation(buttonsSpring, 0, 146, 0), 800);
        final high = s.reduce((a, b) => a > b ? a : b);
        expect(high - 146, closeTo(11, 2));
        expect(s.indexOf(high), inInclusiveRange(260, 340));
        for (var t = 590; t <= 800; t++) {
          expect(s[t], closeTo(146, 1), reason: 't = $t ms');
        }
      },
    );

    test('the tables: fade, Launcher scale, return to Faro', () {
      expect(opacityAt(0), 0.22);
      expect(opacityAt(26.7), closeTo(0.79, 0.001));
      expect(opacityAt(fadeMs), 1);
      expect(launcherScaleAt(0), closeTo(1.08, 1e-9));
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
      'opening: 1.033 wide and 0.966 tall at the first frame, faded at 0.22',
      (tester) async {
        await open(tester);
        expect(panel().width, closeTo(640 * openScaleX, 0.5));
        expect(scaleY(), closeTo(openScaleY, 0.001));
        expect(opacity(), closeTo(0.22, 0.01));
        // Centred on the 640 pt frame while wider than it.
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

    Future<TestGesture> hover(Offset at) async {
      final mouse = await t.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: const Offset(5, 5));
      await mouse.moveTo(at);
      return mouse;
    }

    testWidgets(
      'hover: the panel narrows first, the buttons follow 80 ms later',
      (tester) async {
        await open(tester);
        await tester.pumpAndSettle();
        final frame = panel();
        // The pointer on the right part of the frame, where the buttons will be.
        final mouse = await hover(Offset(frame.right - 40, frame.center.dy));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 60));
        expect(panel().width, lessThan(640));
        // The buttons wait 80 ms: not there yet.
        expect(button(3), findsNothing);
        await tester.pump(const Duration(milliseconds: 40));
        await tester.pump(const Duration(milliseconds: 16));
        // Coming out from behind the panel's right edge.
        final early = tester.getRect(button(3));
        expect(early.left, lessThan(panel().right));
        await tester.pump(const Duration(milliseconds: 80));
        // Undershoot of the width near +180 ms.
        expect(panel().width, lessThan(384));
        await tester.pump(const Duration(milliseconds: 500));
        expect(panel().width, closeTo(384, 1));
        final last = tester.getRect(button(3));
        expect(last.right, closeTo(frame.left + 640, 1.5));
        expect(
          tester.getRect(button(1)).left - tester.getRect(button(0)).left,
          closeTo(64, 0.5),
        );
        // The pointer stays over the buttons: they don't go away.
        await tester.pump(const Duration(milliseconds: 500));
        expect(button(0), findsOneWidget);
        await mouse.removePointer();
      },
    );

    testWidgets('leaving halfway reverses without a jump', (tester) async {
      await open(tester);
      await tester.pumpAndSettle();
      final frame = panel();
      final mouse = await hover(frame.center);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 120));
      var width = panel().width;
      await mouse.moveTo(Offset(frame.center.dx, frame.bottom + 200));
      // Frame by frame the width changes by less than the spring's top speed allows
      // (~5 pt/ms measured: 16 ms frames, under 90 pt) and never teleports to 640.
      for (var i = 0; i < 60; i++) {
        await tester.pump(const Duration(milliseconds: 16));
        final w = panel().width;
        expect((w - width).abs(), lessThan(90), reason: 'frame $i');
        width = w;
      }
      expect(panel().width, closeTo(640, 0.5));
      expect(button(0), findsNothing);
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
