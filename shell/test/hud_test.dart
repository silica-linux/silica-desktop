import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/hud.dart';
import 'package:silica_shell/system/system_status.dart';

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
  test('appearance and disappearance follow the measurements', () {
    expect(hudAppearScale(0), hudScaleFrom);
    expect(hudAppearScale(1), 1);
    expect(hudAppearOpacity(0.5), 0.25);
    // Fast drop in the first 80 ms, then a tail up to 550.
    expect(hudVanishOpacity(80), closeTo(0.22, 1e-9));
    expect(hudVanishOpacity(550), 0);
    expect(hudVanishScale(410), hudScaleFrom);
  });

  testWidgets(
    'a volume change shows the HUD, which stays 1.09 s and then disappears',
    (tester) async {
      final system = await pumpHud(tester);
      expect(find.text('Uscita audio'), findsNothing);
      system.value = system.value.copyWith(volume: 0.5625);
      await tester.pump();
      await tester.pump(hudAppear);
      expect(find.text('Uscita audio'), findsOneWidget);
      // At the top right: 10 pt below the menu bar and from the edge.
      final panel = tester.getTopRight(find.text('Uscita audio')).dy;
      expect(panel, greaterThan(30 + 10));

      // A new change with the panel visible restarts the wait.
      await tester.pump(const Duration(milliseconds: 800));
      system.value = system.value.copyWith(volume: 0.625);
      await tester.pump(const Duration(milliseconds: 800));
      expect(find.text('Uscita audio'), findsOneWidget);
      await tester.pump(hudHold);
      await tester.pumpAndSettle();
      expect(find.text('Uscita audio'), findsNothing);
    },
  );

  testWidgets('brightness requested by Silica: display title', (tester) async {
    final system = await pumpHud(tester);
    system.value = system.value.copyWith(
      brightness: 0.25,
      brightnessRequests: system.value.brightnessRequests + 1,
    );
    await tester.pump();
    expect(find.text('Schermo'), findsOneWidget);
  });

  testWidgets('adaptive brightness does not show the HUD', (tester) async {
    final system = await pumpHud(tester);
    // The value changes by itself, no request from Silica.
    for (final v in [0.4, 0.42, 0.45]) {
      system.value = system.value.copyWith(brightness: v);
      await tester.pump(const Duration(milliseconds: 200));
    }
    expect(find.text('Schermo'), findsNothing);
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
