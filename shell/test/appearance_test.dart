// L'aspetto scuro di Centro di controllo, HUD e notifiche. Centro di controllo e HUD sono
// identici nei due aspetti su grigio (controlcenter.md, hud.md); le notifiche cambiano
// (notifications.md, gray-dark).

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/controlcenter.dart';
import 'package:silica_shell/controlcenter/hud.dart';
import 'package:silica_shell/notifications/model.dart';
import 'package:silica_shell/notifications/notifications.dart';

import 'controlcenter_test.dart' show FakeSystem, laptop, sendCommand;

Widget themed(Brightness brightness, Widget child) => MediaQuery(
  data: MediaQueryData(platformBrightness: brightness),
  child: Directionality(textDirection: TextDirection.ltr, child: child),
);

Color? colorOf(WidgetTester tester, String text) =>
    tester.widget<Text>(find.text(text).first).style?.color;

void main() {
  testWidgets('notifica scura: testo, ora, azioni e pulsanti', (tester) async {
    final n = ShellNotification(
      id: 1,
      appName: 'Tempo',
      summary: 'Resoconto',
      body: 'Corpo',
      actions: const [('default', 'Apri'), ('stop', 'Disattiva')],
      timestamp: DateTime(2026, 10, 13, 9, 2),
    );
    await tester.pumpWidget(
      themed(
        Brightness.dark,
        Align(
          alignment: Alignment.topLeft,
          child: SizedBox(
            width: 344,
            child: NotificationCard(
              notification: n,
              showTime: true,
              showActions: true,
            ),
          ),
        ),
      ),
    );
    expect(colorOf(tester, 'Resoconto'), NotificationColors.dark.text);
    expect(colorOf(tester, 'Corpo'), const Color(0xFFEAEAEB));
    expect(colorOf(tester, 'Disattiva'), const Color(0xFFDADBE0));
    final section = tester
        .widgetList<DecoratedBox>(find.byType(DecoratedBox))
        .map((b) => b.decoration)
        .whereType<BoxDecoration>()
        .where((d) => d.color == NotificationColors.dark.actionsFill);
    expect(section, isNotEmpty);
  });

  testWidgets('Centro di controllo uguale in chiaro e in scuro', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final colors = <Color?>[];
    for (final b in [Brightness.light, Brightness.dark]) {
      await tester.pumpWidget(
        themed(b, ControlCenter(key: ValueKey(b), system: FakeSystem(laptop))),
      );
      await sendCommand(tester, 'controlcenter.toggle');
      await tester.pumpAndSettle();
      colors.add(colorOf(tester, 'Wi-Fi'));
      await sendCommand(tester, 'controlcenter.toggle');
      await tester.pumpAndSettle();
    }
    expect(colors.first, isNotNull);
    expect(colors.last, colors.first);
  });

  testWidgets('HUD uguale in chiaro e in scuro', (tester) async {
    tester.view.physicalSize = const Size(1280, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final colors = <Color?>[];
    for (final b in [Brightness.light, Brightness.dark]) {
      final system = FakeSystem(laptop.copyWith());
      await tester.pumpWidget(
        themed(
          b,
          SystemHud(key: ValueKey(b), system: system, suppressed: false),
        ),
      );
      system.value = system.value.copyWith(
        volume: b == Brightness.light ? 0.5 : 0.25,
      );
      await tester.pump();
      await tester.pump(hudAppear);
      colors.add(colorOf(tester, 'Uscita audio'));
      await tester.pump(hudHold);
      await tester.pumpAndSettle();
    }
    expect(colors.first, isNotNull);
    expect(colors.last, colors.first);
  });
}
