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
  String app = 'Posta',
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
  test('la struttura D-Bus diventa una notifica', () {
    final n = ShellNotification.fromDBus([
      7, 'Posta', 'mail', 'Titolo', 'Corpo', //
      ['default', 'Apri', 'reply', 'Rispondi'],
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
    'il Centro raggruppa per app, dal più recente, senza le transitorie',
    () {
      final groups = groupNotifications([
        note(1, minute: 1),
        note(2, app: 'Messaggi', minute: 2),
        note(3, minute: 3),
        note(4, minute: 4, transient: true),
      ]);
      expect([for (final g in groups) g.key], ['Posta', 'Messaggi']);
      expect([for (final n in groups.first.items) n.id], [3, 1]);
    },
  );

  test("l'entrata del banner segue la decelerazione misurata", () {
    expect(bannerEnterOffset(0), bannerEnterDistance);
    // Un fotogramma a 75 Hz riduce la distanza residua a ~0,82.
    final ratio = bannerEnterOffset(13.33 * 5) / bannerEnterOffset(13.33 * 4);
    expect(ratio, closeTo(0.82, 0.01));
    expect(bannerEnterOffset(bannerEnterDuration.inMilliseconds.toDouble()), 0);
  });

  testWidgets('un banner entra in alto a destra e se ne va da solo', (
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
    // Resta nel Centro.
    expect(source.items, contains(1));
  });

  testWidgets('le notifiche critiche restano, un clic attiva la predefinita', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(note(2, urgency: 2, actions: [('default', 'Apri')]));
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    await tester.pump(bannerDuration * 3);
    expect(find.text('Titolo 2'), findsOneWidget);
    await tester.tap(find.text('Titolo 2'));
    await tester.pumpAndSettle();
    expect(source.calls, ['invoke 2 default']);
    expect(find.text('Titolo 2'), findsNothing);
  });

  testWidgets('il Centro mostra gruppi impilati, azioni ed elimina tutto', (
    tester,
  ) async {
    final source = FakeSource()
      ..items[1] = note(1, minute: 1)
      ..items[2] = note(2, minute: 2)
      ..items[3] = note(
        3,
        app: 'Calendario',
        minute: 3,
        actions: [('default', 'Apri'), ('snooze', 'Posticipa')],
      );
    await pumpLayer(tester, source);
    await sendCommand(tester, 'notifications.toggle');
    await tester.pumpAndSettle();

    expect(find.text('Notifiche'), findsOneWidget);
    // Calendario è il più recente; Posta è chiuso con una carta sotto.
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

    // Un clic sul gruppo lo apre.
    await tester.tap(find.text('Titolo 2'));
    await tester.pumpAndSettle();
    expect(find.text('Titolo 1'), findsOneWidget);

    await tester.tap(find.byType(CloseCircle).first);
    await tester.pumpAndSettle();
    expect(source.calls.last, 'clear');
    expect(find.text('Nessuna notifica'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.text('Notifiche'), findsNothing);
  });

  testWidgets(
    'con Non disturbare niente banner, la notifica resta nel Centro',
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

  test('uscita e Centro seguono le misure della sessione 8', () {
    // Uscita: spostamenti a passi di 13,3 ms.
    expect(bannerExitOffset(0), 0);
    expect(bannerExitOffset(1000 / 75 * 4), 141);
    expect(bannerExitOffset(1000), bannerEnterDistance);
    expect(bannerExitOpacity(190), 0.5);
    // Entrata: sfocata e trasparente all'inizio, a fuoco e piena dopo ~215 ms.
    expect(bannerEnterBlur(0), bannerBlurSigma);
    expect(bannerEnterBlur(215), 0);
    expect(bannerEnterOpacity(210), 1);
    // Centro: bordo dei contenuti a ~2198 dopo 35 ms (2158 a fine corsa, 2545 all'inizio).
    expect(centerOpenOffset(0), centerDistance);
    expect(centerOpenOffset(35), closeTo(2198 - 2158, 5));
    expect(centerOpenOffset(300), 0);
    expect(centerCloseOffset(120), 37);
    expect(centerCloseOffset(150), 357);
    expect(centerCloseOffset(400), centerDistance);
  });

  testWidgets(
    'una notifica nuova sostituisce il banner: il vecchio esce mentre entra',
    (tester) async {
      final source = FakeSource();
      await pumpLayer(tester, source);
      source.post(note(1));
      await tester.pump();
      await tester.pump(bannerEnterDuration);
      source.post(note(2));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 60));
      // Per un momento ci sono entrambi.
      expect(find.text('Titolo 1'), findsOneWidget);
      expect(find.text('Titolo 2'), findsOneWidget);
      await tester.pump(bannerExitDuration);
      await tester.pump();
      expect(find.text('Titolo 1'), findsNothing);
      expect(find.text('Titolo 2'), findsOneWidget);
    },
  );

  testWidgets('sotto il puntatore il banner resta, poi esce ~1,1 s dopo', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(
      note(3, actions: [('default', 'Apri'), ('snooze', 'Posticipa')]),
    );
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);
    await mouse.moveTo(tester.getCenter(find.text('Titolo 3')));
    await tester.pump();
    // Al passaggio: la × e la capsula Dettagli.
    expect(find.byType(CloseCircle), findsOneWidget);
    expect(find.text('Dettagli'), findsOneWidget);
    await tester.pump(bannerDuration * 2);
    expect(find.text('Titolo 3'), findsOneWidget);

    // Dettagli mostra le azioni nel banner.
    await tester.tap(find.text('Dettagli'));
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

  testWidgets('trascinato verso destra esce comunque, senza aprire l\'app', (
    tester,
  ) async {
    final source = FakeSource();
    await pumpLayer(tester, source);
    source.post(note(4, actions: [('default', 'Apri')]));
    await tester.pump();
    await tester.pump(bannerEnterDuration);
    final start = tester.getTopLeft(find.byType(NotificationCard));
    final drag = await tester.startGesture(
      tester.getCenter(find.text('Titolo 4')),
    );
    await drag.moveBy(const Offset(20, 0));
    await drag.moveBy(const Offset(15, 0));
    await tester.pump();
    // Segue il puntatore 1:1 (oltre la soglia del riconoscitore).
    expect(
      tester.getTopLeft(find.byType(NotificationCard)).dx - start.dx,
      greaterThan(14),
    );
    await drag.up();
    await tester.pumpAndSettle();
    expect(find.text('Titolo 4'), findsNothing);
    expect(source.calls, isEmpty);
    expect(source.items, contains(4));
  });
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
}
