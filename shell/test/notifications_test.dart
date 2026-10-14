import 'dart:async';

import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/notifications/model.dart';
import 'package:silica_shell/notifications/notifications.dart';
import 'package:silica_shell/notifications/service.dart';
import 'package:silica_shell/notifications/settings.dart';
import 'package:silica_shell/system/system_status.dart';

class FakeSource extends ChangeNotifier implements NotificationsSource {
  final Map<int, ShellNotification> items = {};
  final StreamController<ShellNotification> _posted =
      StreamController.broadcast(sync: true);
  final List<String> calls = [];

  void post(ShellNotification n) {
    items[n.id] = n;
    _posted.add(n);
    notifyListeners();
  }

  @override
  Map<int, ShellNotification> get notifications => items;

  @override
  Stream<ShellNotification> get posted => _posted.stream;

  @override
  Future<void> dismiss(int id) async {
    calls.add('dismiss $id');
    items.remove(id);
    notifyListeners();
  }

  @override
  Future<void> invoke(int id, String action) async =>
      calls.add('invoke $id $action');

  @override
  Future<void> reply(int id, String text) async => calls.add('reply $id $text');

  @override
  Future<void> clear() async {
    calls.add('clear');
    items.clear();
    notifyListeners();
  }
}

ShellNotification note(
  int id, {
  String app = 'Mail',
  int minute = 0,
  List<(String, String)> actions = const [],
  int urgency = 1,
  bool transient = false,
  String entry = '',
}) => ShellNotification(
  id: id,
  appName: app,
  desktopEntry: entry,
  summary: 'Titolo $id',
  body: 'Corpo della notifica $id',
  actions: actions,
  urgency: urgency,
  transient: transient,
  timestamp: DateTime(2026, 10, 13, 9, minute),
);

Future<void> pumpLayer(
  WidgetTester tester,
  FakeSource source, {
  NotificationSettings settings = const NotificationSettings(),
}) async {
  tester.view.physicalSize = const Size(1280, 686);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: NotificationsLayer(
        source: source,
        settings: () => settings,
        openApp: (n) => source.calls.add('open ${n.appName}'),
      ),
    ),
  );
}

Future<void> sendCommand(WidgetTester tester, String id) =>
    tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      'silica/shell',
      const JSONMethodCodec().encodeMethodCall(MethodCall('command', id)),
      (_) {},
    );

