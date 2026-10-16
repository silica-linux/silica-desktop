// The appearance of Control Center, HUD and notifications. Control Center and HUD follow it
// (dark text on the near-white light glass, Amalfi); notifications change
// (notifications.md, dark over grey).

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/controlcenter/controlcenter.dart';
import 'package:silica_shell/controlcenter/hud.dart';
import 'package:silica_shell/notifications/model.dart';
import 'package:silica_shell/notifications/notifications.dart';
import 'package:silica_ui/silica_ui.dart';

import 'controlcenter_test.dart' show FakeSystem, laptop, sendCommand;

Widget themed(Brightness brightness, Widget child) => MediaQuery(
  data: MediaQueryData(platformBrightness: brightness),
  child: Directionality(textDirection: TextDirection.ltr, child: child),
);

Color? colorOf(WidgetTester tester, String text) =>
    tester.widget<Text>(find.text(text).first).style?.color;

/// The text color as drawn: the style's own, else the inherited one.
Color? drawnColorOf(WidgetTester tester, String text) {
  final finder = find.text(text).first;
  return tester.widget<Text>(finder).style?.color ??
      DefaultTextStyle.of(tester.element(finder)).style.color;
}

void main() {
  testWidgets('dark notification: text, time, actions and buttons', (
    tester,
  ) async {
    final n = ShellNotification(
      id: 1,
      appName: 'Tempo',
      summary: 'Resoconto',
      body: 'Corpo',
      actions: const [('default', 'Open'), ('stop', 'Disattiva')],
      timestamp: DateTime(2026, 10, 13, 9, 2),
    );
    await tester.pumpWidget(
      themed(
        Brightness.dark,
        Align(
          alignment: Alignment.topLeft,
          child: SizedBox(
            width: 348,
            child: NotificationCard(
              notification: n,
              panel: true,
              showTime: true,
              showActions: true,
            ),
          ),
        ),
      ),
    );
    // Amalfi dark tokens: Text/Primary, Text/Secondary and the Surface/Raised of a Centre card.
    expect(colorOf(tester, 'Resoconto'), AmalfiColors.dark.textPrimary);
    expect(colorOf(tester, 'Corpo'), AmalfiColors.dark.textSecondary);
    expect(find.text('Disattiva'), findsOneWidget);
    final card = tester
        .widgetList<DecoratedBox>(find.byType(DecoratedBox))
        .map((b) => b.decoration)
        .whereType<BoxDecoration>()
        .where((d) => d.color == AmalfiColors.dark.surfaceRaised);
    expect(card, isNotEmpty);
  });

  testWidgets('Control Center text follows the appearance', (tester) async {
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
      colors.add(drawnColorOf(tester, 'Wi-Fi'));
      await sendCommand(tester, 'controlcenter.toggle');
      await tester.pumpAndSettle();
    }
    // Light: Amalfi Text/Primary on the near-white glass; dark: white.
    expect(colors.first, AmalfiColors.light.textPrimary);
    expect(colors.last, AmalfiColors.dark.textPrimary);
  });

  testWidgets('HUD text follows the appearance', (tester) async {
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
      colors.add(drawnColorOf(tester, 'Audio output'));
      await tester.pump(hudHold);
      await tester.pumpAndSettle();
    }
    // Light: Amalfi Text/Primary on the near-white glass; dark: white.
    expect(colors.first, AmalfiColors.light.textPrimary);
    expect(colors.last, AmalfiColors.dark.textPrimary);
  });
}
