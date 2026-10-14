import 'dart:async';

import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:silica_shell/notifications/model.dart';
import 'package:silica_shell/notifications/notifications.dart';
import 'package:silica_shell/notifications/service.dart';
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
}) => ShellNotification(
  id: id,
  appName: app,
  summary: 'Titolo $id',
  body: 'Corpo della notifica $id',
  actions: actions,
  urgency: urgency,
  transient: transient,
  timestamp: DateTime(2026, 10, 13, 9, minute),
);

Future<void> pumpLayer(WidgetTester tester, FakeSource source) async {
  tester.view.physicalSize = const Size(1280, 686);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: NotificationsLayer(source: source),
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

    await tester.tap(find.text('Posticipa'));
    expect(source.calls, ['invoke 3 snooze']);

    // A click on the group opens it.
    await tester.tap(find.text('Titolo 2'));
    await tester.pumpAndSettle();
    expect(find.text('Titolo 1'), findsOneWidget);

    await tester.tap(find.byType(CloseCircle).first);
    await tester.pumpAndSettle();
    expect(source.calls.last, 'clear');
    expect(find.text('No notifications'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Notifications'), findsNothing);
  });

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
