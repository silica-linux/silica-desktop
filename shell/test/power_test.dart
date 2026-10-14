import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/menubar/logo_menu.dart';
import 'package:silica_shell/menubar/power.dart';
import 'package:silica_shell/l10n/l10n.dart';
import 'package:silica_shell/system/dbus.dart';
import 'package:silica_ui/silica_ui.dart' show MenuAction;

Widget _card(
  PowerAction action, {
  VoidCallback? onConfirm,
  VoidCallback? onCancel,
}) => Directionality(
  textDirection: TextDirection.ltr,
  child: Center(
    child: PowerConfirmation(
      action: action,
      user: 'Alex Rivera',
      onConfirm: onConfirm ?? () {},
      onCancel: onCancel ?? () {},
    ),
  ),
);

void main() {
  test('the card zooms past 100 % and settles (SH-MB-05)', () {
    expect(powerCardScale(0), closeTo(0.9, 1e-9));
    expect(powerCardScale(90), closeTo(1.06, 1e-9));
    expect(powerCardScale(210), 1);
    expect(powerCardScale(400), 1);
    expect(powerCardOpacity(0), 0);
    expect(powerCardOpacity(150), 1);
  });

  test('power actions only in a real Silica session', () {
    expect(powerAvailable({'XDG_SESSION_DESKTOP': 'silica'}), isTrue);
    expect(powerAvailable({'XDG_SESSION_DESKTOP': 'ubuntu'}), isFalse);
    expect(powerAvailable(const {}), isFalse);
  });

  testWidgets('measured size; the countdown ticks every second from 60', (
    tester,
  ) async {
    await tester.pumpWidget(_card(PowerAction.shutDown));
    await tester.pump(const Duration(milliseconds: 300));
    final card = find.byKey(const ValueKey('power confirmation'));
    expect(tester.getSize(card), const Size(292, 266));
    expect(
      find.text(englishTexts.powerShutDownBody(60)),
      findsOneWidget,
    );
    await tester.pump(const Duration(seconds: 1));
    expect(
      find.text(englishTexts.powerShutDownBody(59)),
      findsOneWidget,
    );
    expect(
      tester.getSize(find.byKey(const ValueKey('power confirm'))),
      const Size(128, 30),
    );
  });

  testWidgets('log out has the taller card for its two-line title', (
    tester,
  ) async {
    await tester.pumpWidget(_card(PowerAction.logOut));
    await tester.pump(const Duration(milliseconds: 300));
    expect(
      tester.getSize(find.byKey(const ValueKey('power confirmation'))),
      const Size(292, 282),
    );
  });

  testWidgets('cancel, confirm, and the countdown reaching zero', (
    tester,
  ) async {
    var cancelled = 0;
    var confirmed = 0;
    await tester.pumpWidget(
      _card(
        PowerAction.restart,
        onCancel: () => cancelled++,
        onConfirm: () => confirmed++,
      ),
    );
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(find.byKey(const ValueKey('power cancel')));
    expect(cancelled, 1);
    await tester.tap(find.byKey(const ValueKey('power confirm')));
    expect(confirmed, 1);
    await tester.pump(const Duration(seconds: 60));
    expect(confirmed, 2);
  });

  testWidgets('logind gets the interactive call on the system bus', (
    tester,
  ) async {
    final calls = <Map<Object?, Object?>>[];
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      DBus.channel,
      (call) async {
        calls.add(call.arguments as Map<Object?, Object?>);
        return <Object?>[];
      },
    );
    // Outside a Silica session nothing is called.
    await runPower(PowerAction.shutDown, logOut: () {});
    expect(calls, isEmpty);
    var loggedOut = 0;
    await runPower(PowerAction.logOut, logOut: () => loggedOut++);
    expect(loggedOut, 1);
  });

  test('the power items: enabled only with power, ⌥ acts at once', () {
    final requests = <(PowerAction, bool)>[];
    List<MenuAction> items({required bool option, required bool enabled}) =>
        logoMenu(
          l: englishTexts,
          run: (_) {},
          launch: (_) {},
          open: (_) {},
          option: option,
          user: 'Alex',
          power: (action, {required now}) => requests.add((action, now)),
          powerEnabled: enabled,
        ).whereType<MenuAction>().toList();
    MenuAction named(List<MenuAction> list, String label) =>
        list.firstWhere((a) => a.label == label);

    final off = items(option: false, enabled: false);
    expect(named(off, englishTexts.shutDown).enabled, isFalse);
    expect(named(off, englishTexts.sleep).enabled, isFalse);

    final on = items(option: false, enabled: true);
    named(on, englishTexts.shutDown).onSelected!();
    named(on, englishTexts.restart).onSelected!();
    named(on, englishTexts.logOutUser('Alex')).onSelected!();
    final now = items(option: true, enabled: true);
    named(now, englishTexts.shutDownNow).onSelected!();
    expect(requests, [
      (PowerAction.shutDown, false),
      (PowerAction.restart, false),
      (PowerAction.logOut, false),
      (PowerAction.shutDown, true),
    ]);
  });
}