void main() {
  test('the D-Bus structure becomes a notification', () {
    final n = ShellNotification.fromDBus([
      7, 'Mail', 'mail', 'Titolo', 'Corpo', //
      ['default', 'Open', 'reply', 'Rispondi'],
      2, '', 'org.example.Mail', '', true, false, 1791878005135, -1,
    ])!;
    expect(n.id, 7);
    expect(n.critical, isTrue);
    expect(n.transient, isTrue);
    expect(n.hasDefault, isTrue);
    expect(n.buttons, [('reply', 'Rispondi')]);
    expect(n.group, 'org.example.Mail');
    expect(ShellNotification.fromDBus([1, 2]), isNull);
    expect(ShellNotification.fromDBus('x'), isNull);
  });

  test(
    'the Center groups by app, from the most recent, without the transient ones',
    () {
      final groups = groupNotifications([
        note(1, minute: 1),
        note(2, app: 'Messages', minute: 2),
        note(3, minute: 3),
        note(4, minute: 4, transient: true),
      ]);
      expect([for (final g in groups) g.key], ['Mail', 'Messages']);
      expect([for (final n in groups.first.items) n.id], [3, 1]);
    },
  );

  test('the banner entrance follows the measured deceleration', () {
    expect(bannerEnterOffset(0), bannerEnterDistance);
    // A frame at 75 Hz reduces the remaining distance to ~0.82.
    final ratio = bannerEnterOffset(13.33 * 5) / bannerEnterOffset(13.33 * 4);
    expect(ratio, closeTo(0.82, 0.01));
    expect(bannerEnterOffset(bannerEnterDuration.inMilliseconds.toDouble()), 0);
  });

  testWidgets('a banner enters at the top right and leaves by itself', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(note(1));
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    final card = find.byType(NotificationCard);
    expect(card, findsOneWidget);
    expect(tester.getTopLeft(card), const Offset(1280 - 16 - 344, 30 + 16));
    expect(tester.getSize(card).width, 344);
    expect(find.text('Titolo 1'), findsOneWidget);

    await tester.pump(bannerDuration);
    await tester.pumpAndSettle();
    expect(find.byType(NotificationCard), findsNothing);
    // It stays in the Center.
    expect(source.items, contains(1));
  });

  testWidgets('critical notifications stay, a click activates the default', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(note(2, urgency: 2, actions: [('default', 'Open')]));
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    await tester.pump(bannerDuration * 3);
    expect(find.text('Titolo 2'), findsOneWidget);
    await tester.tap(find.text('Titolo 2'));
    await tester.pumpAndSettle();
    expect(source.calls, ['invoke 2 default']);
    expect(find.text('Titolo 2'), findsNothing);
  });

  testWidgets('the Center shows stacked groups, actions and clear all', (
    tester,
  ) async {
    final source = FakeSource()
      ..items[1] = note(1, minute: 1)
      ..items[2] = note(2, minute: 2)
      ..items[3] = note(
        3,
        app: 'Calendar',
        minute: 3,
        actions: [('default', 'Open'), ('snooze', 'Posticipa')],
      );
    await pumpLayer(tester, source);
    await sendCommand(tester, 'notifications.toggle');
    await tester.pumpAndSettle();

    expect(find.text('Notifications'), findsOneWidget);
    // Calendario is the most recent; Posta is collapsed with a card below.
    expect(find.text('Titolo 3'), findsOneWidget);
    expect(find.text('Titolo 2'), findsOneWidget);
    expect(find.text('Titolo 1'), findsNothing);
    expect(find.text('09:03'), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('Titolo 3')).dy,
      lessThan(tester.getTopLeft(find.text('Titolo 2')).dy),
    );
    // The most recent group sits above the header, the others below it (SH-NC, #11).
    final header = tester.getTopLeft(find.text('Notifications')).dy;
    expect(tester.getTopLeft(find.text('Titolo 3')).dy, lessThan(header));
    expect(tester.getTopLeft(find.text('Titolo 2')).dy, greaterThan(header));
    // Footer: the pill (disabled, no widgets yet) and the clear menu circle.
    expect(find.text('Choose Widgets'), findsOneWidget);

    await tester.tap(find.text('Posticipa'));
    expect(source.calls, ['invoke 3 snooze']);

    // A click on the group opens it.
    await tester.tap(find.text('Titolo 2'));
    await tester.pumpAndSettle();
    expect(find.text('Titolo 1'), findsOneWidget);

    await tester.tap(find.byType(CloseCircle).first);
    await tester.pumpAndSettle();
    expect(source.calls.last, 'clear');
    expect(find.text('No Recent Notifications'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Notifications'), findsNothing);
  });

  test('per-app styles from notifications.json', () {
    final settings = NotificationSettings.parse(
      '{"apps": {"org.example.Chat": {"style": "alerts"}, '
      '"Mail": {"style": "none"}, "Bad": {"style": "loud"}}}',
    );
    expect(
      settings.styleOf(note(1, app: 'Chat', entry: 'org.example.Chat')),
      NotificationStyle.alerts,
    );
    expect(settings.styleOf(note(2, app: 'Mail')), NotificationStyle.none);
    expect(settings.styleOf(note(3, app: 'Bad')), NotificationStyle.banners);
    expect(
      NotificationSettings.parse('not json').styleOf(note(4)),
      NotificationStyle.banners,
    );
  });

  testWidgets('alerts stay with the × visible; style none shows no banner', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(
      tester,
      source,
      settings: const NotificationSettings({
        'Chat': NotificationStyle.alerts,
        'Quiet': NotificationStyle.none,
      }),
    );
    source.post(note(1, app: 'Quiet'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Titolo 1'), findsNothing);
    source.post(note(2, app: 'Chat'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Titolo 2'), findsOneWidget);
    expect(find.byType(CloseCircle), findsOneWidget);
    // Well past the banner duration it is still there.
    await tester.pump(const Duration(seconds: 12));
    expect(find.text('Titolo 2'), findsOneWidget);
  });

  testWidgets('a card dragged right in the Center leaves and is cleared', (
    tester,
  ) async {
    final source = FakeSource()..items[1] = note(1, minute: 1);
    await pumpLayer(tester, source);
    await sendCommand(tester, 'notifications.toggle');
    await tester.pumpAndSettle();
    await tester.drag(find.text('Titolo 1'), const Offset(40, 0));
    await tester.pumpAndSettle();
    expect(source.calls, ['dismiss 1']);
  });

  testWidgets('a long press opens the banner: its actions work and it leaves', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(note(1, actions: [('default', 'Open'), ('snooze', 'Snooze')]));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));
    expect(find.text('Snooze'), findsNothing);
    await tester.longPress(find.text('Titolo 1'));
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Snooze'), findsOneWidget);
    await tester.tap(find.text('Snooze'));
    await tester.pumpAndSettle();
    expect(source.calls, ['invoke 1 snooze']);
    expect(find.text('Titolo 1'), findsNothing);
  });

  testWidgets('the reply field sends its text (inline-reply)', (tester) async {
    final source = FakeSource()
      ..items[1] = note(
        1,
        minute: 1,
        actions: [('inline-reply', 'Reply to Anna')],
      );
    await pumpLayer(tester, source);
    await sendCommand(tester, 'notifications.toggle');
    await tester.pumpAndSettle();
    expect(find.text('Reply to Anna'), findsOneWidget);
    // inline-reply is a field, never a button.
    expect(source.items[1]!.buttons, isEmpty);
    await tester.tap(find.byType(EditableText));
    await tester.enterText(find.byType(EditableText), 'on my way');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(source.calls, ['reply 1 on my way']);
  });

  testWidgets('a click without a default action opens the sender app', (
    tester,
  ) async {
    final source = FakeSource()..items[1] = note(1, app: 'Chat', minute: 1);
    await pumpLayer(tester, source);
    await sendCommand(tester, 'notifications.toggle');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Titolo 1'));
    await tester.pumpAndSettle();
    expect(source.calls, ['open Chat', 'dismiss 1']);
    expect(find.text('Notifications'), findsNothing);
  });

  testWidgets('the footer menu clears every notification', (tester) async {
    final source = FakeSource()
      ..items[1] = note(1, minute: 1)
      ..items[2] = note(2, app: 'Calendar', minute: 2);
    await pumpLayer(tester, source);
    await sendCommand(tester, 'notifications.toggle');
    await tester.pumpAndSettle();
    final pill = tester.getRect(find.byKey(const ValueKey('choose widgets')));
    // The disabled pill does nothing.
    await tester.tap(find.text('Choose Widgets'));
    await tester.pumpAndSettle();
    expect(source.calls, isEmpty);
    expect(find.text('Notifications'), findsOneWidget);
    // The circle 8 pt to the right of the pill opens the menu.
    await tester.tapAt(Offset(pill.right + 8 + 12, pill.center.dy));
    await tester.pumpAndSettle();
    final item = tester.getCenter(find.text('Clear All Notifications'));
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: item - const Offset(0, 1));
    await mouse.moveTo(item);
    await tester.pump();
    await mouse.down(item);
    await mouse.up();
    // The menu blinks the item, then acts: timers, not frames.
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pumpAndSettle();
    expect(source.calls, ['clear']);
  });

  testWidgets(
    'empty Center: the text 30 pt below the column top, the pill 33 under it',
    (tester) async {
      final source = FakeSource();
      await pumpLayer(tester, source);
      await sendCommand(tester, 'notifications.toggle');
      await tester.pumpAndSettle();
      expect(find.text('Notifications'), findsNothing);
      final text = tester.getRect(find.text('No Recent Notifications'));
      final pill = tester.getRect(find.byKey(const ValueKey('choose widgets')));
      expect(pill.top - text.bottom, closeTo(33 + (17 - text.height) / 2, 1.5));
      expect(pill.size, const Size(91, 24));
      expect(pill.center.dx, closeTo(text.center.dx, 0.5));
    },
  );

  testWidgets(
    'with Do Not Disturb no banner, the notification stays in the Center',
    (tester) async {
      final source = FakeSource();
      final system = _Dnd();
      tester.view.physicalSize = const Size(1280, 686);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: NotificationsLayer(source: source, system: system),
        ),
      );
      source.post(note(5));
      await tester.pumpAndSettle();
      expect(find.byType(NotificationCard), findsNothing);
      expect(source.items, contains(5));
    },
  );

  test('exit and Center follow the measurements of session 8', () {
    // Exit: offsets at 13.3 ms steps.
    expect(bannerExitOffset(0), 0);
    expect(bannerExitOffset(1000 / 75 * 4), 141);
    expect(bannerExitOffset(1000), bannerEnterDistance);
    expect(bannerExitOpacity(190), 0.5);
    // Entrance: blurred and transparent at the start, sharp and solid after ~215 ms.
    expect(bannerEnterBlur(0), bannerBlurSigma);
    expect(bannerEnterBlur(215), 0);
    expect(bannerEnterOpacity(210), 1);
    // Center: content edge at ~2198 after 35 ms (2158 at the end, 2545 at the start).
    expect(centerOpenOffset(0), centerDistance);
    expect(centerOpenOffset(35), closeTo(2198 - 2158, 5));
    expect(centerOpenOffset(300), 0);
    expect(centerCloseOffset(120), 37);
    expect(centerCloseOffset(150), 357);
    expect(centerCloseOffset(400), centerDistance);
  });

  testWidgets(
    'a new notification replaces the banner: the old one leaves while it enters',
    (tester) async {
      final source = FakeSource();
      await pumpLayer(tester, source);
      source.post(note(1));
      await tester.pump();
      await tester.pump(bannerEnterDuration);
      source.post(note(2));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 60));
      // For a moment both are there.
      expect(find.text('Titolo 1'), findsOneWidget);
      expect(find.text('Titolo 2'), findsOneWidget);
      await tester.pump(bannerExitDuration);
      await tester.pump();
      expect(find.text('Titolo 1'), findsNothing);
      expect(find.text('Titolo 2'), findsOneWidget);
    },
  );

  testWidgets('under the pointer the banner stays, then leaves ~1.1 s later', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(
      note(3, actions: [('default', 'Open'), ('snooze', 'Posticipa')]),
    );
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    await mouse.moveTo(tester.getCenter(find.text('Titolo 3')));
    await tester.pump();
    // On hover: the × and the Dettagli capsule.
    expect(find.byType(CloseCircle), findsOneWidget);
    expect(find.text('Details'), findsOneWidget);
    await tester.pump(bannerDuration * 2);
    expect(find.text('Titolo 3'), findsOneWidget);

    // Dettagli shows the actions in the banner.
    await tester.tap(find.text('Details'));
    await tester.pump();
    expect(find.text('Posticipa'), findsOneWidget);

    await mouse.moveTo(const Offset(10, 600));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 1000));
    expect(find.text('Titolo 3'), findsOneWidget);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pumpAndSettle();
    expect(find.text('Titolo 3'), findsNothing);
  });

  testWidgets(
    'dragged to the right it leaves anyway, without opening the app',
    (tester) async {
      final source = FakeSource();
      await pumpLayer(tester, source);
      source.post(note(4, actions: [('default', 'Open')]));
      await tester.pump();
      await tester.pump(bannerEnterDuration);
      final start = tester.getTopLeft(find.byType(NotificationCard));
      final drag = await tester.startGesture(
        tester.getCenter(find.text('Titolo 4')),
      );
      await drag.moveBy(const Offset(20, 0));
      await drag.moveBy(const Offset(15, 0));
      await tester.pump();
      // It follows the pointer 1:1 (beyond the recognizer's threshold).
      expect(
        tester.getTopLeft(find.byType(NotificationCard)).dx - start.dx,
        greaterThan(14),
      );
      await drag.up();
      await tester.pumpAndSettle();
      expect(find.text('Titolo 4'), findsNothing);
      expect(source.calls, isEmpty);
      expect(source.items, contains(4));
    },
  );
}

class _Dnd extends ValueNotifier<SystemSnapshot> implements SystemSource {
  _Dnd() : super(const SystemSnapshot(doNotDisturb: true));

  @override
  Future<void> setWifi(bool enabled) async {}
  @override
  Future<void> setBluetooth(bool powered) async {}
  @override
  Future<void> setBrightness(double v) async {}
  @override
  Future<void> setVolume(double v) async {}
  @override
  Future<void> setMuted(bool muted) async {}
  @override
  Future<void> setDoNotDisturb(bool enabled) async {}
  @override
  Future<List<WifiNetwork>> wifiNetworks() async => const [];
  @override
  Future<void> connectWifi(String ssid) async {}
  @override
  Future<List<AudioOutput>> audioOutputs() async => const [];
  @override
  Future<void> setAudioOutput(int id) async {}
  @override
  Future<List<BluetoothDevice>> bluetoothDevices() async => const [];
  @override
  Future<void> connectBluetooth(String address, bool connect) async {}
}
