import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/hud.dart';
import 'package:silica_shell/system/system_status.dart';
import 'package:silica_ui/silica_ui.dart';

import 'controlcenter_test.dart' show FakeSystem, laptop;

Future<FakeSystem> pumpHud(
  WidgetTester tester, {
  bool suppressed = false,
}) async {
  tester.view.physicalSize = const Size(1280, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final system = FakeSystem(laptop.copyWith());
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: SystemHud(system: system, suppressed: suppressed),
    ),
  );
  return system;
}

void main() {
  test('in with fade.quick and an 8 pt rise, out with fade.quick', () {
    // fade.quick is 150 ms: the opacity reaches 1 long before the rise settles (450 ms).
    expect(hudAppearOpacity(0), 0);
    expect(hudAppearOpacity(150 / 450), closeTo(1, 1e-9));
    expect(hudAppearOpacity(1), 1);
    expect(hudAppearOffset(0), hudRise);
    expect(hudAppearOffset(1), closeTo(0, 0.05));
    expect(hudRise, 8);
    expect(hudVanish, AmalfiFades.quick.duration);
    expect(hudVanishOpacity(0), 1);
    expect(hudVanishOpacity(150), 0);
  });

  testWidgets(
    'a volume change shows the HUD, which stays 1.09 s and then disappears',
    (tester) async {
      final system = await pumpHud(tester);
      expect(find.text('Audio output'), findsNothing);
      system.value = system.value.copyWith(volume: 0.5625);
      await tester.pump();
      await tester.pump(hudAppear);
      expect(find.text('Audio output'), findsOneWidget);
      // At the top right: 10 pt below the menu bar and from the edge.
      final panel = tester.getTopRight(find.text('Audio output')).dy;
      expect(panel, greaterThan(30 + 10));

      // A new change with the panel visible restarts the wait.
      await tester.pump(const Duration(milliseconds: 800));
      system.value = system.value.copyWith(volume: 0.625);
      await tester.pump(const Duration(milliseconds: 800));
      expect(find.text('Audio output'), findsOneWidget);
      await tester.pump(hudHold);
      await tester.pumpAndSettle();
      expect(find.text('Audio output'), findsNothing);
    },
  );

  testWidgets('brightness requested by Silica: display title', (tester) async {
    final system = await pumpHud(tester);
    system.value = system.value.copyWith(
      brightness: 0.25,
      brightnessRequests: system.value.brightnessRequests + 1,
    );
    await tester.pump();
    expect(find.text('Display'), findsOneWidget);
  });

  testWidgets('keyboard backlight set by Silica: its own title (SH-CC-30)', (
    tester,
  ) async {
    final system = await pumpHud(tester);
    system.value = system.value.copyWith(
      keyboardAvailable: true,
      keyboardBrightness: 0.5,
    );
    await tester.pump();
    // Becoming available is not a change by the user.
    expect(find.text('Keyboard Brightness'), findsNothing);
    system.value = system.value.copyWith(
      keyboardBrightness: 0.5625,
      keyboardRequests: system.value.keyboardRequests + 1,
    );
    await tester.pump();
    expect(find.text('Keyboard Brightness'), findsOneWidget);
  });

  testWidgets('adaptive brightness does not show the HUD', (tester) async {
    final system = await pumpHud(tester);
    // The value changes by itself, no request from Silica.
    for (final v in [0.4, 0.42, 0.45]) {
      system.value = system.value.copyWith(brightness: v);
      await tester.pump(const Duration(milliseconds: 200));
    }
    expect(find.text('Display'), findsNothing);
  });

  testWidgets('with the Control Center open no HUD', (tester) async {
    final system = await pumpHud(tester, suppressed: true);
    system.value = system.value.copyWith(volume: 0.1, muted: true);
    await tester.pump(hudAppear);
    expect(find.byType(Text), findsNothing);
  });

  testWidgets('the first read from the daemon does not show the HUD', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    addTearDown(tester.view.reset);
    final system = FakeSystem(const SystemSnapshot());
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: SystemHud(system: system, suppressed: false),
      ),
    );
    system.value = laptop;
    await tester.pump(hudAppear);
    expect(find.byType(Text), findsNothing);
  });
}
