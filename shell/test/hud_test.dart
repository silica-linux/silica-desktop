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
  test('comparsa e scomparsa seguono le misure', () {
    expect(hudAppearScale(0), hudScaleFrom);
    expect(hudAppearScale(1), 1);
    expect(hudAppearOpacity(0.5), 0.25);
    // Calo rapido nei primi 80 ms, poi una coda fino a 550.
    expect(hudVanishOpacity(80), closeTo(0.22, 1e-9));
    expect(hudVanishOpacity(550), 0);
    expect(hudVanishScale(410), hudScaleFrom);
  });

  testWidgets(
    'un cambio di volume mostra l\'HUD, che resta 1,09 s e poi sparisce',
    (tester) async {
      final system = await pumpHud(tester);
      expect(find.text('Uscita audio'), findsNothing);
      system.value = system.value.copyWith(volume: 0.5625);
      await tester.pump();
      await tester.pump(hudAppear);
      expect(find.text('Uscita audio'), findsOneWidget);
      // In alto a destra: 10 pt sotto la barra dei menu e dal bordo.
      final panel = tester.getTopRight(find.text('Uscita audio')).dy;
      expect(panel, greaterThan(30 + 10));

      // Un cambio nuovo con il pannello visibile fa ripartire l'attesa.
      await tester.pump(const Duration(milliseconds: 800));
      system.value = system.value.copyWith(volume: 0.625);
      await tester.pump(const Duration(milliseconds: 800));
      expect(find.text('Uscita audio'), findsOneWidget);
      await tester.pump(hudHold);
      await tester.pumpAndSettle();
      expect(find.text('Uscita audio'), findsNothing);
    },
  );

  testWidgets('luminosità: titolo dello schermo', (tester) async {
    final system = await pumpHud(tester);
    system.value = system.value.copyWith(brightness: 0.25);
    await tester.pump();
    expect(find.text('Schermo'), findsOneWidget);
  });

  testWidgets('con il Centro di controllo aperto niente HUD', (tester) async {
    final system = await pumpHud(tester, suppressed: true);
    system.value = system.value.copyWith(volume: 0.1, muted: true);
    await tester.pump(hudAppear);
    expect(find.byType(Text), findsNothing);
  });

  testWidgets('la prima lettura dal daemon non mostra l\'HUD', (tester) async {
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
